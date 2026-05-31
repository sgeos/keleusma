#![deny(missing_docs)]
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
//!
//! The crate is `no_std + alloc`-compatible when the `std` feature is
//! disabled. Under no_std, the env-variable override for the CPU
//! clock assumption is unavailable; the host runner that calls this
//! crate from std code retains the override. The embedded path
//! consumes only the measurement primitives ([`OPCODE_SPECS`] and
//! [`benchmark_spec`]) and reports raw measurements through the
//! host's chosen transport (defmt RTT for Cortex-M).

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

pub mod counter;

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
    /// Signed integer constant.
    Int(i64),
    /// Boolean constant.
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

/// Default number of times the opcode pattern is inlined into the
/// benchmark chunk on the host. Large values are required because
/// architectural cycle counters such as AArch64's CNTVCT_EL0 run at
/// rates well below the CPU clock (typically 24 MHz to 100 MHz);
/// short runs may not span enough counter ticks to produce useful
/// resolution. Embedded targets whose counter runs at CPU clock
/// (Cortex-M DWT_CYCCNT) can use a much smaller value to keep the
/// constructed chunk inside the device's RAM budget. See
/// [`BenchConfig`].
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

/// Runtime configuration for the bench harness. The host CLI uses
/// [`BenchConfig::host_default`]; embedded callers construct a
/// smaller-repetition variant via [`BenchConfig::embedded_default`]
/// so the constructed chunk fits in device RAM.
#[derive(Clone, Copy, Debug)]
pub struct BenchConfig {
    /// Number of times the opcode pattern is inlined into the
    /// benchmark chunk. Larger values amortise counter resolution
    /// against pattern cost; smaller values fit device-RAM budgets.
    pub repetitions: u32,
    /// Number of warmup passes before measurement begins. Warms
    /// instruction and data caches and stabilises the branch
    /// predictor.
    pub warmup_passes: u32,
    /// Number of measured passes. The minimum across passes is
    /// reported as the pipelined-cycle estimate.
    pub measurement_passes: u32,
    /// Arena capacity in bytes for the constructed VM. Must be large
    /// enough to hold the operand stack and call frames produced by
    /// the inlined pattern.
    pub arena_capacity: usize,
}

impl BenchConfig {
    /// Configuration suitable for host benchmarking, where counter
    /// resolution is the constraint and chunk-size growth is
    /// acceptable.
    pub const fn host_default() -> Self {
        Self {
            repetitions: PATTERN_REPETITIONS,
            warmup_passes: WARMUP_PASSES,
            measurement_passes: MEASUREMENT_PASSES,
            arena_capacity: DEFAULT_ARENA_CAPACITY,
        }
    }

    /// Configuration suitable for embedded targets, where the
    /// counter ticks at CPU clock (so coarse resolution is not a
    /// problem) and the constructed chunk plus the per-Vm arena
    /// must fit in device RAM, with margin for the linked-list
    /// allocator's fragmentation across the seventeen sequential
    /// spec builds. Two hundred pattern repetitions keep the ops
    /// vector under 5 KB; an 8 KB arena suffices because the bench
    /// patterns leave the operand stack near empty between
    /// iterations. The product of both reductions keeps each
    /// spec's allocation footprint under 20 KB, comfortably within
    /// the N6's heap budget even after fragmentation.
    ///
    /// Resolution is not a concern at this scale: at the N6's
    /// 800 MHz CPU clock and per-pattern costs of one thousand to
    /// ten thousand cycles, two hundred repetitions still cover
    /// hundreds of thousands of cycles per measurement pass, which
    /// the DWT_CYCCNT counter measures at single-cycle resolution.
    pub const fn embedded_default() -> Self {
        Self {
            repetitions: 200,
            warmup_passes: WARMUP_PASSES,
            measurement_passes: MEASUREMENT_PASSES,
            arena_capacity: 8 * 1024,
        }
    }
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self::host_default()
    }
}

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
        param_types: Vec::new(),
    };

    Module {
        schema_hash: 0,
        chunks: vec![chunk],
        native_names: Vec::new(),
        entry_point: Some(0),
        data_layout: None,
        word_bits_log2: RUNTIME_WORD_BITS_LOG2,
        addr_bits_log2: RUNTIME_ADDRESS_BITS_LOG2,
        float_bits_log2: RUNTIME_FLOAT_BITS_LOG2,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        flags: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
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
    benchmark_spec_with_config(counter, spec, BenchConfig::host_default())
}

/// Same as [`benchmark_spec`] but with explicit configuration. Used
/// by embedded callers that need a smaller chunk-size repetition
/// count to fit in device RAM.
pub fn benchmark_spec_with_config(
    counter: &dyn CycleCounter,
    spec: &OpcodeSpec,
    config: BenchConfig,
) -> f64 {
    let pattern = (spec.build)();
    let constants: Vec<ConstValue> = spec
        .constants
        .iter()
        .map(|c| c.into_const_value())
        .collect();

    let module = build_benchmark_chunk(&pattern, &constants, config.repetitions);
    let arena = Arena::with_capacity(config.arena_capacity);

    // SAFETY: The bench tool deliberately uses the unchecked
    // constructor because the benchmark chunks are not
    // Stream-classified and the resource bounds check would not
    // apply meaningfully. The chunks are well-formed by construction
    // through this crate.
    let mut vm = unsafe { Vm::new_unchecked(module, &arena) }
        .expect("benchmark module passes structural verification");

    // Warmup. Run the chunk a few times to warm caches and predictor.
    // A non-Ok result indicates a stale or malformed spec; the bench
    // surfaces the error to stderr (when `std` is available) so the
    // operator can diagnose rather than silently reporting zero
    // cycles. Under no_std the warmup failure short-circuits silently
    // and returns zero; downstream callers see the zero and report
    // it through their own channel.
    for _ in 0..config.warmup_passes {
        if let Err(_e) = vm.call(&[]) {
            #[cfg(feature = "std")]
            eprintln!("  warning: warmup vm.call returned Err({:?})", _e);
            return 0.0;
        }
    }

    let scale = counter.cpu_cycles_per_count();
    let mut min_per_pattern = f64::MAX;
    for _ in 0..config.measurement_passes {
        let start = counter.read();
        let _ = core::hint::black_box(vm.call(&[]));
        let end = counter.read();
        // Convert raw counter delta to CPU cycles before dividing
        // across patterns. Counters that run below CPU clock speed
        // (such as AArch64 CNTVCT_EL0 at 24 MHz on Apple Silicon)
        // must scale to CPU cycles or the reported per-pattern value
        // is in counter ticks, not CPU cycles. The scaling depends on
        // the counter's `cpu_cycles_per_count` method, which honors
        // the `KELEUSMA_BENCH_CPU_HZ` environment variable.
        let total_cpu_cycles = (end.wrapping_sub(start) as f64) * scale;
        let per_pattern = total_cpu_cycles / config.repetitions as f64;
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
    /// Static name identifying the opcode category measured.
    pub name: &'static str,
    /// Raw per-pattern CPU-cycle measurement (the minimum across
    /// `measurement_passes`).
    pub cycles_per_pattern: f64,
    /// Number of opcodes the pattern executes.
    pub ops_per_pattern: u32,
    /// Reported per-op cost, the ceiling of `cycles_per_pattern`
    /// saturated to at least `1` so the emitted cost model never
    /// reports a zero-cost opcode.
    pub cycles_per_op: u32,
}

/// Measure a single spec and return a [`Measurement`]. Used by the
/// embedded path that emits each measurement through defmt rather
/// than collecting the full table. The host CLI collects through
/// [`measure_all`] instead.
pub fn measure_one(counter: &dyn CycleCounter, spec: &OpcodeSpec) -> Measurement {
    measure_one_with_config(counter, spec, BenchConfig::host_default())
}

/// Same as [`measure_one`] but with explicit configuration. The
/// embedded `bench_n6` binary calls this with
/// [`BenchConfig::embedded_default`] so the constructed chunk fits in
/// device RAM.
pub fn measure_one_with_config(
    counter: &dyn CycleCounter,
    spec: &OpcodeSpec,
    config: BenchConfig,
) -> Measurement {
    let cycles_per_pattern = benchmark_spec_with_config(counter, spec, config);
    let cycles_per_op = libm::ceil(cycles_per_pattern).max(1.0) as u32;
    Measurement {
        name: spec.name,
        cycles_per_pattern,
        ops_per_pattern: spec.ops_per_pattern,
        cycles_per_op,
    }
}

/// Run the full benchmark suite and return per-opcode measurements.
///
/// The reported `cycles_per_op` is computed as `ceil(cycles_per_pattern)`,
/// not `ceil(cycles_per_pattern / ops_per_pattern)`. The per-pattern
/// quantity already approximates the marginal cost of executing one
/// instance of the pattern (which exercises the target opcode plus
/// minimal setup and cleanup). Dividing by the pattern's op count
/// would distribute the cost across all ops in the pattern and
/// would lose the relative ordering between opcodes when the per-op
/// fractional value is below one architectural counter tick. A
/// counter that runs at a fraction of CPU clock speed (such as
/// AArch64's CNTVCT_EL0 at 24 MHz on Apple Silicon) makes that
/// outcome the common case. Using `cycles_per_pattern` directly
/// preserves the ordering at the cost of overstating per-op cost
/// in absolute terms, which is conservative for WCET.
pub fn measure_all(counter: &dyn CycleCounter) -> Vec<Measurement> {
    OPCODE_SPECS
        .iter()
        .map(|spec| {
            let cycles_per_pattern = benchmark_spec(counter, spec);
            // Round up and clamp to at least 1 so the cost model
            // never returns zero. A zero-cost opcode would let WCET
            // analysis claim free execution, which is unsound.
            let cycles_per_op = libm::ceil(cycles_per_pattern).max(1.0) as u32;
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
pub fn emit_cost_model_source(
    measurements: &[Measurement],
    counter_name: &str,
    counter_frequency_hz: u64,
    assumed_cpu_hz: f64,
) -> String {
    let by_name: BTreeMap<&str, u32> = measurements
        .iter()
        .map(|m| (m.name, m.cycles_per_op))
        .collect();

    // Aggregate by category. The category key matches the structure
    // of `nominal_op_cycles` in `bytecode.rs`. Each category lists
    // the representative spec names that contribute to its bound;
    // the category's reported cycle count is the maximum over those
    // contributors.
    // Categories paired with the representative spec names that
    // contribute to the category bound, plus a nominal-fallback
    // opcode whose `nominal_op_cycles` value is used when no spec
    // produced a measurement for the category. The fallback is
    // conservative: an unmeasured category inherits its WCET
    // estimate from the runtime's bundled nominal table rather
    // than collapsing to one cycle.
    let categories: &[(&str, &[&str], Op)] = &[
        (
            "data_movement",
            &["Const", "PushUnit", "GetLocal", "Pop", "Dup"],
            Op::Const(0),
        ),
        ("control_marker", &[], Op::Yield),
        (
            "arithmetic",
            &[
                "CheckedAdd",
                "CheckedSub",
                "CheckedMul",
                "CheckedNeg",
                "CmpEq",
                "CmpLt",
            ],
            Op::CheckedAdd,
        ),
        ("division", &["Div", "Mod"], Op::Div),
        (
            "composite_construction",
            &["NewArray", "NewTuple"],
            Op::NewArray(0),
        ),
        ("function_call", &[], Op::Call(0, 0)),
    ];

    // First pass: collect measured category costs. Track the
    // measured-to-nominal ratio per measured category so unmeasured
    // categories can be scaled consistently. The bench's measured
    // values are in CPU cycles; the runtime's `nominal_op_cycles`
    // values are relative weights. Mixing both units in the emitted
    // model would be incoherent. Scale unmeasured categories'
    // nominal values by the maximum observed measured-to-nominal
    // ratio across measured categories so the fallback is a
    // conservative CPU-cycle estimate consistent with the measured
    // entries.
    let mut category_costs: BTreeMap<&str, u32> = BTreeMap::new();
    let mut unmeasured: Vec<(&str, &Op)> = Vec::new();
    let mut max_measured_to_nominal: f64 = 0.0;
    for (cat, names, fallback_op) in categories {
        let mut max_cost: u32 = 0;
        for name in *names {
            if let Some(&c) = by_name.get(name)
                && c > max_cost
            {
                max_cost = c;
            }
        }
        if max_cost == 0 {
            // Defer fallback until the scale factor is known.
            unmeasured.push((cat, fallback_op));
        } else {
            // Track the ratio for the scale factor.
            let nominal = keleusma::bytecode::nominal_op_cycles(fallback_op);
            if nominal > 0 {
                let ratio = max_cost as f64 / nominal as f64;
                if ratio > max_measured_to_nominal {
                    max_measured_to_nominal = ratio;
                }
            }
            category_costs.insert(cat, max_cost);
        }
    }
    // Apply scaled-nominal fallback to unmeasured categories. If no
    // measured category exists (the OPCODE_SPECS table is empty), use
    // a unit ratio so the fallback equals the bundled nominal value.
    let scale_factor = if max_measured_to_nominal > 0.0 {
        max_measured_to_nominal
    } else {
        1.0
    };
    for (cat, fallback_op) in unmeasured {
        let nominal = keleusma::bytecode::nominal_op_cycles(fallback_op) as f64;
        let scaled = libm::ceil(nominal * scale_factor).max(1.0) as u32;
        category_costs.insert(cat, scaled);
    }

    let cat = |name: &str| category_costs.get(name).copied().unwrap_or(1);

    let mut out = String::new();
    out.push_str("// Generated by keleusma-bench. Do not edit by hand.\n");
    out.push_str("//\n");
    out.push_str(&format!("// Counter: {}\n", counter_name));
    out.push_str(&format!(
        "// Counter tick frequency: {} Hz ({:.3} MHz)\n",
        counter_frequency_hz,
        counter_frequency_hz as f64 / 1_000_000.0
    ));
    out.push_str(&format!(
        "// Assumed CPU clock:      {:.0} Hz ({:.3} GHz)\n",
        assumed_cpu_hz,
        assumed_cpu_hz / 1_000_000_000.0
    ));
    let scale = if counter_frequency_hz > 0 {
        assumed_cpu_hz / counter_frequency_hz as f64
    } else {
        1.0
    };
    out.push_str(&format!(
        "// Scale (CPU cycles per counter tick): {:.3}\n",
        scale
    ));
    out.push_str("//\n");
    out.push_str("// Per-opcode values below are CPU cycles, computed as\n");
    out.push_str("// `ceil(cycles_per_pattern)` where `cycles_per_pattern` is\n");
    out.push_str("// the minimum observed counter delta across measurement\n");
    out.push_str("// passes, scaled by the counter-to-CPU-cycle ratio above,\n");
    out.push_str("// divided by the number of pattern repetitions in the\n");
    out.push_str("// bench chunk. The per-pattern quantity overstates per-op\n");
    out.push_str("// cost because the pattern carries setup and cleanup ops\n");
    out.push_str("// alongside the target opcode; this is conservative for WCET.\n");
    out.push_str("//\n");
    out.push_str("// Override the assumed CPU clock per host by setting\n");
    out.push_str("// the KELEUSMA_BENCH_CPU_HZ environment variable before\n");
    out.push_str("// running keleusma-bench. The runtime cost model carries\n");
    out.push_str("// no information about CPU clock; consumers that need\n");
    out.push_str("// wall-clock-time bounds divide the reported cycle counts\n");
    out.push_str("// by their measured CPU clock.\n");
    out.push_str("//\n");
    out.push_str("// Per-opcode CPU-cycle measurements:\n");
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
    out.push_str("        | Op::PushImmediate(_)\n");
    out.push_str("        | Op::GetLocal(_)\n");
    out.push_str("        | Op::SetLocal(_)\n");
    out.push_str("        | Op::GetData(_)\n");
    out.push_str("        | Op::SetData(_)\n");
    out.push_str("        | Op::PopN(_)\n");
    out.push_str("        | Op::Dup\n");
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
        "        // Arithmetic, comparison, bitwise, casts ({} cycles).\n",
        ar
    ));
    out.push_str("        Op::Add\n");
    out.push_str("        | Op::Sub\n");
    out.push_str("        | Op::Mul\n");
    out.push_str("        | Op::Neg\n");
    out.push_str("        | Op::CheckedAdd\n");
    out.push_str("        | Op::CheckedSub\n");
    out.push_str("        | Op::CheckedMul(_)\n");
    out.push_str("        | Op::CheckedNeg\n");
    out.push_str("        | Op::CheckedDiv(_)\n");
    out.push_str("        | Op::CheckedMod\n");
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
    out.push_str("        | Op::WordToByte\n");
    out.push_str("        | Op::ByteToWord\n");
    out.push_str("        | Op::WordToFixed(_)\n");
    out.push_str("        | Op::FixedToWord(_)\n");
    out.push_str("        | Op::FixedMul(_)\n");
    out.push_str("        | Op::FixedDiv(_)\n");
    out.push_str("        | Op::BitAnd\n");
    out.push_str("        | Op::BitOr\n");
    out.push_str("        | Op::BitXor\n");
    out.push_str("        | Op::Shl\n");
    out.push_str("        | Op::Shr\n");
    out.push_str("        | Op::BoundsCheck(_)\n");
    out.push_str("        | Op::GetDataIndexed(_, _)\n");
    out.push_str("        | Op::SetDataIndexed(_, _)\n");
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
    out.push_str("        | Op::CallVerifiedNative(_, _)\n");
    out.push_str("        | Op::CallExternalNative(_, _) => ");
    out.push_str(&format!("{},\n", fc));

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
        build: || vec![Op::Const(0), Op::PopN(1)],
        constants: &[ConstValueDescriptor::Int(0)],
        ops_per_pattern: 2,
    },
    OpcodeSpec {
        name: "PushUnit",
        build: || vec![Op::PushImmediate(0), Op::PopN(1)],
        constants: &[],
        ops_per_pattern: 2,
    },
    OpcodeSpec {
        name: "PushTrue",
        build: || vec![Op::PushImmediate(1), Op::PopN(1)],
        constants: &[],
        ops_per_pattern: 2,
    },
    OpcodeSpec {
        name: "GetLocal",
        build: || vec![Op::GetLocal(0), Op::PopN(1)],
        constants: &[],
        ops_per_pattern: 2,
    },
    OpcodeSpec {
        name: "Pop",
        build: || vec![Op::PushImmediate(0), Op::PopN(1)],
        constants: &[],
        ops_per_pattern: 2,
    },
    OpcodeSpec {
        name: "Dup",
        build: || vec![Op::PushImmediate(0), Op::Dup, Op::PopN(1), Op::PopN(1)],
        constants: &[],
        ops_per_pattern: 4,
    },
    // Integer arithmetic in V0.2.0 flows through the `CheckedXxx`
    // opcodes. `Op::Add` / `Op::Sub` / `Op::Mul` / `Op::Neg` were
    // narrowed in Consolidation B to `Byte` / `Fixed` / `Float`
    // operands only; on `Int` operands they trap at runtime. The
    // arithmetic specs below measure the `Int` path through the
    // checked opcodes. Each `CheckedXxx` opcode pushes *three*
    // values (low, high, flag), so the bench pattern uses `PopN(3)`
    // to keep the stack balanced. The compiler's `Int + Int`
    // synthesis is `CheckedAdd; PopN(2)` which discards the carry
    // pair and leaves the low result; the bench discards all three
    // so the per-pattern stack net is zero.
    OpcodeSpec {
        name: "CheckedAdd",
        build: || vec![Op::Const(0), Op::Const(0), Op::CheckedAdd, Op::PopN(3)],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "CheckedSub",
        build: || vec![Op::Const(0), Op::Const(0), Op::CheckedSub, Op::PopN(3)],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "CheckedMul",
        build: || vec![Op::Const(0), Op::Const(0), Op::CheckedMul(0), Op::PopN(3)],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "Div",
        build: || vec![Op::Const(0), Op::Const(0), Op::Div, Op::PopN(1)],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "Mod",
        build: || vec![Op::Const(0), Op::Const(0), Op::Mod, Op::PopN(1)],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "CheckedNeg",
        build: || vec![Op::Const(0), Op::CheckedNeg, Op::PopN(3)],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 3,
    },
    OpcodeSpec {
        name: "CmpEq",
        build: || vec![Op::Const(0), Op::Const(0), Op::CmpEq, Op::PopN(1)],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "CmpLt",
        build: || vec![Op::Const(0), Op::Const(0), Op::CmpLt, Op::PopN(1)],
        constants: &[ConstValueDescriptor::Int(7)],
        ops_per_pattern: 4,
    },
    OpcodeSpec {
        name: "Not",
        build: || vec![Op::PushImmediate(1), Op::Not, Op::PopN(1)],
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
                Op::PopN(1),
            ]
        },
        constants: &[ConstValueDescriptor::Int(0)],
        ops_per_pattern: 5,
    },
    OpcodeSpec {
        name: "NewTuple",
        build: || vec![Op::Const(0), Op::Const(0), Op::NewTuple(2), Op::PopN(1)],
        constants: &[ConstValueDescriptor::Int(0)],
        ops_per_pattern: 4,
    },
    // `Yield` and `Call` are intentionally not in the spec table.
    // `Yield` is rejected by Func chunks; `Call` requires a
    // multi-chunk module that this bench does not construct.
    // Their categories (`control_marker` and `function_call`) fall
    // through the emit logic to the `nominal_op_cycles` table so
    // the generated cost model uses conservative nominal values
    // for those categories rather than a misleadingly optimistic
    // placeholder. Future work may add multi-chunk specs and real
    // Yield measurement through a Stream chunk; until then nominal
    // is the right default for these categories.
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
    #[cfg(feature = "std")]
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
        let source = emit_cost_model_source(&measurements, "test counter", 24_000_000, 3.228e9);
        assert!(source.contains("pub fn measured_op_cycles"));
        assert!(source.contains("MEASURED_COST_MODEL"));
        assert!(source.contains("test counter"));
    }
}
