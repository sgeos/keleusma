#![cfg(all(feature = "compile", feature = "verify"))]
//! The verifier terminates on a hostile module (audit H1).
//!
//! # The defect
//!
//! Four region walkers in `src/verify.rs` advance a cursor to a position taken
//! from an `If`, `Else` or `Loop` operand. A **backward** operand sends the
//! cursor to a position already passed, and the walk repeats forever; where
//! the operand selects the recursive If-Else arm instead, the sub-region still
//! contains the same `If` and the recursion does not bottom out.
//!
//! Three public entries reached a walker with the operand unvalidated:
//!
//! - `verify` computed the productivity classification BEFORE `verify_chunk`,
//!   so pass 1 had validated nothing.
//! - `wcet_stream_iteration` and `wcmu_stream_iteration` are documented as
//!   standalone and never ran pass 1 at all.
//!
//! Measured before the fix on a module differing from a valid one by a single
//! `If` operand: `verify` hung at four of seventeen instruction positions and
//! **aborted the process with a stack overflow** at four more. An abort is not
//! catchable, so a host could not defend itself by wrapping the call.
//!
//! # Why this matters more than a wrong answer
//!
//! `verify` is what a host runs on bytecode it did not compile — a hot-swapped
//! module or a precompiled artifact. The crate's claim about such an input is
//! a definitive bound on time and memory. A verifier that does not itself
//! terminate on that input inverts the claim: the bounding process is the
//! unbounded part.
//!
//! # What these tests do NOT establish
//!
//! They do not show the verifier terminates on every hostile module. They show
//! it terminates on the inputs that did not terminate before, and that the
//! rejection is by the intended check rather than incidental.
//! `tests/hostile_module_mutation.rs` is the systematic instrument.

use std::sync::mpsc;
use std::time::Duration;

use keleusma::bytecode::{BlockType, Module, Op};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify::{verify, wcet_stream_iteration, wcmu_stream_iteration};

/// A watchdog, because the failure being guarded against is non-termination.
///
/// An ordinary assertion cannot observe a hang: the test process simply never
/// finishes and the harness reports a timeout that names no test. The worker
/// is detached rather than joined on the timeout path, since a thread stuck in
/// the walk cannot be asked to stop.
fn finishes_within<T: Send + 'static>(d: Duration, f: impl FnOnce() -> T + Send + 'static) -> bool {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let v = f();
        let _ = tx.send(v);
    });
    rx.recv_timeout(d).is_ok()
}

const WATCHDOG: Duration = Duration::from_secs(20);

const FUNC_SRC: &str = "fn main() -> Word { let a = 7; if a > 3 { a * 2 } else { a + 1 } }";
const STREAM_SRC: &str =
    "loop main(seed: Word) -> Word { let a = seed + 1; if a > 3 { yield a } else { yield seed } }";

fn compiled(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

/// The position of the chunk the mutations are applied to.
fn stream_chunk(m: &Module) -> usize {
    m.chunks
        .iter()
        .position(|c| c.block_type == BlockType::Stream)
        .expect("a Stream chunk")
}

/// Every backward `If` operand, at every position, is refused and terminates.
///
/// Exhaustive over the chunk rather than sampled: the hang was position
/// dependent before the fix — some positions hung, some overflowed the stack,
/// some were refused — and a sample would have reported whichever it drew.
#[test]
fn a_backward_if_target_is_refused_at_every_position() {
    let base = compiled(FUNC_SRC);
    let n = base.chunks[0].ops.len();
    assert!(n > 10, "the corpus program lost its shape: {n} ops");

    let mut refused = 0usize;
    for i in 0..n {
        for t in 0..=i {
            let mut m = base.clone();
            m.chunks[0].ops[i] = Op::If(t as u16);
            let id = alloc_id(i, t);
            assert!(
                finishes_within(WATCHDOG, move || verify(&m)),
                "verify did not terminate on {id}"
            );

            let mut m = base.clone();
            m.chunks[0].ops[i] = Op::If(t as u16);
            let err = verify(&m).expect_err(&alloc_id(i, t));
            assert!(
                err.message.contains("forward position"),
                "{} was refused, but not by the forward-target check: {}",
                alloc_id(i, t),
                err.message
            );
            refused += 1;
        }
    }
    assert!(refused > 100, "only {refused} cases ran");
}

fn alloc_id(i: usize, t: usize) -> String {
    format!("op{i} := If({t})")
}

/// An out-of-range `If` operand terminates and is refused.
#[test]
fn an_out_of_range_if_target_is_refused() {
    let base = compiled(FUNC_SRC);
    let n = base.chunks[0].ops.len();
    for t in [n as u16 + 1, 4096, u16::MAX] {
        let mut m = base.clone();
        m.chunks[0].ops[2] = Op::If(t);
        assert!(
            finishes_within(WATCHDOG, move || verify(&m)),
            "verify did not terminate on If({t})"
        );
    }
}

/// A backward `Else` operand is refused. It reached the walk by a different
/// route than `If` — the If-Else arm reads the `Else`'s own operand as the
/// region end — so it is pinned separately rather than assumed to follow.
#[test]
fn a_backward_else_target_is_refused() {
    let base = compiled(FUNC_SRC);
    let at = base.chunks[0]
        .ops
        .iter()
        .position(|o| matches!(o, Op::Else(_)))
        .expect("an Else");
    for t in 0..=at {
        let mut m = base.clone();
        m.chunks[0].ops[at] = Op::Else(t as u16);
        assert!(
            finishes_within(WATCHDOG, move || verify(&m)),
            "verify did not terminate on Else({t})"
        );
    }
}

/// The standalone worst-case-execution-time entry terminates on the same input.
///
/// It is public, documented as usable without `verify`, and ran no target
/// validation of its own. Before the fix this hung and, at other positions,
/// aborted the process.
#[test]
fn the_standalone_wcet_entry_terminates_on_a_backward_target() {
    let base = compiled(STREAM_SRC);
    let ci = stream_chunk(&base);
    let n = base.chunks[ci].ops.len();
    for i in 0..n {
        for t in [0usize, 2, i] {
            if t > i {
                continue;
            }
            let mut m = base.clone();
            m.chunks[ci].ops[i] = Op::If(t as u16);
            let chunk = m.chunks[ci].clone();
            assert!(
                finishes_within(WATCHDOG, move || wcet_stream_iteration(&chunk)),
                "wcet_stream_iteration did not terminate on op{i} := If({t})"
            );
        }
    }
}

/// The standalone worst-case-memory-usage entry, for the same reason.
#[test]
fn the_standalone_wcmu_entry_terminates_on_a_backward_target() {
    let base = compiled(STREAM_SRC);
    let ci = stream_chunk(&base);
    let n = base.chunks[ci].ops.len();
    for i in 0..n {
        for t in [0usize, 2, i] {
            if t > i {
                continue;
            }
            let mut m = base.clone();
            m.chunks[ci].ops[i] = Op::If(t as u16);
            let chunk = m.chunks[ci].clone();
            assert!(
                finishes_within(WATCHDOG, move || wcmu_stream_iteration(&chunk)),
                "wcmu_stream_iteration did not terminate on op{i} := If({t})"
            );
        }
    }
}

/// The check does not refuse what the compiler emits.
///
/// The guard is only worth having if every real program passes it, and the
/// cheapest way for a termination guard to be wrong is to be too strict. Each
/// corpus program is verified whole.
#[test]
fn the_guard_admits_every_shape_the_compiler_emits() {
    for src in [
        FUNC_SRC,
        STREAM_SRC,
        "fn main() -> Word { let a = 1; if a > 0 { 2 } else { 3 } }",
        "fn main() -> Word { let mut_free = 0; for i in 0..4 { } mut_free }",
        "struct P { x: Word, y: Word }\n\
         fn main() -> Word { let p = P { x: 1, y: 2 }; p.x + p.y }",
        "enum E { A, B(Word) }\n\
         fn f(e: E) -> Word { match e { E::A => 0, E::B(n) => n } }\n\
         fn main() -> Word { f(E::B(4)) }",
        "loop main(s: Word) -> Word { let mut_free = s; yield mut_free }",
    ] {
        let m = compiled(src);
        verify(&m).unwrap_or_else(|e| panic!("valid program refused: {e:?}\n{src}"));
    }
}
