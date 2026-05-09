//! Demonstration that a generic identity function compiles and runs
//! end to end against the existing bytecode runtime.
//!
//! Keleusma's `Value` enum is runtime-tagged. Bytecode operations
//! dispatch on the tag, so a generic chunk that flows a value through
//! unchanged works for any type without compile-time monomorphization.
//! More complex generics that constrain `T` to a specific shape
//! (arithmetic, fields) require monomorphization to enforce the
//! constraint at compile time.
//!
//! Run with: `cargo run --example generic_identity`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn main() {
    let src = r#"
        fn id<T>(x: T) -> T { x }
        fn main() -> i64 { id(42) }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]) {
        Ok(VmState::Finished(Value::Int(n))) => {
            println!("id(42) = {}", n);
            assert_eq!(n, 42);
            println!("generic identity executed end to end");
        }
        other => panic!("unexpected: {:?}", other),
    }
}
