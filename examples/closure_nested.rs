//! End-to-end demonstration of nested closures.
//!
//! An outer closure constructs an inner closure inside its body. The
//! inner closure captures both an outer-function local and the
//! outer closure's parameter. The hoist pass lifts each closure
//! body into its own synthetic chunk and prepends the captured
//! names as implicit parameters. At invocation, the runtime pushes
//! captured values onto the operand stack as implicit arguments
//! before the explicit ones.
//!
//! Run with: `cargo run --example closure_nested`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn main() {
    let src = r#"
        fn main() -> i64 {
            let base: i64 = 100;
            let outer = |x: i64| {
                let inner = |y: i64| base + x + y;
                inner(3)
            };
            outer(7)
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]) {
        Ok(VmState::Finished(Value::Int(n))) => {
            println!("nested closure: 100 + 7 + 3 = {}", n);
            assert_eq!(n, 110);
            println!("nested closure executed end to end");
        }
        other => panic!("unexpected: {:?}", other),
    }
}
