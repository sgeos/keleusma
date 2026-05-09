//! End-to-end demonstration of passing a closure as a function
//! argument and invoking it from inside the receiving function.
//!
//! `fn apply<F>(f: F, x: i64) -> i64 { f(x) }` accepts a generic
//! parameter `F` whose runtime value is a `Value::Func`. The body
//! invokes `f(x)` which the compiler resolves as an indirect call
//! through `Op::CallIndirect`. Monomorphization specializes `apply`
//! per concrete `F`, but since closures all share the `Func` runtime
//! representation, a single specialization handles every closure
//! instantiation.
//!
//! Run with: `cargo run --example closure_as_arg`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn main() {
    let src = r#"
        fn apply<F>(f: F, x: i64) -> i64 { f(x) }
        fn main() -> i64 {
            let g = |x: i64| x + 1;
            apply(g, 41)
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]) {
        Ok(VmState::Finished(Value::Int(n))) => {
            println!("apply(g, 41) = {}", n);
            assert_eq!(n, 42);
            println!("first-class closure as function argument executed end to end");
        }
        other => panic!("unexpected: {:?}", other),
    }
}
