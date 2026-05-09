//! Probe whether pattern matching on a generic enum and nested
//! generic structs work end to end.
//!
//! Run with: `cargo run --example generic_match`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn run(label: &str, src: &str) -> Value {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]) {
        Ok(VmState::Finished(v)) => {
            println!("{}: {:?}", label, v);
            v
        }
        other => panic!("{}: {:?}", label, other),
    }
}

fn main() {
    let r1 = run(
        "pattern match on generic enum",
        r#"
            enum Maybe<T> { Just(T), Nothing }
            fn main() -> i64 {
                let m = Maybe::Just(42);
                match m {
                    Maybe::Just(x) => x,
                    Maybe::Nothing => 0,
                }
            }
        "#,
    );
    assert_eq!(r1, Value::Int(42));

    let r2 = run(
        "nested generic structs",
        r#"
            struct Cell<T> { value: T }
            struct Wrap<T> { inner: Cell<T> }
            fn main() -> i64 {
                let w = Wrap { inner: Cell { value: 7 } };
                w.inner.value
            }
        "#,
    );
    assert_eq!(r2, Value::Int(7));

    println!("all generic_match probes passed");
}
