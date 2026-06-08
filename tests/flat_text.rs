#![cfg(all(feature = "compile", feature = "verify"))]
//! End-to-end coverage for flat `Text` fields (B28 P3).
//!
//! A `Text` field of a struct or enum is flat: the two-word
//! `(data_ptr, len)` reference points directly into the arena string
//! bytes, and the epoch is reattached at extraction (the `KString`
//! wrapper). A heap-owned static string literal is copied into the arena
//! at construction and becomes a `KStr`.

extern crate alloc;

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

/// Resolve a returned text value to an owned `String`, accepting either
/// the arena-backed `KStr` form (read from a flat field) or a `StaticStr`.
fn text_of(v: &Value, arena: &Arena) -> alloc::string::String {
    match v {
        Value::KStr(ks) => ks.get(arena).expect("kstr not stale").into(),
        Value::StaticStr(s) => s.clone(),
        other => panic!("expected text, got {:?}", other),
    }
}

#[test]
fn static_text_field_in_flat_struct_round_trips() {
    // The struct field `s: Text` is flat. The literal "hello" is a heap
    // static string; construction copies it into the arena (a `KStr`) and
    // packs its (ptr, len). Reading `w.s` rebuilds the string from those
    // two words against the current epoch.
    let src = "struct W { s: Text, n: Word }\n\
               fn main() -> Text { let w = W { s: \"hello\", n: 7 }; w.s }";
    let tokens = tokenize(src).expect("lex error");
    let program = parse(&tokens).expect("parse error");
    let module = compile(&program).expect("compile error");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    let val = match vm.call(&[]).expect("vm call") {
        VmState::Finished(v) => v,
        other => panic!("expected finished, got {:?}", other),
    };
    assert_eq!(text_of(&val, vm.arena()), "hello");
}

#[test]
fn flat_struct_with_text_and_scalar_reads_following_field() {
    // The two-word Text slot does not disturb the following field's
    // offset: the `Word` field reads back correctly.
    let src = "struct W { s: Text, n: Word }\n\
               fn main() -> Word { let w = W { s: \"abcd\", n: 42 }; w.n }";
    let tokens = tokenize(src).expect("lex error");
    let program = parse(&tokens).expect("parse error");
    let module = compile(&program).expect("compile error");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    let val = match vm.call(&[]).expect("vm call") {
        VmState::Finished(v) => v,
        other => panic!("expected finished, got {:?}", other),
    };
    assert_eq!(val, Value::Int(42));
}
