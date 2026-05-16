//! Method call dispatch through receiver-style syntax.
//!
//! `n.double()` resolves at compile time to `Doubler::i64::double(n)`
//! based on the receiver's inferred type. The compiler looks up the
//! mangled function name in the function map and emits a regular
//! call with the receiver passed as the first argument.
//!
//! Generic-receiver method calls require monomorphization (B2.4).
//! Concrete receivers resolve directly through the narrow inference
//! pass.
//!
//! Run with: `cargo run --example method_call`

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn main() {
    let src = r#"
        trait Doubler { fn double(x: Word) -> Word; }
        impl Doubler for Word { fn double(x: Word) -> Word { x + x } }
        fn main() -> Word {
            let n: Word = 21;
            n.double()
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]) {
        Ok(VmState::Finished(Value::Int(n))) => {
            println!("21.double() = {}", n);
            assert_eq!(n, 42);
            println!("method call dispatch executed end to end");
        }
        other => panic!("unexpected: {:?}", other),
    }
}
