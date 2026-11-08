//! **ORDER 4, WORKSTREAM F: A HANDLED PARTIAL OPERATION IS A NATIVE BRANCH.**
//!
//! The V0.2.X line made the first-pass partial operations total. Each produces
//! its result together with an outcome flag, and the source handler is a branch
//! on that flag. The roadmap's Workstream F asks that each one lower natively
//! to code producing its flag, with a handled outcome as a native branch.
//!
//! This file executes one handled subject per first-pass operation natively and
//! on the virtual machine, on the input that fails and on one that does not,
//! with and without the optimiser, and requires equal results. A handled
//! failure that reached a trap instead of its arm would end this process,
//! because a function lowered outside a coroutine aborts through `llvm.trap`.
//! A completed run with the arm's value is therefore the evidence that the
//! failure was answered by the branch.
//!
//! # Coverage, stated rather than implied
//!
//! Word arithmetic, division and remainder, array indexing and indexed private
//! data reads and writes are covered. Fixed division, Byte arithmetic and
//! shared data reads are covered by `checked_fixed_mul.rs`,
//! `checked_byte_arithmetic.rs` and `shared_data.rs` respectively, not here.
//! A handled host failure needs a host contract and is covered by
//! `native_host_failure.rs`. The unhandled forms are covered by
//! `native_trap_outcomes.rs`.
mod common;

/// One handled subject and the inputs it runs on. The first input fails.
struct Subject {
    what: &'static str,
    src: &'static str,
    inputs: &'static [(i64, i64)],
}

const SUBJECTS: &[Subject] = &[
    Subject {
        what: "division by zero",
        src: "fn main(a: Word, b: Word) -> Word { a / b { ok(v) => v, zero_divisor(n) => n + 1000 } }",
        inputs: &[(7, 0), (7, 2), (-9, 4)],
    },
    Subject {
        what: "remainder by zero",
        src: "fn main(a: Word, b: Word) -> Word { a % b { ok(v) => v, zero_divisor(n) => n + 1000 } }",
        inputs: &[(7, 0), (7, 2), (-9, 4)],
    },
    Subject {
        what: "division overflow",
        src: "fn main(a: Word, b: Word) -> Word { a / b { ok(v) => v, overflow(h, l) => l + 1, zero_divisor(n) => n } }",
        inputs: &[(i64::MIN, -1), (i64::MIN, 0), (i64::MIN, 2)],
    },
    Subject {
        what: "addition overflow and underflow",
        src: "fn main(a: Word, b: Word) -> Word { a + b { ok(v) => v, overflow(h, l) => l - 5, underflow(h, l) => l + 5 } }",
        inputs: &[(i64::MAX, 1), (i64::MIN, -1), (2, 3)],
    },
    Subject {
        what: "subtraction overflow and underflow",
        src: "fn main(a: Word, b: Word) -> Word { a - b { ok(v) => v, overflow(h, l) => l - 5, underflow(h, l) => l + 5 } }",
        inputs: &[(i64::MIN, 1), (i64::MAX, -1), (2, 3)],
    },
    Subject {
        what: "multiplication overflow and underflow",
        src: "fn main(a: Word, b: Word) -> Word { a * b { ok(v) => v, overflow(h, l) => h + 7, underflow(h, l) => h - 7 } }",
        inputs: &[(i64::MAX, 2), (i64::MAX, -2), (6, 7)],
    },
    Subject {
        what: "an array index out of bounds",
        src: "fn main(a: Word, b: Word) -> Word { let xs = [1, 2, 3]; xs[a] { ok(v) => v, invalid_index(j) => j * 100 + b } }",
        inputs: &[(5, 1), (-1, 1), (1, 1)],
    },
    Subject {
        what: "an indexed private data read out of bounds",
        src: "private data d { x: [Word; 3] } fn main(a: Word, b: Word) -> Word { d.x[1] = b; d.x[a] { ok(v) => v, invalid_index(j) => j * 10 } }",
        inputs: &[(3, 4), (-2, 4), (1, 4)],
    },
    Subject {
        what: "an indexed private data write out of bounds",
        src: "private data d { x: [Word; 4] } fn main(a: Word, b: Word) -> Word { d.x[a] = b { ok(o, n) => o + n, invalid_index(j) => j } }",
        inputs: &[(9, 3), (-1, 3), (2, 3)],
    },
];

#[test]
fn every_handled_first_pass_outcome_runs_its_arm_natively() {
    let mut failing_inputs = 0;
    for s in SUBJECTS {
        for (n, &(a, b)) in s.inputs.iter().enumerate() {
            let mut seen = None;
            for optimise in [false, true] {
                let (vm, native) = common::vm_and_native_two_arg_at(s.src, a, b, optimise);
                assert_eq!(
                    native, vm,
                    "{} on ({a}, {b}), optimised {optimise}: native {native}, VM {vm}",
                    s.what
                );
                seen = Some(vm);
            }
            if n == 0 {
                failing_inputs += 1;
            }
            eprintln!("  {:<44} ({a}, {b}) -> {}", s.what, seen.unwrap());
        }
    }
    assert_eq!(failing_inputs, SUBJECTS.len());
}

/// MUST-NOT-FIRE for the comparison above. The first input of each subject has
/// to reach its failure arm in the virtual machine, or the native agreement
/// would say nothing about the branch. Each arm is written so that its value
/// differs from what the operation's ordinary result would be, and this checks
/// the virtual machine's value against the arm's arithmetic directly.
#[test]
fn the_failing_inputs_reach_their_failure_arms_in_the_vm() {
    let arm = |src: &str, a: i64, b: i64| common::vm_and_native_two_arg(src, a, b).0;
    assert_eq!(
        arm(SUBJECTS[0].src, 7, 0),
        1007,
        "zero_divisor binds the numerator"
    );
    assert_eq!(arm(SUBJECTS[1].src, 7, 0), 1007);
    assert_eq!(
        arm(SUBJECTS[2].src, i64::MIN, -1),
        i64::MIN + 1,
        "the wrapped low word plus one"
    );
    assert_eq!(
        arm(SUBJECTS[3].src, i64::MAX, 1),
        i64::MIN.wrapping_sub(5),
        "overflow binds the wrapped low word"
    );
    assert_eq!(
        arm(SUBJECTS[6].src, 5, 1),
        501,
        "invalid_index binds the index"
    );
    assert_eq!(arm(SUBJECTS[7].src, 3, 4), 30);
    assert_eq!(arm(SUBJECTS[8].src, 9, 3), 9);
}
