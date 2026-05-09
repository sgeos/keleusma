//! End-to-end demonstration of method dispatch on a generic struct
//! field. `c.value.double()` where `c: Cell<i64>` and the struct's
//! field type is the generic parameter `T`.
//!
//! Without struct monomorphization, `c.value` would have an opaque
//! type and the method dispatch on `.double()` would fail. The
//! generic struct specialization pass clones `Cell` to `Cell__i64`
//! with the field type substituted, so `c.value` resolves to `i64`
//! and the trait method dispatches to `Doubler::i64::double`.
//!
//! Run with: `cargo run --example struct_method_dispatch`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn main() {
    let src = r#"
        trait Doubler { fn double(x: i64) -> i64; }
        impl Doubler for i64 { fn double(x: i64) -> i64 { x + x } }
        struct Cell<T> { value: T }
        fn main() -> i64 {
            let c = Cell { value: 21 };
            c.value.double()
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]) {
        Ok(VmState::Finished(Value::Int(n))) => {
            println!("Cell {{ value: 21 }}.value.double() = {}", n);
            assert_eq!(n, 42);
            println!("generic struct method dispatch executed end to end");
        }
        other => panic!("unexpected: {:?}", other),
    }
}
