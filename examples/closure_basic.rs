//! End-to-end demonstration of closure literals and indirect call.
//!
//! `let f = |x: i64| x + 1;` hoists the closure body to a synthetic
//! top-level chunk and evaluates the expression to a `Value::Func`
//! carrying that chunk's index. `f(5)` resolves to an indirect call:
//! the compiler emits `GetLocal(f)` followed by the explicit
//! arguments and an `Op::CallIndirect(arg_count)`. The runtime pops
//! the args plus the `Func` value and invokes the chunk.
//!
//! Environment capture is not yet supported; closures cannot
//! reference outer-scope variables. See B3 in BACKLOG for the
//! follow-on plan.
//!
//! Run with: `cargo run --example closure_basic`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn main() {
    let src = r#"
        fn main() -> i64 {
            let f = |x: i64| x + 1;
            f(41)
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]) {
        Ok(VmState::Finished(Value::Int(n))) => {
            println!("(|x| x + 1)(41) = {}", n);
            assert_eq!(n, 42);
            println!("closure executed end to end");
        }
        other => panic!("unexpected: {:?}", other),
    }
}
