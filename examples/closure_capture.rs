//! End-to-end demonstration of closure environment capture.
//!
//! `let n = 10; let f = |x| x + n; f(5)` captures the local `n`
//! at closure creation time. The compiler hoists the body into a
//! synthetic chunk whose parameter list is `(n, x)`. At the
//! construction site, the compiler emits `GetLocal(n)` followed by
//! `Op::MakeClosure(chunk_idx, 1)` to build a `Value::Func` whose
//! `env` carries the captured value of `n`. At invocation,
//! `Op::CallIndirect` extracts the env values and pushes them as
//! additional implicit arguments before the explicit ones.
//!
//! Run with: `cargo run --example closure_capture`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn main() {
    let src = r#"
        fn main() -> i64 {
            let n: i64 = 10;
            let f = |x: i64| x + n;
            f(5)
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]) {
        Ok(VmState::Finished(Value::Int(n))) => {
            println!("(let n = 10; |x| x + n)(5) = {}", n);
            assert_eq!(n, 15);
            println!("closure environment capture executed end to end");
        }
        other => panic!("unexpected: {:?}", other),
    }
}
