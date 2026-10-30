//! A host-built value of a generic instance is read by its instance layout.
//!
//! # The defect
//!
//! Monomorphization specialized a generic struct only where the program built
//! one. A parameter typed `Box<Word>` in a program that never constructs a
//! `Box` kept the template's name, and the field read was baked from the
//! template's field type, an unresolved type parameter, as an opaque handle.
//! Measured on 2026-10-30, `main(b: Box<Word>)` given a host-built `Box` failed
//! with "expects Word ... got Opaque and Int", and `main(p: Pair<Word, Text>)`
//! returned `OpaqueRef(5)` for `p.a`. A pykeleusma finding. The const-generic
//! enum case in the same finding already had its instance layout on `v0.2.3`.
//!
//! # What these tests do NOT establish
//!
//! They do not cover a generic struct nested in an enum payload or reached only
//! through a native's declared type.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
use keleusma::{Arena, Value};

fn call(source: &str, arg: Value) -> Value {
    let tokens = keleusma::lexer::tokenize(source).expect("lex");
    let module = keleusma::compiler::compile(&keleusma::parser::parse(&tokens).expect("parse"))
        .unwrap_or_else(|e| panic!("{source}\n{}", e.message));
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verifies");
    match vm.call(&[arg]).expect("runs") {
        VmState::Finished(v) => v,
        other => panic!("{source}: {other:?}"),
    }
}

#[test]
fn a_host_built_generic_struct_is_read_by_its_instance_layout() {
    let boxed = Value::struct_value("Box".into(), vec![("v".into(), Value::Int(5))]);
    assert_eq!(
        call(
            "struct Box<T> { v: T }\nfn main(b: Box<Word>) -> Word { b.v + 1 }",
            boxed
        ),
        Value::Int(6)
    );
    let pair = Value::struct_value(
        "Pair".into(),
        vec![
            ("a".into(), Value::Int(5)),
            ("b".into(), Value::StaticStr("x".into())),
        ],
    );
    assert_eq!(
        call(
            "struct Pair<A, B> { a: A, b: B }\nfn main(p: Pair<Word, Text>) -> Word { p.a }",
            pair
        ),
        Value::Int(5)
    );
}

#[test]
fn a_host_built_const_generic_enum_is_read_by_its_instance_layout() {
    let e = Value::enum_value(
        "E".into(),
        "A".into(),
        0,
        vec![Value::array(vec![Value::Int(4), Value::Int(9)])],
    );
    assert_eq!(
        call(
            "enum E<const n: Word> { A([Word; n]), B }\nfn main(e: E<2>) -> Word { match e { E::A(x) => x[1], E::B => 0 } }",
            e
        ),
        Value::Int(9)
    );
}
