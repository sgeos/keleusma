//! Text concatenation verifies and then always faults, and this pins it until `Text<N>` closes it.
//!
//! # The hole
//!
//! `"ab" + "cd"` passes the type checker, passes `Vm::new`, and then faults at runtime with
//! `TypeError("cannot add KStr and KStr")`. V0.2.0 removed the script-side text-composition
//! machinery from the virtual machine, but the type checker still admits `Text + Text`.
//!
//! It is a clean trap rather than anything memory-unsafe. It is still wrong: the
//! conservative-verification stance is that a program the runtime cannot execute is rejected at the
//! SAFE CONSTRUCTOR, not at runtime. A program that verifies and cannot run is exactly the shape
//! `verify()` exists to exclude.
//!
//! # Why this asserts the WRONG behaviour on purpose
//!
//! Recorded as a GAP pin in this tree's convention: it asserts what the tree does today, so that
//! **closing the hole makes it FAIL** and the next person is told to update it rather than
//! discovering the change by accident.
//!
//! The authorized `Text<N>` work restores concatenation with a capacity computed by const
//! arithmetic (`Text<A>` and `Text<B>` yielding `Text<A + B>`), so the feature fills the hole rather
//! than fencing it. See [`docs/decisions/TEXT_CAPACITY_TYPE.md`].
//!
//! # What this does NOT claim
//!
//! Nothing about whether the fault is reachable in a shipped program. A host that never writes
//! `Text + Text` never meets it. The claim is narrower: the surface admits it and the runtime
//! cannot honour it.

#![cfg(all(feature = "compile", feature = "verify"))]

extern crate alloc;

use keleusma::VmError;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm};

#[test]
fn text_concatenation_verifies_and_then_always_faults() {
    let src = "fn main() -> Text { \"ab\" + \"cd\" }";
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse"))
        .expect("the type checker admits `Text + Text`; if this now fails, the hole is closed");

    let arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena)
        .expect("the verifier admits it too; if this now fails, the hole is closed at load time");

    // MUST-FIRE, and it is the whole point. A program that reaches here has verified.
    match vm.call(&[]) {
        Err(VmError::TypeError(msg)) => {
            assert!(
                msg.contains("cannot add"),
                "text concatenation faulted, but not with the addition type error this pins: {msg:?}"
            );
        }
        other => panic!(
            "`\"ab\" + \"cd\"` no longer faults at runtime; it produced {other:?}.\n\
             THAT IS GOOD NEWS AND THIS TEST HAS DONE ITS JOB. Text composition has been restored, \
             presumably by the `Text<N>` work. Replace this pin with one asserting the new \
             behaviour, and check that the type checker and the runtime now agree on the whole \
             text surface rather than only on this expression."
        ),
    }
}
