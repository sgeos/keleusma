//! End-to-end demonstration of recursive closures.
//!
//! `let fact = |n| if n <= 1 { 1 } else { n * fact(n - 1) };` binds
//! the factorial function to a closure that recurses by referencing
//! its own let-binding name. The hoist pass detects the
//! self-reference and synthesizes a chunk whose parameter list is
//! `(__self, n)`. The compiler emits `Op::MakeRecursiveClosure` at
//! the construction site. At each invocation through
//! `Op::CallIndirect`, the runtime pushes the closure value itself
//! into the self parameter slot, so references to `fact` inside the
//! body resolve to the closure value and dispatch through indirect
//! call.
//!
//! Run with: `cargo run --example closure_recursive`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn main() {
    let src = r#"
        fn main() -> i64 {
            let fact = |n: i64| if n <= 1 { 1 } else { n * fact(n - 1) };
            fact(5)
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]) {
        Ok(VmState::Finished(Value::Int(n))) => {
            println!("fact(5) = {}", n);
            assert_eq!(n, 120);
            println!("recursive closure executed end to end");
        }
        other => panic!("unexpected: {:?}", other),
    }
}
