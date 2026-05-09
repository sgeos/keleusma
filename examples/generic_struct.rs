//! Demonstration that generic structs compile and execute end to end.
//!
//! Generic structs declare type parameters in `<T, U>` form between
//! the struct name and the field block. Field type expressions may
//! reference these parameters. Construction at use sites instantiates
//! the parameters, and field access on a generic struct returns the
//! per-instance instantiated field type.
//!
//! Run with: `cargo run --example generic_struct`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn main() {
    let src = r#"
        struct Cell<T> { value: T }
        fn main() -> i64 {
            let c = Cell { value: 42 };
            c.value
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]) {
        Ok(VmState::Finished(Value::Int(n))) => {
            println!("Cell {{ value: 42 }}.value = {}", n);
            assert_eq!(n, 42);
            println!("generic struct executed end to end");
        }
        other => panic!("unexpected: {:?}", other),
    }
}
