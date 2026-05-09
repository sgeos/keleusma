//! End-to-end demonstration that monomorphization makes trait method
//! calls inside generic function bodies resolve.
//!
//! Without monomorphization, `x.double()` inside
//! `fn use_doubler<T: Doubler>(x: T) -> i64` cannot resolve at
//! compile time because `x`'s type is the abstract type parameter
//! `T`. The monomorphization pass walks the call graph from `main`,
//! infers `T = i64` from the call `use_doubler(21)`, and generates
//! `use_doubler__i64` as a specialization with `T` replaced by `i64`
//! throughout. Inside the specialization, `x: i64` is concrete and
//! `x.double()` resolves to `Doubler::i64::double`.
//!
//! Run with: `cargo run --example monomorphize_generic_method`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn main() {
    let src = r#"
        trait Doubler { fn double(x: i64) -> i64; }
        impl Doubler for i64 { fn double(x: i64) -> i64 { x + x } }
        fn use_doubler<T: Doubler>(x: T) -> i64 { x.double() }
        fn main() -> i64 { use_doubler(21) }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]) {
        Ok(VmState::Finished(Value::Int(n))) => {
            println!("use_doubler(21) = {}", n);
            assert_eq!(n, 42);
            println!("monomorphization-driven method dispatch executed end to end");
        }
        other => panic!("unexpected: {:?}", other),
    }
}
