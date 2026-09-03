//! A latent trap: the compiler emits `Op::Len` on an array, and the VM refuses it.
//!
//! # The hazard
//!
//! `Op::Len` on a flat array returns `VmError::InvalidBytecode`, on the stated
//! grounds that array length is a compile-time constant the compiler folds, so
//! "it never emits `Op::Len` on an array". **That comment was false.**
//! `static_for_in_length` has no `Expr::If` arm, so `for x in if c { a } else
//! { b }` falls through to the dynamic path and emits exactly that opcode.
//!
//! **`verify()` accepts the module.** What currently holds the trap shut is the
//! resource-bound check, which refuses the loop for having no statically
//! extractable iteration bound.
//!
//! That refusal is in the SECOND category of this project's
//! conservative-verification taxonomy: provable in principle, analysis not yet
//! implemented. It is liftable, and lifting it is a desirable improvement
//! someone would make with no reason to look at `Op::Len`. On the day the bound
//! extractor learns to see through an `if` expression, this program stops being
//! rejected and starts loading and trapping.
//!
//! # What this test is for
//!
//! It is a RATCHET on that sequence, not a proof of correctness. It pins that
//! the program is refused and WHICH refusal refuses it. If someone lifts the
//! bound extractor without handling `Op::Len`, this test fails and points at
//! the work rather than letting a rejected program become a trapping one.
//!
//! Reported by the `v0.3.0` line, whose native backend refuses `Op::Len`
//! deliberately for this reason. Verified here rather than taken on report:
//! the opcode's presence, the control's absence, `verify()` returning clean,
//! and the refusal's identity were each measured.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm};

const IF_ITERABLE: &str = "fn main() -> Word { \
     let a: [Word; 3] = [1, 2, 3]; let b: [Word; 3] = [4, 5, 6]; \
     for x in if true { a } else { b } { } 0 }";

const PLAIN_ITERABLE: &str = "fn main() -> Word { \
     let a: [Word; 3] = [1, 2, 3]; for x in a { } 0 }";

fn compiled_ops(src: &str) -> String {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    format!(
        "{:?}",
        module.chunks.iter().map(|c| &c.ops).collect::<Vec<_>>()
    )
}

/// The compiler emits `Op::Len` on an array, contradicting the VM's comment.
#[test]
fn the_compiler_does_emit_len_on_an_array() {
    assert!(
        compiled_ops(IF_ITERABLE).contains("Len"),
        "the if-expression iterable no longer emits Op::Len. If the compiler now folds \
         the length, this hazard is closed at the root and this file should record that \
         rather than being deleted quietly."
    );
}

/// The control: an ordinary array iterable does NOT emit it.
///
/// Without this the test above could pass because every program emits `Len`,
/// which would make it evidence of nothing.
#[test]
fn a_plain_array_iterable_does_not_emit_len() {
    assert!(
        !compiled_ops(PLAIN_ITERABLE).contains("Len"),
        "a plain array iterable emits Op::Len, so the if-expression case is not the \
         distinguishing factor and this file's diagnosis is wrong"
    );
}

/// `verify()` accepts the module. The structural verifier is not what stops it.
#[test]
fn the_verifier_accepts_the_module_that_would_trap() {
    let tokens = tokenize(IF_ITERABLE).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    assert!(
        keleusma::verify::verify(&module).is_ok(),
        "verify() now rejects this module. That is an improvement and closes the hazard \
         at the right layer; update this file rather than deleting it."
    );
}

/// The program is refused at load, and the refusal is the LIFTABLE one.
///
/// This is the ratchet. If the loop-bound extractor learns to see through an
/// `if` expression, the refusal disappears and the program loads and traps on
/// `Op::Len`. This test fails at that moment.
#[test]
fn the_only_thing_stopping_it_is_the_liftable_bound_refusal() {
    let tokens = tokenize(IF_ITERABLE).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let err = Vm::new(module, &arena)
        .err()
        .map(|e| format!("{e:?}"))
        .expect(
            "the module now LOADS. If it also runs, the compiler was fixed and this file \
             should say so. If it loads and traps, the trap this file exists to predict \
             has been opened: Op::Len on a flat array must be handled before the bound \
             extractor is allowed to admit this loop.",
        );
    assert!(
        err.contains("iteration bound"),
        "the refusal changed identity. It was the loop-bound extractor, which is liftable; \
         whatever refuses now must be checked to be at least as strong. Got: {err}"
    );
}
