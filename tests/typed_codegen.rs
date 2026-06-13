#![cfg(all(feature = "compile", feature = "verify"))]
//! The compiler consumes the type checker's authoritative per-function
//! expression types (B28 P3 item 5).
//!
//! The post-monomorphization type-check pass records each function's resolved
//! expression types into `Program::fn_expr_types`, keyed per function (the
//! mangled specialization name) and per span. The compiler's `infer_expr_type`
//! consults that table first and falls back to its structural inference, so a
//! present entry is the concrete resolved type and an absent one is handled as
//! before. The load-bearing safety property is that two specializations of one
//! generic, which share source spans because monomorphization clones the
//! generic body, are distinct functions with distinct tables, so neither
//! mis-bakes the other's access. These tests exercise that property end to end:
//! a generic carrying a composite is specialized at two scalar widths and both
//! specializations must read back correctly.

extern crate alloc;

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};

fn run_word(src: &str) -> i64 {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let v = {
        let mut vm = Vm::new(m, &arena).expect("verify");
        match vm.call(&[]).expect("call") {
            VmState::Finished(v) => v,
            other => panic!("expected finished, got {:?}", other),
        }
    };
    match v.materialized(&arena) {
        Value::Int(n) => n,
        other => panic!("expected Int, got {:?}", other),
    }
}

#[test]
fn generic_tuple_returning_function_specialized_twice() {
    // A generic returning a tuple, specialized at two widths, with tuple
    // field access on each result.
    let src = "\
        fn dup<T>(x: T) -> (T, T) { (x, x) }\n\
        fn main() -> Word { \
            let p = dup(7); \
            let q = dup(3 as Byte); \
            p.0 + p.1 + (q.0 as Word) + (q.1 as Word) }";
    // 7 + 7 + 3 + 3 = 20.
    assert_eq!(run_word(src), 20);
}

#[test]
fn non_generic_composite_still_correct() {
    // A baseline non-generic composite: the table records its types directly.
    let src = "\
        struct P { x: Word, y: Word }\n\
        fn main() -> Word { let p = P { x: 4, y: 5 }; p.x + p.y }";
    assert_eq!(run_word(src), 9);
}
