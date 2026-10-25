//! **ORDER 4, WORKSTREAM F: A NATIVE TRAP IS AN OUTCOME THE HOST RECEIVES.**
//!
//! The virtual machine returns an unhandled partial operation to its host as a
//! recoverable error naming the trap. Before Order 4 the same operation in a
//! native coroutine reached `llvm.trap` and ended the host process, so a host
//! linking a Keleusma library could not survive a trap it would have survived
//! under the virtual machine.
//!
//! A coroutine's stable handles now report a trap as status three, with the
//! `TrapKind` code as the value, and leave the instance inactive. This file
//! drives one subject per first-pass trap kind through both executions and
//! requires the same yields and the same kind. It also places the trap in an
//! ordinary callee, two callees deep, in a delegated callee and in a stream, so
//! the propagation path is exercised and not only the entry's own trap block.
//!
//! # What this does not establish
//!
//! The payload of a trap is not reported. An indexing trap's index and bound
//! and a host failure's code stay inside the instance. A function lowered
//! outside a coroutine still aborts through `llvm.trap`, which
//! `partial_operation_census.rs` shows by reading the emitted code.
mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use keleusma::bytecode::{TrapKind, Value};
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
type Release = unsafe extern "C" fn(*mut u8);

/// One trap subject, with replies that reach the trap and replies that do not.
struct Subject {
    what: &'static str,
    src: &'static str,
    first: i64,
    trapping: &'static [i64],
    completing: &'static [i64],
    kind: TrapKind,
    vm_error: &'static str,
}

const SUBJECTS: &[Subject] = &[
    Subject {
        what: "division by zero in the entry",
        src: "yield main(a: Word) -> Word { let d = yield a; 100 / d }",
        first: 7,
        trapping: &[0],
        completing: &[4],
        kind: TrapKind::ZeroDivisor,
        vm_error: "DivisionByZero",
    },
    Subject {
        what: "remainder by zero in the entry",
        src: "yield main(a: Word) -> Word { let d = yield a; 100 % d }",
        first: 7,
        trapping: &[0],
        completing: &[7],
        kind: TrapKind::ZeroDivisor,
        vm_error: "DivisionByZero",
    },
    Subject {
        what: "an index out of bounds in the entry",
        src: "yield main(a: Word) -> Word { let xs = [4, 5, 6]; let i = yield a; xs[i] }",
        first: 1,
        trapping: &[3],
        completing: &[2],
        kind: TrapKind::IndexOutOfBounds,
        vm_error: "IndexOutOfBounds",
    },
    Subject {
        what: "a negative index in an ordinary callee",
        src: "fn pick(i: Word) -> Word { let xs = [4, 5, 6]; xs[i] } \
              yield main(a: Word) -> Word { let i = yield a; pick(i) + 1 }",
        first: 1,
        trapping: &[-1],
        completing: &[0],
        kind: TrapKind::IndexOutOfBounds,
        vm_error: "IndexOutOfBounds",
    },
    Subject {
        what: "division two ordinary callees deep",
        src: "fn inner(d: Word) -> Word { 100 / d } \
              fn outer(d: Word) -> Word { inner(d) + 1 } \
              yield main(a: Word) -> Word { let d = yield a; outer(d) * 2 }",
        first: 3,
        trapping: &[0],
        completing: &[5],
        kind: TrapKind::ZeroDivisor,
        vm_error: "DivisionByZero",
    },
    Subject {
        what: "division in a stream after a full iteration",
        src: "loop main(a: Word) -> Word { let d = yield a; yield 100 / d }",
        first: 7,
        trapping: &[5, 1, 0],
        completing: &[5, 1, 4, 2],
        kind: TrapKind::ZeroDivisor,
        vm_error: "DivisionByZero",
    },
    Subject {
        what: "division in a delegated callee",
        src: "yield helper(a: Word) -> Word { let r = yield a + 1; 10 / r } \
              loop main(a: Word) -> Word { let v = helper(a); yield v }",
        first: 2,
        trapping: &[0],
        completing: &[5, 9],
        kind: TrapKind::ZeroDivisor,
        vm_error: "DivisionByZero",
    },
    Subject {
        what: "a loop limit reached before the range ends",
        src: "private data d { n: Word } \
              fn count(n: Word) -> Word { for i in 0..n limit 4 { d.n = d.n + i; } d.n } \
              yield main(a: Word) -> Word { let n = yield a; count(n) }",
        first: 1,
        trapping: &[9],
        completing: &[3],
        kind: TrapKind::LoopLimitExceeded,
        vm_error: "LoopLimitExceeded",
    },
    Subject {
        what: "a refinement predicate failing in a callee",
        src: "fn in_range(x: Word) -> bool { x >= 0 and x <= 100 } \
              newtype Percent = Word where in_range; \
              fn admit(r: Word) -> Word { let p = Percent(r); r + 1 } \
              yield main(a: Word) -> Word { let r = yield a; admit(r) }",
        first: 1,
        trapping: &[500],
        completing: &[50],
        kind: TrapKind::RefinementFailed,
        vm_error: "RefinementFailed",
    },
    Subject {
        what: "a discriminant matching no variant",
        src: "enum Status { Ok = 0, Pending = 1 } \
              fn conv(c: Word) -> Word { let s = c as Status { ok(v) => v }; \
              match s { Status::Ok => 10, Status::Pending => 11 } } \
              yield main(a: Word) -> Word { let c = yield a; conv(c) }",
        first: 1,
        trapping: &[99],
        completing: &[1],
        kind: TrapKind::EnumVariantUnmapped,
        vm_error: "EnumVariantUnmapped",
    },
];

/// The virtual machine's yields and its error, or `None` when it completed.
fn reference(src: &str, first: i64, replies: &[i64]) -> (Vec<i64>, Option<String>) {
    let m = common::build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity") + need + (64 << 10);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent region");
    let mut shared = vec![0u8; m.shared_data_bytes as usize];
    let mut vm = Vm::new(m, &arena).expect("vm");
    let mut yields = Vec::new();
    let mut state = vm.call_with_shared(&mut shared, &[Value::Int(first)]);
    let mut replies = replies.iter();
    let mut last = 0;
    loop {
        match state {
            Ok(VmState::Yielded(Value::Int(v))) => {
                yields.push(v);
                let Some(&r) = replies.next() else {
                    return (yields, None);
                };
                last = r;
                state = vm.resume_with_shared(&mut shared, Value::Int(r));
            }
            // The native driver collapses the rewind, as the shared drivers do,
            // so the rewind is offered the reply of the suspension it follows.
            Ok(VmState::Reset) => {
                state = vm.resume_with_shared(&mut shared, Value::Int(last));
            }
            Ok(VmState::Finished(_)) => return (yields, None),
            Err(e) => return (yields, Some(format!("{e:?}"))),
            Ok(other) => panic!("{src}: unexpected state {other:?}"),
        }
    }
}

/// The native yields and the outcome that ended the run.
///
/// The run ends at the first outcome that is not a yield, or when the replies
/// run out. It then requires a trapped instance to be inactive and a second
/// start in the same slot to run normally, so a trap leaves no state behind.
fn native(src: &str, first: i64, replies: &[i64], optimise: bool) -> (Vec<i64>, Outcome) {
    let program = common::build(src);
    let tm = machine();
    let bound = coroutine::minimum_frame_bytes(&program, &tm, &HostContracts::default())
        .unwrap_or_else(|e| panic!("{src}: {e}"));
    let ctx = Context::create();
    let module = coroutine::lower(&ctx, &program, &tm, bound).unwrap();
    if optimise {
        common::force_optimize(&module);
    }
    let engine = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .unwrap();
    let prefix = format!("kel_coroutine_{}", program.entry_point.unwrap());
    let (start, resume, release) = unsafe {
        (
            engine
                .get_function::<Start>(&format!("{prefix}_start"))
                .unwrap(),
            engine
                .get_function::<Resume>(&format!("{prefix}_resume"))
                .unwrap(),
            engine
                .get_function::<Release>(&format!("{prefix}_release"))
                .unwrap(),
        )
    };
    let words = |bytes: usize| vec![0u64; bytes.div_ceil(8).max(1)];
    let mut slot = words(coroutine::slot_bytes(bound).unwrap() as usize);
    let mut shared = words(program.shared_data_bytes as usize);
    let mut private = words(
        required_persistent_capacity_for(&program)
            + region::persistent_supplement_bytes(&program) as usize,
    );
    let mut bodies = words(region::host_arena_supplement_bytes(&program) as usize);
    let pointers = |s: &mut Vec<u64>, p: &mut Vec<u64>, b: &mut Vec<u64>| {
        (
            s.as_mut_ptr().cast::<u8>(),
            p.as_mut_ptr().cast::<u8>(),
            b.as_mut_ptr().cast::<u8>(),
        )
    };
    let begin = |slot: &mut Vec<u64>, s: &mut Vec<u64>, p: &mut Vec<u64>, b: &mut Vec<u64>| {
        p.fill(0);
        common::install_private_init_bytes(&program, unsafe {
            std::slice::from_raw_parts_mut(p.as_mut_ptr().cast::<u8>(), p.len() * 8)
        });
        let (sp, pp, bp) = pointers(s, p, b);
        unsafe { start.call(first, sp, pp, bp, slot.as_mut_ptr().cast()) }
    };
    let mut yields = Vec::new();
    let mut outcome = begin(&mut slot, &mut shared, &mut private, &mut bodies);
    let mut pending = replies.iter();
    while outcome.status == 1 {
        yields.push(outcome.value);
        let Some(&r) = pending.next() else {
            break;
        };
        outcome = unsafe { resume.call(slot.as_mut_ptr().cast(), r) };
    }
    if outcome.status == 3 {
        let slot_ptr = slot.as_mut_ptr().cast();
        assert_eq!(
            unsafe { resume.call(slot_ptr, 0) },
            Outcome {
                status: 0,
                value: 0
            },
            "{src}: a trapped instance must be inactive"
        );
        unsafe { release.call(slot_ptr) };
        let again = begin(&mut slot, &mut shared, &mut private, &mut bodies);
        assert_eq!(
            again,
            Outcome {
                status: 1,
                value: yields[0]
            },
            "{src}: a slot that trapped must start again normally"
        );
    }
    unsafe { release.call(slot.as_mut_ptr().cast()) };
    (yields, outcome)
}

#[test]
fn every_first_pass_trap_kind_reaches_the_host_as_an_outcome() {
    for s in SUBJECTS {
        let (vm_yields, vm_error) = reference(s.src, s.first, s.trapping);
        let error = vm_error.unwrap_or_else(|| panic!("{}: the VM did not trap", s.what));
        assert!(
            error.contains(s.vm_error),
            "{}: the VM reported {error}, not {}",
            s.what,
            s.vm_error
        );
        for optimise in [false, true] {
            let (yields, outcome) = native(s.src, s.first, s.trapping, optimise);
            assert_eq!(yields, vm_yields, "{} (optimised {optimise})", s.what);
            assert_eq!(
                outcome,
                Outcome {
                    status: 3,
                    value: i64::from(s.kind.code())
                },
                "{} (optimised {optimise}): the VM reported {error}",
                s.what
            );
        }
        eprintln!(
            "  {:<44} VM {error:<40} native status 3, code {}",
            s.what,
            s.kind.code()
        );
    }
}

/// MUST-NOT-FIRE for the test above. The same programs on replies that avoid
/// the trap must complete or keep yielding exactly as the virtual machine
/// does. Without this, a lowering that reported every instance as trapped would
/// pass the test above.
#[test]
fn the_same_subjects_without_a_failing_input_do_not_trap() {
    for s in SUBJECTS {
        let (vm_yields, vm_error) = reference(s.src, s.first, s.completing);
        assert_eq!(
            vm_error, None,
            "{}: the control input trapped in the VM",
            s.what
        );
        for optimise in [false, true] {
            let (yields, outcome) = native(s.src, s.first, s.completing, optimise);
            assert_ne!(outcome.status, 3, "{} (optimised {optimise})", s.what);
            assert_eq!(yields, vm_yields, "{} (optimised {optimise})", s.what);
        }
    }
}

/// A completed Reentrant entry reports status two with its value, which the
/// trap record must not disturb. The value is checked against the virtual
/// machine's own completion.
#[test]
fn a_completion_after_the_trap_change_still_reports_its_value() {
    let src = "yield main(a: Word) -> Word { let d = yield a; 100 / d }";
    let m = common::build(src);
    let arena = keleusma_arena::Arena::with_capacity(1 << 16);
    let mut vm = Vm::new(m, &arena).unwrap();
    assert!(matches!(
        vm.call(&[Value::Int(7)]).unwrap(),
        VmState::Yielded(Value::Int(7))
    ));
    let VmState::Finished(Value::Int(expected)) = vm.resume(Value::Int(4)).unwrap() else {
        panic!("the VM did not complete");
    };
    for optimise in [false, true] {
        let (_, outcome) = native(src, 7, &[4], optimise);
        assert_eq!(
            outcome,
            Outcome {
                status: 2,
                value: expected
            }
        );
    }
}
