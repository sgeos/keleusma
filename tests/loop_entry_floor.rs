#![cfg(all(feature = "compile", feature = "verify"))]
//! `verify()` floors a loop body's operand-stack pops at the loop's entry
//! height. This was a GAP until 2026-08-23 and is now enforced.
//!
//! # What the gap was
//!
//! Back-edge neutrality compares SHAPES, not identities, so a body that pops a
//! below-entry operand and pushes a same-shape replacement is neutral — and the
//! surviving entry was then constructed in the current iteration while touching
//! none of the five escaping opcodes. Any confinement argument over loop bodies
//! is defeated by that shape, and the composite-region-reuse proof's M6(d) had
//! to record it as an emission invariant rather than an enforced one.
//!
//! **The frame floor did not cover it.** `interp_region`'s pops guarded against
//! an EMPTY abstract stack, and **122 of 245** compiled `Loop` instructions in
//! the shipped corpus carry a non-empty operand stack at entry, so for about
//! half of them the frame floor sits strictly below the loop floor.
//!
//! # Why closing it was safe, and how that was known before it was closed
//!
//! **Zero of 588 loop instances across 23 shipped modules would be rejected.**
//! Measured by instrumenting the typed pass's own per-path abstract
//! interpretation, with the instrument proven to fire on the breaching shape.
//! A linear depth scan gave the same zero and was discarded as exact for only
//! **4** of the 245 loops — the flattering number came from the broken
//! instrument first, which is why the sound one was built.
//!
//! **This narrows what `verify()` accepts**, so it was an operator decision
//! rather than an agent's, and it was taken as one.
//!
//! # What this file pinned BEFORE the change, and why the inversion is recorded
//!
//! It asserted that `verify()` ACCEPTED the breaching shape, with a message
//! telling a future editor that closing the gap moves a proof premise and to
//! tell the proof's owner rather than treat the failure as a routine fix. That
//! is exactly what happened: the change was made deliberately, this test failed
//! as designed, and it was inverted rather than deleted. **A gap pin that is
//! silently removed when the gap closes leaves no record that the guarantee
//! changed.**

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
fn a_loop_body_may_not_consume_from_below_its_entry_height() {
    let err = verify_ops(pop_below_entry())
        .expect_err("the below-entry reach must now be refused at load time");
    assert!(
        err.contains("below") || err.contains("floor") || err.contains("entry"),
        "the refusal must name the loop floor, or a reader cannot tell it from \
         an ordinary underflow: {err}"
    );
}

#[test]
fn the_control_is_still_accepted_so_the_refusal_is_about_the_reach() {
    // The control is what makes the refusal above mean something. Without it,
    // that test would pass on a verifier that rejected EVERY loop of this
    // shape, which is a different and much worse change. A probe whose control
    // was missing is how three earlier measurements in this repository ended up
    // measuring something other than what was intended -- and the first draft
    // of THIS probe was malformed (`EndLoop` targeting the `Loop` rather than
    // the body), which only the control revealed.
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
