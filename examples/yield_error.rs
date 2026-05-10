//! End-to-end demonstration of error propagation through yield.
//!
//! The script declares a Result-shaped enum and the yield expression
//! evaluates to a value of that enum's type at each resume. The host
//! calls `Vm::resume(Value::Enum(...Ok...))` for success and
//! `Vm::resume_err(Value::Enum(...Err...))` for failure. Both are
//! routed through the same operand-stack mechanism; `resume_err` is
//! a documentation alias that signals intent. The script handles
//! both cases by pattern matching.
//!
//! Idiomatic patterns for B7. The script may also use script-defined
//! `Option<T>`-shaped enums or any other variant union appropriate
//! to the dialogue surface. The host honors the script's declared
//! type by constructing the corresponding `Value::Enum` payload.
//!
//! WCET note. Error propagation introduces no new runtime mechanism
//! beyond the existing yield/resume cycle. The bytecode does not
//! change. The match-arm dispatch is bounded by the number of arms
//! at compile time, so WCET analysis applies unchanged.
//!
//! Run with: `cargo run --example yield_error`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn main() {
    let src = r#"
        enum Reply { Ok(i64), Err }
        loop main(input: Reply) -> i64 {
            let reply = yield 0;
            match reply {
                Reply::Ok(v) => v,
                Reply::Err => -1,
            }
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");

    let seed = Value::Enum {
        type_name: String::from("Reply"),
        variant: String::from("Ok"),
        fields: alloc_vec(Value::Int(0)),
    };
    match vm.call(&[seed]).expect("call") {
        VmState::Yielded(v) => println!("first yield: {:?}", v),
        other => panic!("expected yield, got {:?}", other),
    }

    println!("simulating successful host reply Ok(42)");
    let success = Value::Enum {
        type_name: String::from("Reply"),
        variant: String::from("Ok"),
        fields: alloc_vec(Value::Int(42)),
    };
    match vm.resume(success).expect("resume ok") {
        VmState::Reset => println!("script reset after Ok arm"),
        other => panic!("expected reset, got {:?}", other),
    }

    println!("starting next iteration");
    let seed2 = Value::Enum {
        type_name: String::from("Reply"),
        variant: String::from("Ok"),
        fields: alloc_vec(Value::Int(0)),
    };
    match vm.resume(seed2).expect("resume seed") {
        VmState::Yielded(_) => println!("yielded (waiting for reply)"),
        other => panic!("expected yield, got {:?}", other),
    }

    println!("simulating failure host reply Err");
    let err = Value::Enum {
        type_name: String::from("Reply"),
        variant: String::from("Err"),
        fields: alloc::vec::Vec::new(),
    };
    match vm.resume_err(err).expect("resume_err") {
        VmState::Reset => println!("script reset after Err arm"),
        other => panic!("expected reset on err, got {:?}", other),
    }

    println!("yield error propagation executed end to end");
}

extern crate alloc;

fn alloc_vec(v: Value) -> alloc::vec::Vec<Value> {
    alloc::vec![v]
}
