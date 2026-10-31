//! **ORDER 5, WORKSTREAM G: THE FLAT MACHINE IN NATIVE CODE.**
//!
//! Backlog entry B28 deferred one step of its flat-machine redesign to native
//! code generation, an operand stack of untyped bytes on which no operation
//! dispatches by kind. Native operands are already untagged words, and outside
//! a coroutine every kind is resolved while lowering. Inside a coroutine the
//! emitter keeps a run-time kind beside a value wherever control flow can join
//! different kinds, and dispatches or guards on it.
//!
//! This file measures where that happens over the corpus, with
//! `coroutine::kind_census`, and checks `LowerOptions::static_kinds`. With the
//! option set a coroutine lowers only if no run-time kind is needed. Every
//! corpus coroutine the option admits is driven natively and must agree with
//! the virtual machine, and a coroutine the census calls dynamic must be
//! refused by the option while the default lowering still admits it.
//!
//! # What this does not establish
//!
//! The bytecode instruction set is unchanged. Its arithmetic and comparison
//! opcodes still take their kind from the operand, and only native code is
//! flat. Changing the opcodes is B28's other half, which needs the operator.
//! The census counts facts of the analysis, not instructions of the optimised
//! object, so it states where kinds are needed and not how much code that costs.
mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use keleusma::bytecode::{BlockType, Module, Op, TypeTag, Value};
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::LowerOptions;
use keleusma_native::coroutine::{self, HostContracts};
use keleusma_native::region;
use std::collections::BTreeMap;

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

const FRAME: u32 = 1 << 16;
fn static_kinds() -> LowerOptions {
    LowerOptions {
        static_kinds: true,
        ..LowerOptions::default()
    }
}

/// Corpus modules whose entry is a coroutine.
fn coroutines() -> Vec<(String, Module)> {
    common::corpus()
        .into_iter()
        .filter(|(_, m)| {
            m.entry_point.is_some_and(|e| {
                matches!(
                    m.chunks[e].block_type,
                    BlockType::Stream | BlockType::Reentrant
                )
            })
        })
        .collect()
}

/// A subject this file can drive: a single `Word` parameter and no host
/// natives, whose input a scalar reply sequence fully determines.
fn drivable(m: &Module) -> bool {
    let entry = m.entry_point.unwrap();
    m.chunks[entry].param_types.as_slice() == [TypeTag::Word]
        && !m
            .chunks
            .iter()
            .flat_map(|c| &c.ops)
            .any(|op| matches!(op, Op::CallVerifiedNative(..) | Op::CallExternalNative(..)))
}

fn reference(m: &Module, replies: &[i64]) -> Vec<i64> {
    let need = required_persistent_capacity_for(m);
    let cap = auto_arena_capacity_for(m, &[]).unwrap() + need + (1 << 16);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).unwrap();
    let mut shared = vec![0u8; m.shared_data_bytes as usize];
    let mut vm = Vm::new(m.clone(), &arena).unwrap();
    let mut yields = Vec::new();
    let mut state = vm.call_with_shared(&mut shared, &[Value::Int(replies[0])]);
    let mut last = replies[0];
    let mut rest = replies[1..].iter();
    loop {
        match state {
            Ok(VmState::Yielded(Value::Int(v))) => {
                yields.push(v);
                let Some(&r) = rest.next() else {
                    return yields;
                };
                last = r;
                state = vm.resume_with_shared(&mut shared, Value::Int(r));
            }
            Ok(VmState::Reset) => state = vm.resume_with_shared(&mut shared, Value::Int(last)),
            Ok(VmState::Finished(Value::Int(v))) => {
                yields.push(v);
                return yields;
            }
            other => {
                yields.push(i64::MIN);
                let _ = other;
                return yields;
            }
        }
    }
}

fn native(m: &Module, replies: &[i64]) -> Vec<i64> {
    let tm = machine();
    let ctx = Context::create();
    let module = coroutine::lower_with_options(
        &ctx,
        m,
        &tm,
        FRAME,
        &HostContracts::default(),
        static_kinds(),
    )
    .unwrap();
    let ir = module.print_to_string().to_string();
    assert!(
        !ir.contains("kind_checked"),
        "a static coroutine emitted a run-time kind guard"
    );
    common::force_optimize(&module);
    let engine = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .unwrap();
    let prefix = format!("kel_coroutine_{}", m.entry_point.unwrap());
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
    let mut shared = words(m.shared_data_bytes as usize);
    let mut private = words(
        required_persistent_capacity_for(m) + region::persistent_supplement_bytes(m) as usize,
    );
    common::install_private_init_bytes(m, unsafe {
        std::slice::from_raw_parts_mut(private.as_mut_ptr().cast::<u8>(), private.len() * 8)
    });
    let mut bodies = words(region::host_arena_supplement_bytes(m) as usize);
    let slot = slot.as_mut_ptr().cast::<u8>();
    let mut outcome = unsafe {
        start.call(
            replies[0],
            shared.as_mut_ptr().cast(),
            private.as_mut_ptr().cast(),
            bodies.as_mut_ptr().cast(),
            slot,
        )
    };
    let mut yields = Vec::new();
    let mut rest = replies[1..].iter();
    loop {
        match outcome.status {
            1 => {
                yields.push(outcome.value);
                let Some(&r) = rest.next() else {
                    return yields;
                };
                outcome = unsafe { resume.call(slot, r) };
            }
            2 => {
                yields.push(outcome.value);
                return yields;
            }
            _ => {
                yields.push(i64::MIN);
                return yields;
            }
        }
    }
}

/// **A coroutine that needs a run-time kind, built on purpose.** Since
/// absorption 124 no corpus coroutine needs one, because V0.2.X's widening gave
/// `14_frame_log.kel`'s private slot its type's zero, which was the last `Unit`
/// start. This subject declares a reply contract of an array for a value the
/// program stores in a private tuple slot and reads through a tuple field, so
/// the slot's kinds are the tuple of its initializer and the array of the
/// reply, and the read needs a run-time kind guard.
fn synthetic_dynamic() -> (Module, HostContracts) {
    use keleusma::bytecode::WireShape;
    use keleusma_native::coroutine::{Dialogue, YieldSite};
    let m = common::build(
        "private data st { p: (Word, Word) } \
         yield main(a: Word) -> Word { let p: (Word, Word) = yield a; st.p = p; st.p.0 }",
    );
    let entry = m.entry_point.expect("entry");
    let ip = m.chunks[entry]
        .ops
        .iter()
        .position(|op| matches!(op, Op::Yield))
        .expect("a yield");
    let contracts = HostContracts {
        dialogues: Some(BTreeMap::from([(
            YieldSite {
                chunk: entry as u32,
                instruction: ip as u32,
            },
            Dialogue {
                yielded: WireShape::Scalar { kind: 3 },
                reply: WireShape::Flat { kind: 1, size: 16 },
            },
        )])),
        ..HostContracts::default()
    };
    (m, contracts)
}

#[test]
fn the_kind_census_over_the_corpus() {
    let mut statics = 0;
    let mut dynamics: BTreeMap<String, usize> = BTreeMap::new();
    let mut refused = 0;
    let mut instructions = 0;
    let mut needing = 0;
    let mut ambiguous_locals = 0;
    for (name, m) in coroutines() {
        match coroutine::kind_census(&m, &HostContracts::default()) {
            Ok(c) => {
                instructions += c.instructions;
                needing += c.guarded + c.dynamic_operands;
                ambiguous_locals += c.dynamic_locals;
                match &c.first_dynamic {
                    None => statics += 1,
                    Some(why) => {
                        let class = why.split(" instruction ").nth(1).unwrap_or(why);
                        let class: String =
                            class.split_once(' ').map_or(class, |(_, r)| r).to_string();
                        *dynamics.entry(class).or_default() += 1;
                        eprintln!("  dynamic {name}: {why}");
                    }
                }
            }
            Err(e) => {
                refused += 1;
                eprintln!("  refused {name}: {e}");
            }
        }
    }
    let dynamic: usize = dynamics.values().sum();
    eprintln!("  coroutine entries: static {statics}, dynamic {dynamic}, refused {refused}");
    for (why, n) in &dynamics {
        eprintln!("    {n:>3} first need: {why}");
    }
    eprintln!(
        "  instructions analysed {instructions}, needing a run-time kind {needing}, \
         with an ambiguous but unread local {ambiguous_locals}"
    );
    assert!(statics > 0, "no corpus coroutine is static");
    // **SINCE ABSORPTION 124 THE CORPUS NEEDS NO RUN-TIME KIND.** Question 9's
    // aim, measured: 0 of 192,597 instructions, with 31 static, 0 dynamic and
    // 2 refused coroutines. A dynamic coroutine here is a regression to explain.
    assert_eq!(
        (dynamic, needing),
        (0, 0),
        "a corpus coroutine needs a run-time kind again"
    );
    // **NON-VACUITY.** A census that cannot report a need would pass the
    // assertion above forever, so it must report one on a subject that has it.
    let (m, contracts) = synthetic_dynamic();
    let c = coroutine::kind_census(&m, &contracts).expect("the synthetic subject is analysed");
    assert!(
        c.first_dynamic.is_some() && c.guarded + c.dynamic_operands > 0,
        "the census reports no run-time kind for a subject that needs one"
    );
}

#[test]
fn every_static_corpus_coroutine_lowers_tag_free_and_agrees_with_the_vm() {
    let replies: Vec<i64> = (0..24).map(|i| (i * 7) % 11 - 3).collect();
    let mut driven = 0;
    for (name, m) in coroutines() {
        let Ok(c) = coroutine::kind_census(&m, &HostContracts::default()) else {
            continue;
        };
        if !c.is_static() || !drivable(&m) {
            continue;
        }
        // A static census does not make a module lowerable. One the default
        // route also refuses is reported, not driven.
        if let Err(e) = coroutine::lower(&Context::create(), &m, &machine(), FRAME) {
            eprintln!("  {name}: static but refused by the coroutine route: {e}");
            continue;
        }
        let expected = reference(&m, &replies);
        assert_eq!(native(&m, &replies), expected, "{name}");
        driven += 1;
        eprintln!("  {name}: {} yields agree", expected.len());
    }
    assert!(driven >= 3, "too few static subjects were driven: {driven}");
}

#[test]
fn a_dynamic_coroutine_is_refused_only_when_static_kinds_are_requested() {
    // The corpus has no dynamic coroutine since absorption 124, so the subject
    // is the synthetic one, and the default route must admit it first.
    let tm = machine();
    let name = "synthetic_dynamic";
    let (m, contracts) = synthetic_dynamic();
    assert!(
        coroutine::kind_census(&m, &contracts).is_ok_and(|c| !c.is_static()),
        "the synthetic subject must be dynamic"
    );
    coroutine::lower_with_options(
        &Context::create(),
        &m,
        &tm,
        FRAME,
        &contracts,
        LowerOptions::default(),
    )
    .expect("the default lowering admits the dynamic subject");
    let error = coroutine::lower_with_options(
        &Context::create(),
        &m,
        &tm,
        FRAME,
        &contracts,
        static_kinds(),
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("static kinds requested"),
        "{name}: {error}"
    );
    eprintln!("  {name}: {error}");
}

/// The census counts only operands an instruction reads. Each `piano_roll`
/// coroutine holds a native result of unknown kind that only `PopN` and the
/// soft-failure `Trap` pop, so all ten are static and the option admits them on
/// the coroutine route. `interp.kel` used to move an operand of more than one
/// kind into a local at chunk 2 instruction 51, where the wrapping default of a
/// covered checked addition joined its arms. Since absorption 116 both
/// compilers omit that unreachable default, so `interp.kel` is static too and
/// the option admits it.
///
/// The piano roll calls host natives, so this file cannot drive it. Its native
/// agreement with the virtual machine is checked whole by
/// `module_differential.rs` through `lower_module`, not through this route.
#[test]
fn an_operand_no_instruction_reads_needs_no_run_time_kind() {
    let tm = machine();
    let mut piano = 0;
    let mut interp = false;
    for (name, m) in coroutines() {
        let Ok(c) = coroutine::kind_census(&m, &HostContracts::default()) else {
            continue;
        };
        if name.starts_with("piano_roll_") {
            assert!(c.is_static(), "{name}: {:?}", c.first_dynamic);
            coroutine::lower_with_options(
                &Context::create(),
                &m,
                &tm,
                FRAME,
                &HostContracts::default(),
                static_kinds(),
            )
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            piano += 1;
        } else if name == "interp.kel" {
            assert!(c.is_static(), "{name}: {:?}", c.first_dynamic);
            coroutine::lower_with_options(
                &Context::create(),
                &m,
                &tm,
                FRAME,
                &HostContracts::default(),
                static_kinds(),
            )
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            interp = true;
        }
    }
    assert_eq!(piano, 10, "the piano roll family was not reached whole");
    assert!(interp, "interp.kel was not reached");
}
