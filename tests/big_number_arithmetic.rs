//! Integration test for the big-number arithmetic worked example.
//!
//! Compiles `examples/scripts/09_big_numbers.kel` and runs it
//! end to end. The expected return value is `1`, indicating that
//! both the `mul_full` and `add_with_carry` patterns produced the
//! expected high-half and carry-out values respectively.

#![cfg(all(feature = "compile", feature = "verify"))]

extern crate alloc;

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};

const SRC_BIG_NUMBERS: &str = include_str!("../examples/scripts/09_big_numbers.kel");

#[test]
fn big_number_example_returns_1() {
    let tokens = tokenize(SRC_BIG_NUMBERS).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    match vm.call(&[]).expect("call") {
        VmState::Finished(v) => assert_eq!(v, Value::Int(1)),
        other => panic!("expected finished, got {:?}", other),
    }
}
