//! An external native can be a closure with the native context.
//!
//! A pykeleusma request. `register_external_native` takes a `fn` pointer, which a
//! host whose natives are values of its own language cannot supply.
#![cfg(all(feature = "compile", feature = "verify"))]

use std::cell::Cell;
use std::rc::Rc;

use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

#[test]
fn a_closure_serves_a_use_external_native() {
    let src = "use external host::bump(Word) -> Word\nfn main() -> Word { host::bump(2) + host::bump(3) }";
    let tokens = keleusma::lexer::tokenize(src).expect("lex");
    let module = keleusma::compiler::compile(&keleusma::parser::parse(&tokens).expect("parse"))
        .expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verifies");
    let calls = Rc::new(Cell::new(0i64));
    let seen = Rc::clone(&calls);
    vm.register_external_native_with_ctx_closure(
        "host::bump",
        move |_ctx, args| {
            seen.set(seen.get() + 1);
            match args {
                [Value::Int(n)] => Ok(Value::Int(n * 10)),
                other => Err(keleusma::vm::VmError::NativeError(format!("{other:?}"))),
            }
        },
        2,
    );
    match vm.call(&[]).expect("runs") {
        VmState::Finished(Value::Int(v)) => assert_eq!(v, 50),
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(
        calls.get(),
        2,
        "the closure, with its captured state, served both calls"
    );
}
