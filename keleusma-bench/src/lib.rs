//! Calibration tool for the Keleusma cost model.
//!
//! Measures pipelined-cycle cost per opcode on a host CPU and emits
//! a generated `op_cycles` function that the runtime can use for
//! WCET analysis on that host. See the crate-level README for usage
//! and methodology.
//!
//! Architecture extensibility lives in [`counter`]. Opcode coverage
//! lives in the [`OPCODE_SPECS`] table here. Source emission lives in
//! [`emit_cost_model_source`].

pub mod counter;

use std::collections::BTreeMap;

use keleusma::Arena;
use keleusma::bytecode::{
    BlockType, Chunk, ConstValue, Module, Op, RUNTIME_ADDRESS_BITS_LOG2, RUNTIME_FLOAT_BITS_LOG2,
    RUNTIME_WORD_BITS_LOG2, StructTemplate,
};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm};

use crate::counter::CycleCounter;

/// Specification for benchmarking a single opcode. The benchmark
/// engine constructs a Func chunk that inlines the spec's pattern
/// many times, runs the chunk in tight repetition, and computes the
/// per-pattern cycle cost.
///
/// To add a new opcode, append a `OpcodeSpec` to [`OPCODE_SPECS`]
/// with appropriate setup and cleanup operations to keep the operand
/// stack balanced across pattern repetitions. The benchmark engine is
/// otherwise architecture-independent.
pub struct OpcodeSpec {
    /// Display name for the opcode used in generated output.
    pub name: &'static str,

    /// Function constructing the operations sequence to inline. The
    /// sequence must leave the operand stack at the same depth it
    /// found it. The target opcode is included in the sequence
    /// somewhere; surrounding ops set up and tear down its stack
    /// state.
    pub build: fn() -> Vec<Op>,

    /// Constants the spec uses, indexed by the `Op::Const` operands
    /// in the build output. The benchmark engine populates the
    /// chunk's constant pool with these.
    pub constants: &'static [ConstValueDescriptor],

    /// Number of operations the build returns. Used to compute the
    /// per-pattern cost from the per-iteration measurement.
    pub ops_per_pattern: u32,
}

/// Helper enum describing constants the spec wants in the constant
/// pool. The benchmark engine converts these to `ConstValue` at
/// chunk-construction time.
#[derive(Clone, Copy)]
pub enum ConstValueDescriptor {
    Int(i64),
    Bool(bool),
}

impl ConstValueDescriptor {
    fn into_const_value(self) -> ConstValue {
        match self {
            ConstValueDescriptor::Int(v) => ConstValue::Int(v),
            ConstValueDescriptor::Bool(v) => ConstValue::Bool(v),
        }
    }
}

/// Number of times the opcode pattern is inlined into the benchmark
/// chunk. Large values are required because architectural cycle
/// counters such as AArch64's CNTVCT_EL0 run at rates well below the
/// CPU clock (typically 24 MHz to 100 MHz). Short runs may not
/// span enough counter ticks to produce useful resolution. The
/// chunk size grows linearly with this value but has a fixed
/// per-iteration cost in instruction-cache footprint.
pub const PATTERN_REPETITIONS: u32 = 100_000;

/// Number of measurement passes. The minimum across passes is taken
/// as the pipelined-cycle estimate, on the rationale that the
/// minimum corresponds to the run with warmest caches and best
/// branch prediction.
pub const MEASUREMENT_PASSES: u32 = 16;

/// Number of warmup passes before measurement. Warms instruction and
/// data caches and stabilizes the branch predictor before
/// measurement begins.
pub const WARMUP_PASSES: u32 = 4;

/// Builds a Func chunk that executes the given opcode pattern
/// `repetitions` times. The chunk has a `Return` at the end so
/// `Vm::call` exits cleanly.
fn build_benchmark_chunk(pattern: &[Op], constants: &[ConstValue], repetitions: u32) -> Module {
    let mut ops: Vec<Op> = Vec::with_capacity(pattern.len() * repetitions as usize + 1);
    for _ in 0..repetitions {
        ops.extend_from_slice(pattern);
    }
    ops.push(Op::Return);

    let chunk = Chunk {
        name: String::from("bench"),
        ops,
        constants: constants.to_vec(),
        struct_templates: Vec::<StructTemplate>::new(),
        local_count: 4,
        param_count: 0,
        block_type: BlockType::Func,
    };

    Module {
        chunks: vec![chunk],
        native_names: Vec::new(),
        entry_point: Some(0),
        data_layout: None,
        word_bits_log2: RUNTIME_WORD_BITS_LOG2,
        addr_bits_log2: RUNTIME_ADDRESS_BITS_LOG2,
        float_bits_log2: RUNTIME_FLOAT_BITS_LOG2,
        wcet_cycles: 0,
        wcmu_bytes: 0,
    }
}

/// Run the benchmark for a single opcode spec and return the
/// estimated pipelined-cycle cost per pattern repetition.
///
/// The function builds the benchmark chunk, constructs a VM, runs
/// warmup passes, runs measurement passes, and returns the minimum
/// observed per-pattern cycle count. The minimum approximates
/// pipelined cycles because warm caches and stable branch prediction
/// produce the lowest observed cycle count.
///
/// Internal arithmetic uses `f64` to preserve precision when the
/// raw counter ticks per pattern is below one. The result is
/// rounded to `u64` at the end.
pub fn benchmark_spec(counter: &dyn CycleCounter, spec: &OpcodeSpec) -> f64 {
    let pattern = (spec.build)();
    let constants: Vec<ConstValue> = spec
        .constants
        .iter()
        .map(|c| c.into_const_value())
        .collect();

    let module = build_benchmark_chunk(&pattern, &constants, PATTERN_REPETITIONS);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);

    // SAFETY: The bench tool deliberately uses the unchecked
    // constructor because the benchmark chunks are not
    // Stream-classified and the resource bounds check would not
    // apply meaningfully. The chunks are well-formed by construction
    // through this crate.
    let mut vm = unsafe { Vm::new_unchecked(module, &arena) }
        .expect("benchmark module passes structural verification");

    // Warmup. Run the chunk a few times to warm caches and predictor.
    for _ in 0..WARMUP_PASSES {
        let _ = vm.call(&[]);
    }

    let mut min_per_pattern = f64::MAX;
    for _ in 0..MEASUREMENT_PASSES {
        let start = counter.read();
        let _ = std::hint::black_box(vm.call(&[]));
        let end = counter.read();
        let total = end.wrapping_sub(start) as f64;
        let per_pattern = total / PATTERN_REPETITIONS as f64;
        if per_pattern < min_per_pattern {
            min_per_pattern = per_pattern;
        }
    }
    min_per_pattern
}

/// Result of measuring a single opcode spec. Carries both the
/// floating-point pattern measurement (for diagnostic precision) and
/// the rounded per-op count used in the generated cost model. A
/// minimum reported value of 1 ensures the cost model never reports
/// zero cycles, which would be unsound for use in WCET analysis.
pub struct Measurement {
    pub name: &'static str,
    pub cycles_per_pattern: f64,
    pub ops_per_pattern: u32,
    pub cycles_per_op: u32,
}

/// Run the full benchmark suite and return per-opcode measurements.
pub fn measure_all(counter: &dyn CycleCounter) -> Vec<Measurement> {
    OPCODE_SPECS
        .iter()
        .map(|spec| {
            let cycles_per_pattern = benchmark_spec(counter, spec);
            let raw = cycles_per_pattern / spec.ops_per_pattern as f64;
            // Round up and clamp to at least 1 so the cost model
            // never returns zero. A zero-cost opcode would let WCET
            // analysis claim free execution, which is unsound.
            let cycles_per_op = raw.ceil().max(1.0) as u32;
            Measurement {
                name: spec.name,
                cycles_per_pattern,
                ops_per_pattern: spec.ops_per_pattern,
                cycles_per_op,
            }
        })
        .collect()
}

/// Aggregate measurements by category and emit a Rust source fragment
/// implementing `measured_op_cycles`. The fragment is written to a
/// file the host includes into its build.
///
/// The output uses the same opcode-category structure as the bundled
/// `nominal_op_cycles`. Category cycle counts are computed as the
/// maximum over the category's representative opcodes, which is
/// conservative for pipelined-cycle WCET.
pub fn emit_cost_model_source(measurements: &[Measurement], counter_name: &str) -> String {
    let by_name: BTreeMap<&str, u32> = measurements
        .iter()
        .map(|m| (m.name, m.cycles_per_op))
        .collect();

    // Aggregate by category. The category key matches the structure
    // of `nominal_op_cycles` in `bytecode.rs`. Each category lists
    // the representative spec names that contribute to its bound;
    // the category's reported cycle count is the maximum over those
    // contributors.
    let categories: &[(&str, &[&str])] = &[
        (
            "data_movement",
            &["Const", "PushUnit", "GetLocal", "Pop", "Dup"],
        ),
        ("control_marker", &["Yield"]),
        (
            "arithmetic",
            &["Add", "Sub", "Mul", "Neg", "CmpEq", "CmpLt"],
        ),
        ("division", &["Div", "Mod"]),
        ("composite_construction", &["NewArray", "NewTuple"]),
        ("function_call", &["Call"]),
        ("closure_construction", &["MakeClosure"]),
    ];

    let mut category_costs: BTreeMap<&str, u32> = BTreeMap::new();
    for (cat, names) in categories {
        let mut max_cost: u32 = 0;
        for name in *names {
            if let Some(&c) = by_name.get(name)
                && c > max_cost
            {
                max_cost = c;
            }
        }
        if max_cost == 0 {
            max_cost = 1;
        }
        category_costs.insert(cat, max_cost);
    }

    let cat = |name: &str| category_costs.get(name).copied().unwrap_or(1);

    let mut out = String::new();
    out.push_str("// Generated by keleusma-bench. Do not edit by hand.\n");
    out.push_str("//\n");
    out.push_str(&format!("// Counter: {}\n", counter_name));
    out.push_str("//\n");
    out.push_str("// Cycle counts are pipelined-cycle estimates measured on the\n");
    out.push_str("// host that ran the benchmark. Pipelined cycles assume warm\n");
    out.push_str("// caches, correct branch prediction, and no memory-bus contention.\n");
    out.push_str("// Hosts converting to wall-clock time apply a calibration factor.\n");
    out.push_str("//\n");
    out.push_str("// Per-opcode raw measurements:\n");
    out.push_str("//   name                         per-pattern (f64)   per-op (u32, min 1)\n");
    for m in measurements {
        out.push_str(&format!(
            "//   {:<28} {:>16.4}   {:>10}\n",
            m.name, m.cycles_per_pattern, m.cycles_per_op
        ));
    }
    out.push('\n');
    out.push_str("pub fn measured_op_cycles(op: &keleusma::bytecode::Op) -> u32 {\n");
    out.push_str("    use keleusma::bytecode::Op;\n");
    out.push_str("    match op {\n");

    let dm = cat("data_movement");
    out.push_str(&format!(
        "        // Data movement and trivial control flow ({} cycles).\n",
        dm
    ));
    out.push_str("        Op::Const(_)\n");
    out.push_str("        | Op::PushUnit\n");
    out.push_str("        | Op::PushTrue\n");
    out.push_str("        | Op::PushFalse\n");
    out.push_str("        | Op::GetLocal(_)\n");
    out.push_str("        | Op::SetLocal(_)\n");
    out.push_str("        | Op::GetData(_)\n");
    out.push_str("        | Op::SetData(_)\n");
    out.push_str("        | Op::Pop\n");
    out.push_str("        | Op::Dup\n");
    out.push_str("        | Op::PushNone\n");
    out.push_str("        | Op::WrapSome\n");
    out.push_str("        | Op::Not => ");
    out.push_str(&format!("{},\n\n", dm));

    let cm = cat("control_marker");
    out.push_str(&format!(
        "        // Control flow markers ({} cycles).\n",
        cm
    ));
    out.push_str("        Op::If(_)\n");
    out.push_str("        | Op::Else(_)\n");
    out.push_str("        | Op::EndIf\n");
    out.push_str("        | Op::Loop(_)\n");
    out.push_str("        | Op::EndLoop(_)\n");
    out.push_str("        | Op::Break(_)\n");
    out.push_str("        | Op::BreakIf(_)\n");
    out.push_str("        | Op::Stream\n");
    out.push_str("        | Op::Reset\n");
    out.push_str("        | Op::Yield\n");
    out.push_str("        | Op::Trap(_) => ");
    out.push_str(&format!("{},\n\n", cm));

    let ar = cat("arithmetic");
    out.push_str(&format!(
        "        // Arithmetic and comparison ({} cycles).\n",
        ar
    ));
    out.push_str("        Op::Add\n");
    out.push_str("        | Op::Sub\n");
    out.push_str("        | Op::Mul\n");
    out.push_str("        | Op::Neg\n");
    out.push_str("        | Op::CmpEq\n");
    out.push_str("        | Op::CmpNe\n");
    out.push_str("        | Op::CmpLt\n");
    out.push_str("        | Op::CmpGt\n");
    out.push_str("        | Op::CmpLe\n");
    out.push_str("        | Op::CmpGe\n");
    out.push_str("        | Op::GetIndex\n");
    out.push_str("        | Op::GetTupleField(_)\n");
    out.push_str("        | Op::GetEnumField(_)\n");
    out.push_str("        | Op::Len\n");
    out.push_str("        | Op::IntToFloat\n");
    out.push_str("        | Op::FloatToInt\n");
    out.push_str("        | Op::Return => ");
    out.push_str(&format!("{},\n\n", ar));

    let dv = cat("division");
    out.push_str(&format!(
        "        // Division, field lookup, type checks ({} cycles).\n",
        dv
    ));
    out.push_str("        Op::Div\n");
    out.push_str("        | Op::Mod\n");
    out.push_str("        | Op::GetField(_)\n");
    out.push_str("        | Op::IsEnum(_, _)\n");
    out.push_str("        | Op::IsStruct(_) => ");
    out.push_str(&format!("{},\n\n", dv));

    let cc = cat("composite_construction");
    out.push_str(&format!(
        "        // Composite value construction ({} cycles).\n",
        cc
    ));
    out.push_str("        Op::NewStruct(_)\n");
    out.push_str("        | Op::NewEnum(_, _, _)\n");
    out.push_str("        | Op::NewArray(_)\n");
    out.push_str("        | Op::NewTuple(_) => ");
    out.push_str(&format!("{},\n\n", cc));

    let fc = cat("function_call");
    out.push_str(&format!("        // Function calls ({} cycles).\n", fc));
    out.push_str("        Op::Call(_, _)\n");
    out.push_str("        | Op::CallNative(_, _)\n");
    out.push_str("        | Op::CallIndirect(_) => ");
    out.push_str(&format!("{},\n", fc));
    out.push_str("        Op::PushFunc(_) => 0,\n\n");

    let cl = cat("closure_construction");
    out.push_str(&format!(
        "        // Closure construction ({} cycles).\n",
        cl
    ));
    out.push_str("        Op::MakeClosure(_, _)\n");
    out.push_str("        | Op::MakeRecursiveClosure(_, _) => ");
    out.push_str(&format!("{},\n", cl));

    out.push_str("    }\n");
    out.push_str("}\n");
    out.push('\n');
    out.push_str("/// Measured cost model for the host that ran the benchmark.\n");
    out.push_str("/// Uses the runtime's value-slot byte size paired with the\n");
    out.push_str("/// measured per-opcode cycle table.\n");
    out.push_str("pub const MEASURED_COST_MODEL: keleusma::CostModel = keleusma::CostModel {\n");
    out.push_str("    value_slot_bytes: keleusma::VALUE_SLOT_SIZE_BYTES,\n");
    out.push_str("    op_cycles: measured_op_cycles,\n");
    out.push_str("};\n");

    out
}

/// Master table of opcode benchmark specs. Each entry tells the
/// benchmark engine how to exercise one opcode. To add coverage for a
/// new opcode, append an entry here with appropriate setup and
/// cleanup ops to keep the operand stack balanced across pattern
/// repetitions.
///
/// The patterns favor balance over isolation. A pattern for `Add`
/// pushes two values, executes Add, and pops the result, so that
/// repetition leaves the stack depth unchanged. The reported cycles
/// per pattern are divided by the number of ops in the pattern to
/// give a per-opcode-contribution estimate.
pub const OPCODE_SPECS: &[OpcodeSpec] = &[
    OpcodeSpec {
        name: "Const",
        build: || vec![Op::Const(0), Op::Pop],
        constants: &[ConstValueDescriptor::Int(0)],
        ops_per_pattern: 2,
    },
    OpcodeSpec {
        name: "PushUnit",
        build: || vec![Op::PushUnit, Op::Pop],
        constants: &[],
        ops_per_pattern: 2,
    },
    OpcodeSpec {
        name: "PushTrue",
        build: || vec![Op::PushTrue, Op::Pop],
        constants: &[],
        ops_per_pattern: 2,
    },
    OpcodeSpec {
        name: "GetLocal",
        build: || vec![Op::GetLocal(0), Op::Pop],
        constants: &[],
        ops_per_pattern: 2,
    },
    OpcodeSpec {
        name: "Pop",
        build: || vec![Op::PushUnit, Op::Pop],
        constants: &[],
        ops_per_pattern: 2,
    },
    OpcodeSpec {
        name: "Dup",
        build: || vec![Op::PushUnit, Op::Dup, Op::Pop, Op::Pop],
        constants: &[],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "Add",
        build: || vec![Op::Const(0), Op::Const(0), Op::Add, Op::Pop],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "Sub",
        build: || vec![Op::Const(0), Op::Const(0), Op::Sub, Op::Pop],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "Mul",
        build: || vec![Op::Const(0), Op::Const(0), Op::Mul, Op::Pop],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "Div",
        build: || vec![Op::Const(0), Op::Const(0), Op::Div, Op::Pop],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "Mod",
        build: || vec![Op::Const(0), Op::Const(0), Op::Mod, Op::Pop],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "Neg",
        build: || vec![Op::Const(0), Op::Neg, Op::Pop],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 3,
    },
    OpcodeSpec {
        name: "CmpEq",
        build: || vec![Op::Const(0), Op::Const(0), Op::CmpEq, Op::Pop],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "CmpLt",
        build: || vec![Op::Const(0), Op::Const(0), Op::CmpLt, Op::Pop],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "Not",
        build: || vec![Op::PushTrue, Op::Not, Op::Pop],
        constants: &[],
        ops_per_pattern: 3,
    },
    OpcodeSpec {
        name: "NewArray",
        build: || {
            vec![
                Op::Const(0),
                Op::Const(0),
                Op::Const(0),
                Op::NewArray(3),
                Op::Pop,
            ]
        },
        constants: &[ConstValueDescriptor::Int(0)],
        ops_per_pattern: 5,
    },
    OpcodeSpec {
        name: "NewTuple",
        build: || vec![Op::Const(0), Op::Const(0), Op::NewTuple(2), Op::Pop],
        constants: &[ConstValueDescriptor::Int(0)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "Yield",
        // Yield takes a value off the stack. Cannot be benchmarked
        // inside a Func chunk because Func chunks reject yields.
        // Substitute Pop+PushUnit as a control-flow-marker proxy.
        // The category emitter treats this as the marker class.
        build: || vec![Op::PushUnit, Op::Pop],
        constants: &[],
        ops_per_pattern: 2,
    },
    OpcodeSpec {
        name: "Call",
        // Direct call to a trivial function chunk. The benchmark
        // module would need a callee chunk; for this simple version
        // we measure a no-op pattern as a placeholder. A full version
        // would construct a multi-chunk module with a trivial callee.
        build: || vec![Op::PushUnit, Op::Pop],
        constants: &[],
        ops_per_pattern: 2,
    },
    OpcodeSpec {
        name: "MakeClosure",
        // MakeClosure requires a chunk index. The simple version
        // measures a substitute pattern. A full version would
        // construct a multi-chunk module with a closure target.
        build: || vec![Op::PushUnit, Op::Pop],
        constants: &[],
        ops_per_pattern: 2,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcode_specs_table_is_nonempty() {
        assert!(!OPCODE_SPECS.is_empty());
    }

    #[test]
    fn opcode_specs_have_balanced_stack_patterns() {
        // Each pattern's net stack effect should be zero so that
        // repeated execution does not exhaust or grow the stack
        // unboundedly. This is an invariant of the benchmark
        // construction.
        for spec in OPCODE_SPECS {
            let pattern = (spec.build)();
            let mut depth: i32 = 0;
            for op in &pattern {
                depth += op.stack_growth() as i32;
                depth -= op.stack_shrink() as i32;
            }
            assert_eq!(
                depth, 0,
                "pattern for {} has unbalanced stack effect {}",
                spec.name, depth
            );
        }
    }

    #[test]
    fn benchmark_runs_to_completion() {
        // Quick smoke test: run a single small spec through the
        // benchmark engine and verify the result is finite and
        // nonnegative.
        let counter = counter::default_counter();
        let spec = &OPCODE_SPECS[0];
        let cycles = benchmark_spec(counter.as_ref(), spec);
        assert!(cycles >= 0.0);
        assert!(cycles.is_finite());
    }

    #[test]
    fn emit_produces_compilable_skeleton() {
        // Generate output and verify it contains expected markers.
        let measurements = vec![Measurement {
            name: "Const",
            cycles_per_pattern: 10.0,
            ops_per_pattern: 2,
            cycles_per_op: 5,
        }];
        let source = emit_cost_model_source(&measurements, "test counter");
        assert!(source.contains("pub fn measured_op_cycles"));
        assert!(source.contains("MEASURED_COST_MODEL"));
        assert!(source.contains("test counter"));
    }
}
