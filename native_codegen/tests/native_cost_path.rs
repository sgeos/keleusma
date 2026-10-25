//! **ORDER 4, WORKSTREAM E: NATIVE CODE EXECUTES THE BYTECODE'S COST PATH.**
//!
//! The verifier's time bound is a sum, over the bytecode path one step can take,
//! of each instruction's nominal cost. `NATIVE_BOUNDS_TRANSFER.md` argued that
//! the lowering preserves control flow one for one, so the native code takes the
//! same path. That was an argument. This file makes it a measurement.
//!
//! The native code is lowered with `LowerOptions::op_meter`, which counts each
//! bytecode instruction as its native code runs. The virtual machine runs the
//! same program with a breakpoint at every instruction, which counts the
//! instructions it executes. For every host step, a start or a resume, the two
//! count vectors must be equal, instruction by instruction.
//!
//! Equal counts make the bytecode cost of a native step equal to the bytecode
//! cost of the virtual machine's step. The second test then checks every traced
//! step against [`keleusma_native::cost::entry_bound`], the bound composed
//! through calls, so the claim a library exports covers what the native code
//! actually executed.
//!
//! # What this does not establish
//!
//! A bytecode cost is not a native time. These tests show that the nominal
//! bound applies to the native path. They say nothing about how long native
//! code takes, which is roadmap open decision 1 and the operator's.
//! The traced costs leave out the per-byte text term, which the bound includes,
//! so the comparison with the bound is conservative in the bound's favour.
mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use keleusma::bytecode::{BlockType, Module, NOMINAL_COST_MODEL, SlotVisibility, Value};
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::coroutine::{self, HostContracts};
use keleusma_native::region;

fn machine() -> TargetMachine {
    Target::initialize_native(&InitializationConfig::default()).unwrap();
    let triple = TargetMachine::get_default_triple();
    Target::from_triple(&triple)
        .unwrap()
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .unwrap()
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    status: u64,
    value: i64,
}
type Start = unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8, *mut u8) -> Outcome;
type Resume = unsafe extern "C" fn(*mut u8, i64) -> Outcome;

/// A frame reservation large enough for every subject here. The size does not
/// affect which instructions run.
const FRAME: u32 = 1 << 16;

/// One program and its inputs.
struct Run<'a> {
    what: &'a str,
    program: Module,
    shared: Vec<u8>,
    first: i64,
    replies: Vec<i64>,
}

/// Per-step instruction counts, the yields, and the cost of each completed
/// `Stream` iteration, from the virtual machine.
struct Trace {
    steps: Vec<Vec<u64>>,
    yields: Vec<i64>,
    iterations: Vec<u64>,
}

fn index_table(program: &Module) -> Vec<usize> {
    let mut base = Vec::with_capacity(program.chunks.len());
    let mut total = 0;
    for c in &program.chunks {
        base.push(total);
        total += c.ops.len();
    }
    base.push(total);
    base
}

fn reference(run: &Run<'_>) -> Trace {
    let program = &run.program;
    let base = index_table(program);
    let total = *base.last().unwrap();
    let need = required_persistent_capacity_for(program);
    let cap = auto_arena_capacity_for(program, &[]).unwrap() + need + (1 << 16);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(program.clone(), &arena).unwrap();
    for (c, chunk) in program.chunks.iter().enumerate() {
        for i in 0..chunk.ops.len() {
            vm.set_breakpoint(c, i);
        }
    }
    let mut shared = run.shared.clone();
    let mut trace = Trace {
        steps: Vec::new(),
        yields: Vec::new(),
        iterations: Vec::new(),
    };
    let mut current = vec![0u64; total];
    let mut iteration = 0u64;
    let mut replies = run.replies.iter();
    let mut last = 0;
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(run.first)])
        .unwrap();
    loop {
        match state {
            VmState::BreakpointHit { chunk, op } => {
                current[base[chunk] + op] += 1;
                iteration += u64::from(NOMINAL_COST_MODEL.cycles(&program.chunks[chunk].ops[op]));
                state = vm.resume_from_breakpoint_with_shared(&mut shared).unwrap();
            }
            VmState::Yielded(Value::Int(v)) => {
                trace
                    .steps
                    .push(std::mem::replace(&mut current, vec![0; total]));
                trace.yields.push(v);
                let Some(&r) = replies.next() else {
                    return trace;
                };
                last = r;
                state = vm.resume_with_shared(&mut shared, Value::Int(r)).unwrap();
            }
            // One iteration ended at its Reset. The native step continues
            // through it without returning, as the shared drivers assume.
            VmState::Reset => {
                trace.iterations.push(std::mem::take(&mut iteration));
                state = vm
                    .resume_with_shared(&mut shared, Value::Int(last))
                    .unwrap();
            }
            VmState::Finished(Value::Int(v)) => {
                trace.steps.push(current);
                trace.yields.push(v);
                return trace;
            }
            other => panic!("{}: {other:?}", run.what),
        }
    }
}

/// Per-step instruction counts from the metered native code, and its yields.
fn native(run: &Run<'_>) -> (Vec<Vec<u64>>, Vec<i64>) {
    let program = &run.program;
    let total = *index_table(program).last().unwrap();
    let tm = machine();
    let ctx = Context::create();
    let module = coroutine::lower_metered(&ctx, program, &tm, FRAME, &HostContracts::default())
        .unwrap_or_else(|e| panic!("{}: {e}", run.what));
    let mut meter = vec![0u64; total.max(1)];
    let engine = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .unwrap();
    let global = module
        .get_global(keleusma_native::OP_METER_SYMBOL)
        .expect("a metered module declares its meter");
    engine.add_global_mapping(&global, meter.as_mut_ptr() as usize);
    let prefix = format!("kel_coroutine_{}", program.entry_point.unwrap());
    let (start, resume) = unsafe {
        (
            engine
                .get_function::<Start>(&format!("{prefix}_start"))
                .unwrap(),
            engine
                .get_function::<Resume>(&format!("{prefix}_resume"))
                .unwrap(),
        )
    };
    let words = |bytes: usize| vec![0u64; bytes.div_ceil(8).max(1)];
    let mut slot = words(coroutine::slot_bytes(FRAME).unwrap() as usize);
    let mut shared = words(run.shared.len());
    unsafe { std::slice::from_raw_parts_mut(shared.as_mut_ptr().cast::<u8>(), run.shared.len()) }
        .copy_from_slice(&run.shared);
    let mut private = words(
        required_persistent_capacity_for(program)
            + region::persistent_supplement_bytes(program) as usize,
    );
    common::install_private_init_bytes(program, unsafe {
        std::slice::from_raw_parts_mut(private.as_mut_ptr().cast::<u8>(), private.len() * 8)
    });
    let mut bodies = words(region::host_arena_supplement_bytes(program) as usize);
    let slot = slot.as_mut_ptr().cast::<u8>();
    let mut steps = Vec::new();
    let mut yields = Vec::new();
    let mut seen = vec![0u64; total];
    let mut take = |meter: &[u64]| {
        let step: Vec<u64> = meter[..total]
            .iter()
            .zip(&seen)
            .map(|(now, before)| now - before)
            .collect();
        seen.copy_from_slice(&meter[..total]);
        step
    };
    let mut outcome = unsafe {
        start.call(
            run.first,
            shared.as_mut_ptr().cast(),
            private.as_mut_ptr().cast(),
            bodies.as_mut_ptr().cast(),
            slot,
        )
    };
    let mut replies = run.replies.iter();
    loop {
        steps.push(take(&meter));
        match outcome.status {
            1 => {
                yields.push(outcome.value);
                let Some(&r) = replies.next() else {
                    break;
                };
                outcome = unsafe { resume.call(slot, r) };
            }
            2 => {
                yields.push(outcome.value);
                break;
            }
            other => panic!("{}: status {other}", run.what),
        }
    }
    (steps, yields)
}

fn step_cost(program: &Module, counts: &[u64]) -> u64 {
    let ops = program.chunks.iter().flat_map(|c| c.ops.iter());
    ops.zip(counts)
        .map(|(op, n)| n * u64::from(NOMINAL_COST_MODEL.cycles(op)))
        .sum()
}

fn shared_offset(program: &Module, name: &str) -> usize {
    let layout = program.data_layout.as_ref().unwrap();
    let index = layout
        .slots
        .iter()
        .filter(|s| s.visibility == SlotVisibility::Shared)
        .position(|s| s.name == name)
        .unwrap();
    layout.shared_layout[index].offset as usize
}

fn lexer_run(text: &[u8], turns: i64) -> Run<'static> {
    let program = common::build(&std::fs::read_to_string("../src/selfhost/kel/lexer.kel").unwrap());
    let mut shared = vec![0u8; program.shared_data_bytes as usize];
    let (len, bytes) = (
        shared_offset(&program, "src.len"),
        shared_offset(&program, "src.bytes"),
    );
    shared[len..len + 8].copy_from_slice(&(text.len() as i64).to_le_bytes());
    shared[bytes..bytes + text.len()].copy_from_slice(text);
    Run {
        what: "the self-hosted lexer",
        program,
        shared,
        first: 0,
        replies: (1..turns).collect(),
    }
}

fn source_run(what: &'static str, src: &str, first: i64, replies: &[i64]) -> Run<'static> {
    let program = common::build(src);
    let shared = vec![0u8; program.shared_data_bytes as usize];
    Run {
        what,
        program,
        shared,
        first,
        replies: replies.to_vec(),
    }
}

fn runs() -> Vec<Run<'static>> {
    vec![
        lexer_run(b"fn add(x: Word, y: Word) -> Word { x + y }", 40),
        lexer_run(b"struct P { a: Word } fn f(p: P) -> Word { p.a >= 3 }", 40),
        source_run(
            "a stream calling a helper twice per iteration",
            "fn plus(a: Word) -> Word { a + 19 } \
             loop main(t: Word) -> Word { let a = plus(t); let r = yield a; yield plus(r + a) }",
            7,
            &[3, 5, 8, 13, 21, 34],
        ),
        source_run(
            "a stream with a bounded loop around its yields",
            "loop main(t: Word) -> Word { for i in 0..3 { yield i + t; } yield t + 100 }",
            2,
            &[1, 2, 3, 4, 5, 6, 7, 8, 9],
        ),
        source_run(
            "a reentrant entry calling a helper in a loop",
            "private data d { s: Word } fn sq(x: Word) -> Word { x * x } \
             yield main(a: Word) -> Word { let b = yield sq(a); \
             for i in 0..4 { d.s = d.s + sq(i + b); } let c = yield d.s; sq(c) }",
            3,
            &[2, 5],
        ),
        source_run(
            "a stream delegating its suspension to a callee",
            "yield helper(a: Word) -> Word { let r = yield a + 1; r + a } \
             loop main(a: Word) -> Word { let v = helper(a + 2); let w = yield v; yield w + a }",
            3,
            &[13, 4, 9, 2, 6, 1],
        ),
    ]
}

#[test]
fn every_native_step_executes_exactly_the_vm_instructions() {
    for run in runs() {
        let trace = reference(&run);
        let (steps, yields) = native(&run);
        assert_eq!(yields, trace.yields, "{}: yields", run.what);
        assert_eq!(steps.len(), trace.steps.len(), "{}: step count", run.what);
        let mut executed = 0u64;
        for (k, (native, vm)) in steps.iter().zip(&trace.steps).enumerate() {
            if native != vm {
                let base = index_table(&run.program);
                let first = native.iter().zip(vm).position(|(a, b)| a != b).unwrap();
                let chunk = base.iter().rposition(|&b| b <= first).unwrap();
                let op = first - base[chunk];
                panic!(
                    "{}: step {k} differs first at chunk {chunk} op {op} {:?}: native {} VM {}",
                    run.what, run.program.chunks[chunk].ops[op], native[first], vm[first]
                );
            }
            executed += vm.iter().sum::<u64>();
        }
        assert!(executed > 0, "{}: nothing was executed", run.what);
        eprintln!(
            "  {:<48} {} steps, {executed} instructions, identical",
            run.what,
            steps.len()
        );
    }
}

#[test]
fn every_traced_step_stays_within_the_composed_claim() {
    let mut established = 0;
    for run in runs() {
        let entry = run.program.entry_point.unwrap();
        let block = run.program.chunks[entry].block_type;
        let claim = keleusma_native::cost::entry_bound(&run.program);
        let trace = reference(&run);
        let costs: Vec<u64> = match block {
            BlockType::Stream => trace.iterations.clone(),
            _ => trace
                .steps
                .iter()
                .map(|s| step_cost(&run.program, s))
                .collect(),
        };
        match claim {
            Ok(bound) => {
                assert!(!costs.is_empty(), "{}: no step was costed", run.what);
                let worst = *costs.iter().max().unwrap();
                assert!(
                    worst <= u64::from(bound),
                    "{}: a step cost {worst}, above the composed claim {bound}",
                    run.what
                );
                established += 1;
                eprintln!(
                    "  {:<48} worst traced {worst} of claim {bound} over {} steps",
                    run.what,
                    costs.len()
                );
            }
            // Two subjects have no claim, each for a reason stated in
            // ORDER_4_COMPLETION.md. A suspending callee's cost spans host
            // steps. The lexer's `intern_id` compares two `Word` call results,
            // which the verifier's text pass treats as an unbounded text
            // comparison, so that callee has no bound of its own.
            Err(why) => {
                eprintln!("  {:<48} no claim: {why}", run.what);
                let expected = if run.what.contains("delegating") {
                    "suspending chunk"
                } else if run.what.contains("lexer") {
                    "unbounded-length string"
                } else {
                    panic!("{}: an unexpected refusal: {why}", run.what)
                };
                assert!(
                    why.contains(expected),
                    "{}: refused for another reason: {why}",
                    run.what
                );
            }
        }
    }
    assert!(
        established >= 3,
        "too few claims were checked: {established}"
    );
}

/// MUST-NOT-FIRE for the claim. Composition must raise the bound above the
/// verifier's shallow figure exactly when a chunk calls another, or a claim
/// that ignored callees would pass the test above whenever a step happened to
/// stay small.
#[test]
fn composition_charges_the_callee_body() {
    let src = "fn heavy(x: Word) -> Word { let a = x * 3 + 1; let b = a * a - x; b * 2 + a } \
               loop main(t: Word) -> Word { yield heavy(t) + heavy(t + 1) }";
    let program = common::build(src);
    let entry = program.entry_point.unwrap();
    let shallow = keleusma::verify::wcet_stream_iteration(&program.chunks[entry]).unwrap();
    let composed = keleusma_native::cost::entry_bound(&program).unwrap();
    let heavy = program
        .chunks
        .iter()
        .position(|c| c.name == "heavy")
        .unwrap();
    let callee = keleusma::verify::wcet_whole_chunk(&program.chunks[heavy]).unwrap();
    assert_eq!(
        composed,
        shallow + 2 * callee,
        "two calls add two callee bodies to the shallow figure"
    );
}

/// **How far the composed claim reaches, counted rather than assumed.**
///
/// Every corpus module with a `Stream` or `Reentrant` entry is asked for its
/// composed claim, and each refusal is grouped by its reason. The figures are
/// printed for the completion record and pinned loosely, so a change that made
/// every claim fail, or the corpus vanish, is noticed.
#[test]
fn the_composed_claim_census_over_the_corpus() {
    use std::collections::BTreeMap;
    let mut established = 0usize;
    let mut refused: BTreeMap<String, usize> = BTreeMap::new();
    let mut entries = 0usize;
    for (_name, program) in common::corpus() {
        let Some(entry) = program.entry_point else {
            continue;
        };
        if !matches!(
            program.chunks[entry].block_type,
            BlockType::Stream | BlockType::Reentrant
        ) {
            continue;
        }
        entries += 1;
        match keleusma_native::cost::entry_bound(&program) {
            Ok(_) => established += 1,
            Err(why) => {
                // Group by the innermost reason, which follows the last colon
                // chain of "has no bound" prefixes.
                let reason = why.rsplit("has no bound: ").next().unwrap_or(&why);
                let reason: String = reason
                    .split(" at instruction")
                    .next()
                    .unwrap_or(reason)
                    .chars()
                    .take(70)
                    .collect();
                *refused.entry(reason).or_default() += 1;
            }
        }
    }
    eprintln!("  corpus entries with a coroutine shape: {entries}");
    eprintln!("  composed claim established: {established}");
    for (why, n) in &refused {
        eprintln!("  refused {n:>3}: {why}");
    }
    assert!(
        entries >= 20,
        "the corpus walk found only {entries} entries"
    );
    assert!(established > 0, "no corpus entry has a composed claim");
}

/// The exported constants carry the same claim, with its status, and the
/// header says what unit the figure is in.
#[test]
fn a_library_exports_the_composed_claim_with_its_status() {
    use keleusma_native::linkage::{self, CostExport};
    let tm = machine();
    let src = "fn plus(a: Word) -> Word { a + 19 } \
               loop main(t: Word) -> Word { let a = plus(t); let r = yield a; yield plus(r + a) }";
    let program = common::build(src);
    let ctx = Context::create();
    let lib = linkage::build(&ctx, &program, &tm, "kel_cost", &HostContracts::default()).unwrap();
    let bound = keleusma_native::cost::entry_bound(&program).unwrap();
    assert_eq!(lib.wcet, CostExport::Established(bound));
    let header = lib.c_header();
    assert!(
        header.contains(&format!("#define KEL_COST_WCET_COST {bound}u")),
        "{header}"
    );
    assert!(
        header.contains("#define KEL_COST_WCET_STATUS 1u"),
        "{header}"
    );
    assert!(header.contains("bytecode cost units"), "{header}");
    assert!(
        lib.exported_symbols()
            .contains(&String::from("kel_cost_wcet_status")),
        "the status is an exported symbol"
    );

    // MUST-NOT-FIRE: a module with no composed claim exports status zero and a
    // zero cost, never a figure that reads as a bound.
    let delegating = common::build(
        "yield helper(a: Word) -> Word { let r = yield a + 1; r + a } \
         loop main(a: Word) -> Word { let v = helper(a + 2); let w = yield v; yield w + a }",
    );
    let ctx = Context::create();
    let lib = linkage::build(
        &ctx,
        &delegating,
        &tm,
        "kel_none",
        &HostContracts::default(),
    )
    .unwrap();
    assert!(
        matches!(lib.wcet, CostExport::Unestablished(_)),
        "{:?}",
        lib.wcet
    );
    let header = lib.c_header();
    assert!(header.contains("#define KEL_NONE_WCET_COST 0u"), "{header}");
    assert!(
        header.contains("#define KEL_NONE_WCET_STATUS 0u"),
        "{header}"
    );
}
