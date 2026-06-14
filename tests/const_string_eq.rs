#![cfg(all(feature = "compile", feature = "verify"))]
//! String equality compares by content across `StaticStr` and `KStr`
//! (B28 P3 item 4).
//!
//! `Op::CmpEq`/`CmpNe` resolve two string operands through the arena and
//! compare their bytes, rather than the structural `PartialEq` that compares a
//! `KStr` by handle identity. This is the prerequisite for loading a string
//! constant as a rodata `KStr`: once `"a"` is a handle, `"a" == "a"` (two
//! distinct handles) must still be content-true. These tests exercise the
//! cross-representation case now, using a host-returned dynamic `KStr` compared
//! against a `StaticStr` literal, which the old handle-identity comparison got
//! wrong (different variants compared unequal).

extern crate alloc;

use alloc::string::String;
use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};

fn run_eq(src: &str, dyntext: &'static str) -> i64 {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(m, &arena).expect("verify");
    vm.register_fn("host::dyntext", move || -> String { String::from(dyntext) });
    match vm.call(&[Value::Int(0)]).expect("call") {
        VmState::Yielded(Value::Int(n)) => n,
        other => panic!("expected Yielded(Int), got {:?}", other),
    }
}

#[test]
fn kstr_equals_staticstr_by_content() {
    // A host-returned dynamic string equals a static literal of the same
    // content. The old structural comparison saw a `KStr` and a `StaticStr` as
    // different variants and returned false.
    let src = "use host::dyntext() -> Text\n\
               loop main(seed: Word) -> Word { \
                   let _ = yield if host::dyntext() == \"hello\" { 1 } else { 0 }; \
                   0 \
               }";
    assert_eq!(
        run_eq(src, "hello"),
        1,
        "KStr == StaticStr of equal content"
    );
}

#[test]
fn kstr_differs_from_staticstr_by_content() {
    let src = "use host::dyntext() -> Text\n\
               loop main(seed: Word) -> Word { \
                   let _ = yield if host::dyntext() == \"world\" { 1 } else { 0 }; \
                   0 \
               }";
    assert_eq!(
        run_eq(src, "hello"),
        0,
        "KStr != StaticStr of differing content"
    );
}

#[test]
fn kstr_ne_staticstr_by_content() {
    // `!=` is the content negation, not a variant check.
    let src = "use host::dyntext() -> Text\n\
               loop main(seed: Word) -> Word { \
                   let _ = yield if host::dyntext() != \"hello\" { 1 } else { 0 }; \
                   0 \
               }";
    assert_eq!(
        run_eq(src, "hello"),
        0,
        "KStr != StaticStr is false for equal content"
    );
}

#[test]
fn static_literals_equal_by_content() {
    // Regression: two static literals of equal content still compare equal.
    let src = "loop main(seed: Word) -> Word { \
                   let _ = yield if \"a\" == \"a\" { 1 } else { 0 }; \
                   0 \
               }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(m, &arena).expect("verify");
    match vm.call(&[Value::Int(0)]).expect("call") {
        VmState::Yielded(Value::Int(n)) => assert_eq!(n, 1, "\"a\" == \"a\""),
        other => panic!("expected Yielded(Int), got {:?}", other),
    }
}
