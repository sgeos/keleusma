#![cfg(all(feature = "compile", feature = "verify"))]
//! `verify()` does NOT floor a loop body at the loop's entry height, and this
//! pins the gap rather than the absence of one.
//!
//! # The question, and why the gap is worth recording
//!
//! The proof session asks whether a loop body can consume an operand from BELOW
//! its loop's entry height and push a same-shape replacement. Back-edge
//! neutrality would accept that — it compares SHAPES, not identities — and the
//! surviving entry would then have been constructed in the current iteration
//! while touching none of the five escaping opcodes. Their confinement
//! condition cannot see it.
//!
//! **The answer is that `verify()` accepts it.** `interp_region`'s pops guard
//! against an EMPTY abstract stack — the frame floor — not against the enclosing
//! loop's entry height. The two are not the same: measured across the shipped
//! corpus, **122 of 245 compiled `Loop` instructions have a non-empty operand
//! stack at entry**, so the frame floor sits strictly below the loop floor for
//! roughly half of them.
//!
//! # What is NOT claimed here
//!
//! That any compiler output does this. **It does not**: instrumenting the typed
//! pass's own abstract interpretation and running the shipped corpus gave ZERO
//! ops consuming below their innermost enclosing loop's entry height, over 588
//! loop instances in 23 modules, with the instrument proven to fire on the
//! shape below. That instrumentation was temporary and is NOT in the tree, so
//! that figure is a measurement rather than a standing guarantee — unlike
//! `tests/stream_never_returns.rs`, which pins its invariant permanently.
//!
//! Recording the gap is the part that can be pinned cheaply. If someone later
//! makes the verifier floor at loop entry, this test fails and tells them a
//! proof premise changed, which is the outcome worth engineering for.

use keleusma::bytecode::{Module, Op};

fn base() -> Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(
            &keleusma::lexer::tokenize("fn main() -> Word { 0 }").expect("lex"),
        )
        .expect("parse"),
    )
    .expect("compile")
}

fn verify_ops(ops: Vec<Op>) -> Result<(), String> {
    let mut m = base();
    m.chunks[0].ops = ops;
    keleusma::verify::verify(&m).map_err(|e| format!("{e:?}"))
}

/// The shape in question: one scalar below the loop, popped by the body and
/// replaced with a same-shape value built in this iteration.
fn pop_below_entry() -> Vec<Op> {
    vec![
        Op::Const(0),   // 0: depth 0 -> 1, BELOW the loop
        Op::Loop(5),    // 1: entry height 1
        Op::PopN(1),    // 2: consumes the below-entry item
        Op::Const(0),   // 3: same-shape replacement
        Op::EndLoop(2), // 4: back edge targets the body start
        Op::Return,     // 5
    ]
}

/// The same loop, never reaching below its entry height.
fn contained() -> Vec<Op> {
    vec![
        Op::Const(0),
        Op::Loop(5),
        Op::Const(0),
        Op::PopN(1),
        Op::EndLoop(2),
        Op::Return,
    ]
}

#[test]
fn a_loop_body_may_consume_from_below_its_entry_height() {
    assert_eq!(
        verify_ops(pop_below_entry()),
        Ok(()),
        "verify() now rejects a loop body that consumes below its entry height. \
         That is very likely an IMPROVEMENT, but it changes a premise the \
         composite-region-reuse proof reasons over: its confinement condition \
         was written knowing this shape was admissible. Tell the proof's owner \
         before treating this failure as a simple test fix."
    );
}

#[test]
fn the_control_is_accepted_too_so_acceptance_is_not_about_the_loop_shape() {
    // Without this, the test above proves only that `verify()` accepts SOME
    // loop of this form, not that it tolerates the reach below entry. A probe
    // whose control was missing is how three earlier measurements in this
    // repository ended up measuring something other than what was intended --
    // and the first draft of THIS probe was malformed (`EndLoop` targeting the
    // `Loop` rather than the body), which only the control revealed.
    assert_eq!(verify_ops(contained()), Ok(()));
}

#[test]
fn compiled_loops_really_do_carry_a_non_empty_entry_stack() {
    // The frame floor and the loop floor coincide only when the entry stack is
    // empty. If every compiled loop started empty, the underflow guard would
    // incidentally cover the loop floor and the gap above would be unreachable
    // from compiled code. It does not.
    use keleusma::verify::op_depth_effect;

    let src = include_str!("../src/selfhost/kel/wire.kel");
    let module = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
    )
    .expect("compile");

    let mut loops = 0usize;
    let mut non_empty = 0usize;
    for chunk in &module.chunks {
        let mut depth = 0i32;
        for op in &chunk.ops {
            if matches!(op, Op::Loop(_)) {
                loops += 1;
                if depth != 0 {
                    non_empty += 1;
                }
            }
            depth += op_depth_effect(op, chunk).1;
        }
    }

    assert!(loops > 0, "the corpus module must contain loops");
    assert!(
        non_empty > 0,
        "every compiled loop starts with an empty operand stack, which would \
         make the frame-underflow guard cover the loop floor incidentally and \
         the gap above unreachable from compiled code. If this becomes true, \
         the proof's P6(d) can be closed on those grounds instead."
    );
}
