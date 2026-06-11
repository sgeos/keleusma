#![cfg(all(feature = "compile", feature = "verify"))]
//! `Option::Some` flattens to a flat enum body (B28 P3 item 5 C4).
//!
//! `Option` is the built-in generic enum `None | Some(T)`. Its `Some(T)`
//! payload now flattens to a flat enum `[disc=1][T]` in the arena, the same
//! as any uniformly-flat enum, instead of a global-heap boxed body. `None`
//! stays the scalar `Value::None` (the host contract: native functions
//! return `Value::None` for the none case), so `Option` is hybrid but carries
//! no global-heap allocation. Equality of two `Some` bodies is compiled
//! field-wise so it stays IEEE-correct for a float payload rather than
//! comparing raw bytes.

extern crate alloc;

use keleusma::bytecode::EnumBody;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn run(src: &str) -> Value {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(m, &arena).expect("verify");
    match vm.call(&[]).expect("call") {
        VmState::Finished(v) => v.materialized(&arena),
        other => panic!("expected finished, got {:?}", other),
    }
}

fn run_word(src: &str) -> i64 {
    match run(src) {
        Value::Int(n) => n,
        other => panic!("expected Int, got {:?}", other),
    }
}

#[test]
fn option_some_flattens() {
    let v = run("fn main() -> Option<Word> { Option::Some(7) }");
    assert!(
        matches!(v, Value::Enum(EnumBody::Flat(_))),
        "Option::Some must flatten to a flat enum body, got {:?}",
        v
    );
}

#[test]
fn option_some_match_extracts_payload() {
    // None arm first exercises the relaxed flat-composite compare (a flat
    // `Some` against `Value::None` must be a clean `false`, not a fault).
    let src = "fn main() -> Word { \
                   let o = Option::Some(7); \
                   match o { Option::None => 0, Option::Some(x) => x } \
               }";
    assert_eq!(run_word(src), 7);
}

#[test]
fn option_some_nested_struct_payload() {
    let src = "struct P { x: Word, y: Word }\n\
               fn main() -> Word { \
                   let o = Option::Some(P { x: 3, y: 4 }); \
                   match o { Option::Some(p) => p.x + p.y, Option::None => 0 } \
               }";
    assert_eq!(run_word(src), 7);
}

#[test]
fn option_equality_is_fieldwise() {
    assert_eq!(
        run_word(
            "fn main() -> Word { \
                 let a = Option::Some(7); let b = Option::Some(7); \
                 if a == b { 1 } else { 0 } }"
        ),
        1,
        "Some(7) == Some(7) must be true and must not fault"
    );
    assert_eq!(
        run_word(
            "fn main() -> Word { \
                 let a = Option::Some(7); let b = Option::Some(8); \
                 if a == b { 1 } else { 0 } }"
        ),
        0,
        "Some(7) == Some(8) must be false"
    );
    assert_eq!(
        run_word(
            "fn main() -> Word { \
                 let a = Option::Some(7); let b = Option::Some(8); \
                 if a != b { 1 } else { 0 } }"
        ),
        1,
        "Some(7) != Some(8) must be true"
    );
}
