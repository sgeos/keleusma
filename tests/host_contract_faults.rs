#![cfg(all(feature = "compile", feature = "verify"))]
//! WHAT A HOST SEES WHEN IT BREAKS A STATED CONTRACT.
//!
//! # The question
//!
//! `docs/decisions/INVALID_BYTECODE_CENSUS.md` groups F and J are the
//! host-contract surfaces: a mis-sized data segment, and a native the host did
//! not register. The census judges them lower value than the rest, on the
//! grounds that "a host that supplies a mis-sized buffer or an unregistered
//! native has broken a stated contract", which is true and is not the whole
//! question.
//!
//! **`InvalidBytecode` means *this artefact should never have been produced*.**
//! It is the class `verify()` exists to exclude. When a host's own mistake is
//! reported with it, the message tells the reader to distrust the bytecode,
//! which is the one thing that is not wrong.
//!
//! `docs/process/HANDOFF.md` already records this for ONE site -- a hot-swap
//! path that "reports a fault as `InvalidBytecode` when the artefact was fine"
//! -- and notes that changing which variant a public API returns is a breaking
//! change. This file asks whether that is an isolated site or the shape of both
//! host-contract groups, which is a different claim and needs measuring.
//!
//! # Nothing here is a defect report
//!
//! Every refusal below is CORRECT: the runtime must refuse, and it does, with a
//! message that names the actual mismatch. The observation is about which
//! variant carries it, and that is the operator's call.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError, required_persistent_capacity_for};

/// A module with exactly one private data slot.
const ONE_PRIVATE_SLOT: &str = "private data d { n: Word }\n\
                                loop main(seed: Word) -> Word { \
                                    d.n = seed; \
                                    let _ = yield d.n; \
                                    0 \
                                }";

/// Group F, the reachable host-facing member: a hot swap whose replacement
/// data vector does not match the new module's private slot count.
///
/// **This is the site `HANDOFF.md` already names.** It is pinned here so the
/// observation stops resting on prose, and so that if the variant is ever
/// changed the test says which claim moved.
#[test]
fn a_hot_swap_with_the_wrong_data_length_is_reported_as_invalid_bytecode() {
    let build = || {
        compile(&parse(&tokenize(ONE_PRIVATE_SLOT).expect("lex")).expect("parse")).expect("compile")
    };
    let m = build();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(m, &arena).expect("verify");

    // The module declares ONE private slot; the host supplies three values.
    let wrong: Vec<Value> = alloc::vec![Value::Unit, Value::Unit, Value::Unit];
    let err = vm
        .replace_module(build(), wrong)
        .expect_err("a data vector of the wrong length must be refused");

    assert!(
        alloc::format!("{err:?}").contains("private data segment size mismatch"),
        "the refusal no longer names the mismatch, so a host given this error cannot tell what it \
         got wrong: {err:?}"
    );
    assert!(
        matches!(err, VmError::InvalidBytecode(_)),
        "the variant changed. That may well be an improvement -- the artefact was fine and the \
         HOST was wrong -- but `docs/process/HANDOFF.md` records the current variant as an open \
         API observation, so update it rather than this assertion: {err:?}"
    );
}

/// Group J: a program calling a native the host never registered.
///
/// This is the second instance, and it is what turns the handoff's single
/// observation into a statement about the GROUPS rather than about one site.
#[test]
fn an_unregistered_native_is_reported_as_invalid_bytecode() {
    const USES_A_NATIVE: &str = "use absent() -> Word\nfn main() -> Word { absent() }";
    let m =
        compile(&parse(&tokenize(USES_A_NATIVE).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(m, &arena).expect("verify");
    // Deliberately register nothing.
    let err = vm
        .call(&[])
        .expect_err("calling an unregistered native must be refused");

    let text: String = alloc::format!("{err:?}");
    assert!(
        text.contains("native"),
        "the refusal no longer names the native, so a host cannot tell which one it forgot: {text}"
    );
    assert!(
        matches!(err, VmError::InvalidBytecode(_)),
        "the variant changed for the unregistered-native path. The observation in \
         `docs/process/HANDOFF.md` covers a hot-swap site; if this one has moved, the two are no \
         longer the same shape and the census's group J note needs revising: {text}"
    );
}

/// The CONTROL, and the reason the two tests above say anything.
///
/// If every host mistake produced `InvalidBytecode`, the observation would be
/// vacuous -- a runtime with one error variant cannot be said to choose it
/// wrongly. It does not: a host that supplies the right shapes and then asks
/// the value to be something it is not gets a `TypeError`, and the
/// read-before-resume contract gets one too.
#[test]
fn not_every_host_mistake_is_reported_as_invalid_bytecode() {
    const RETURNS_A_WORD: &str = "fn main() -> Word { 7 }";
    let m =
        compile(&parse(&tokenize(RETURNS_A_WORD).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(m, &arena).expect("verify");
    // Call with an argument the entry point does not take.
    let err = vm.call(&[Value::Int(1)]);
    if let Err(VmError::InvalidBytecode(m)) = err {
        panic!(
            "an argument-count mistake by the HOST is also reported as InvalidBytecode, so the \
             observation above is about the runtime having few variants rather than about these \
             two sites: {m}"
        );
    }
}
