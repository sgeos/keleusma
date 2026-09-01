//! Text concatenation is refused at the compile boundary rather than faulting at runtime.
//!
//! # The hole this file was opened for, and how it closed
//!
//! `"ab" + "cd"` used to pass the type checker, pass `Vm::new`, and then fault at runtime with
//! `TypeError("cannot add KStr and KStr")`. V0.2.0 removed the script-side text-composition
//! machinery from the virtual machine, but the type checker kept an arm returning `Type::Str` for
//! `Str + Str`, so the surface admitted an expression nothing could execute.
//!
//! **A program the compiler accepts and the verifier passes must run.** That is what `verify()`
//! promises, and an expression that always faults breaks it. The arm now refuses, so the rejection
//! sits at the compile boundary where the conservative-verification stance puts an unsupported
//! construct.
//!
//! # This file was a GAP pin and is now a guard, which is the transition working
//!
//! It previously asserted the WRONG behaviour on purpose, so that closing the hole would make it
//! FAIL and tell the next reader to update it. That is exactly what happened. It now asserts the
//! refusal.
//!
//! # The refusal is temporary by design
//!
//! Bounded composition returns with `Text<N>`: a literal carries its own byte capacity and
//! concatenation composes them by const arithmetic, `Text<A>` and `Text<B>` yielding `Text<A + B>`.
//! **When that lands this test fails again**, and it should -- at that point concatenation must
//! COMPILE AND RUN, and the assertion below becomes the wrong one. See
//! `docs/decisions/TEXT_CAPACITY_TYPE.md`.
//!
//! # What this does NOT claim
//!
//! Nothing about host-registered natives, which remain the supported way to compose text and are
//! untouched. The claim is narrow: the `+` operator on two text values does not type-check.

#![cfg(all(feature = "compile", feature = "verify"))]

extern crate alloc;

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;

#[test]
fn text_concatenation_is_refused_at_compile_time() {
    let src = "fn main() -> Text { \"ab\" + \"cd\" }";
    let program = parse(&tokenize(src).expect("lex")).expect("parse");

    let err = match compile(&program) {
        Err(e) => e,
        Ok(_) => panic!(
            "`\"ab\" + \"cd\"` compiles again.\n\
             If `Text<N>` has landed, THIS IS THE EXPECTED FAILURE and the test has done its job: \
             concatenation must now compile AND RUN, so replace this with a pin asserting the \
             result type and value. If `Text<N>` has NOT landed, the verify-and-fault hole has \
             reopened -- a program the verifier admits and the runtime cannot execute."
        ),
    };

    // The message must name the reason, not merely refuse. A bare "cannot add Text and Text" would
    // read as a type mismatch and send someone looking for the wrong problem.
    assert!(
        err.message.contains("text concatenation is not available"),
        "concatenation was refused, but not with the diagnostic that explains why: {}",
        err.message
    );
    assert!(
        err.message.contains("Text<N>"),
        "the refusal should point at what restores the capability: {}",
        err.message
    );
}
