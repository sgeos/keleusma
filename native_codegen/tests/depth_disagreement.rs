//! **A BLOCK ENTERED AT TWO OPERAND DEPTHS IS REFUSED, NOT ASSERTED AWAY.**
//!
//! # What this was
//!
//! `lower_chunk` records the operand-stack depth at every branch target and
//! required agreement with `assert_eq!`, whose message read *"the typed verifier
//! guarantees agreement, so this is a lowering bug"*.
//!
//! **That reasoning holds for a VERIFIED module, and `lower_module` is a public
//! entry point that does not require one.** `Vm::new_unchecked` exists because
//! trust-skip is a supported mode, and this package already converted 58 panics
//! on that entry point into refusals for exactly this reason.
//!
//! # Found by construction, because the sweep could not reach it
//!
//! `lowering_robustness.rs` was green throughout. Its mutations change OPCODES;
//! a depth disagreement needs a JUMP TARGET moved to a valid-but-wrong index.
//! Retargeting the `If(6)` of `if t > 0 { 1 } else { 2 }` to `If(7)` makes op 7
//! arrive from the branch edge at depth 0 and from the then-arm's `Else` at depth
//! 1.
//!
//! **A clean guard proves its own reach before it proves the tree.** The sweep
//! now carries a retarget mutation, so the class is covered rather than the
//! instance.
//!
//! # The invariant is unchanged
//!
//! Agreement is still required. Only the failure mode moved, from a panic to a
//! refusal a caller can handle. **Dropping the check would trade a panic for a
//! miscompilation**, which is strictly worse: the depth decides which operand
//! slots a block's code reads.

use keleusma::bytecode::Op;
use keleusma_native::{LowerOptions, module_refusals};

mod common;

/// `if t > 0 { 1 } else { 2 }` compiles to a shape whose `If` target can be
/// moved one instruction later, onto an op the `Else` edge also reaches.
const SUBJECT: &str = "fn main(t: Word) -> Word { if t > 0 { 1 } else { 2 } }\n";

#[test]
fn a_branch_retargeted_to_a_disagreeing_depth_is_refused() {
    let clean = common::build(SUBJECT);
    // **NON-VACUITY.** If the unmodified subject did not lower, the refusal below
    // would say nothing about the retarget.
    assert!(
        module_refusals(&clean, LowerOptions::default()).is_empty(),
        "the unmodified subject must lower, or the refusal proves nothing"
    );

    let mut m = clean.clone();
    let entry = m.entry_point.expect("entry point");
    let ops = &mut m.chunks[entry].ops;
    let retargeted = ops.iter().position(|o| matches!(o, Op::If(_))).map(|i| {
        let Op::If(t) = ops[i] else { unreachable!() };
        let near = t + 1;
        ops[i] = Op::If(near);
        (i, t, near)
    });
    let (at, from, to) = retargeted.expect("the subject contains an If");
    assert!(
        (to as usize) < m.chunks[entry].ops.len(),
        "the new target must be a VALID index — an out-of-range one is refused for \
         being out of range, which is a different defect the sweep already covers"
    );

    let refusals = module_refusals(&m, LowerOptions::default());
    assert!(
        !refusals.is_empty(),
        "retargeting the If at op {at} from {from} to {to} produced no refusal. If it \
         now lowers, two edges reach one block at different operand depths and the \
         emitted code reads whichever slots the last edge happened to leave."
    );
    let why = format!("{refusals:?}");
    assert!(
        why.contains("depth disagreement"),
        "the refusal must name the disagreement, or it could be any other objection \
         to the retarget: {why}"
    );
}

/// **The panic is gone, which a refusal alone does not prove.**
///
/// A refusal and a panic are different observables and this file's whole subject
/// is which one a public entry point produces. Driven through `catch_unwind` so a
/// regression reports as "it panicked again" rather than as an aborted run.
#[test]
fn the_entry_point_does_not_panic_on_the_disagreeing_module() {
    let mut m = common::build(SUBJECT);
    let entry = m.entry_point.expect("entry point");
    let ops = &mut m.chunks[entry].ops;
    if let Some(i) = ops.iter().position(|o| matches!(o, Op::If(_))) {
        let Op::If(t) = ops[i] else { unreachable!() };
        ops[i] = Op::If(t + 1);
    }

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        module_refusals(&m, LowerOptions::default())
    }));
    assert!(
        outcome.is_ok(),
        "`module_refusals` PANICKED on a module with a depth disagreement. It is a \
         public entry point that does not require a verified module, so it must \
         refuse rather than abort the caller."
    );
}

/// **The sweep now reaches this class**, so the next shape like it is found by
/// the instrument rather than by someone thinking to construct it.
#[test]
fn the_robustness_sweep_carries_a_retarget_mutation() {
    let src = std::fs::read_to_string("tests/lowering_robustness.rs")
        .expect("the robustness sweep is readable");
    assert!(
        src.contains("branch retargeted"),
        "`lowering_robustness.rs` no longer generates a branch-retarget mutation. \
         Its opcode mutations cannot reach a depth disagreement — that is why this \
         defect was found by hand — so removing it returns the class to being \
         unreachable by the sweep."
    );
}
