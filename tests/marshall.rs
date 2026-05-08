//! Integration tests for the static marshalling layer.
//!
//! These tests exercise the `#[derive(KeleusmaType)]` macro and the
//! ergonomic `register_fn` and `register_fn_fallible` registration API.

extern crate alloc;

use alloc::string::String;
use alloc::vec;

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{Vm, VmState};
use keleusma::{KeleusmaType, Value, VmError};

// -- Derive on structs --

#[derive(KeleusmaType, Debug, Clone, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(KeleusmaType, Debug, Clone, PartialEq)]
struct Frame {
    origin: Point,
    width: f64,
    height: f64,
}

#[test]
fn derive_struct_roundtrip() {
    let p = Point { x: 3.0, y: 4.0 };
    let v = p.clone().into_value();
    let recovered = Point::from_value(&v).unwrap();
    assert_eq!(recovered, p);
}

#[test]
fn derive_nested_struct_roundtrip() {
    let f = Frame {
        origin: Point { x: 1.0, y: 2.0 },
        width: 100.0,
        height: 50.0,
    };
    let v = f.clone().into_value();
    let recovered = Frame::from_value(&v).unwrap();
    assert_eq!(recovered, f);
}

#[test]
fn derive_struct_wrong_type_name_errors() {
    let bogus = Value::Struct {
        type_name: String::from("Square"),
        fields: vec![
            (String::from("x"), Value::Float(1.0)),
            (String::from("y"), Value::Float(2.0)),
        ],
    };
    let err = Point::from_value(&bogus).unwrap_err();
    match err {
        VmError::TypeError(msg) => assert!(msg.contains("Point")),
        other => panic!("expected TypeError, got {:?}", other),
    }
}

#[test]
fn derive_struct_missing_field_errors() {
    let bogus = Value::Struct {
        type_name: String::from("Point"),
        fields: vec![(String::from("x"), Value::Float(1.0))],
    };
    let err = Point::from_value(&bogus).unwrap_err();
    match err {
        VmError::TypeError(msg) => assert!(msg.contains("y")),
        other => panic!("expected TypeError, got {:?}", other),
    }
}

// -- Derive on enums --

#[derive(KeleusmaType, Debug, Clone, PartialEq)]
enum Status {
    Idle,
    Active(i64),
    Pair(i64, f64),
    Range { start: i64, end: i64 },
}

#[test]
fn derive_enum_unit_variant() {
    let s = Status::Idle;
    let v = s.clone().into_value();
    let recovered = Status::from_value(&v).unwrap();
    assert_eq!(recovered, s);
}

#[test]
fn derive_enum_tuple_variant_one_field() {
    let s = Status::Active(42);
    let v = s.clone().into_value();
    let recovered = Status::from_value(&v).unwrap();
    assert_eq!(recovered, s);
}

#[test]
fn derive_enum_tuple_variant_two_fields() {
    let s = Status::Pair(7, 2.5);
    let v = s.clone().into_value();
    let recovered = Status::from_value(&v).unwrap();
    assert_eq!(recovered, s);
}

#[test]
fn derive_enum_struct_variant() {
    let s = Status::Range { start: 1, end: 10 };
    let v = s.clone().into_value();
    let recovered = Status::from_value(&v).unwrap();
    assert_eq!(recovered, s);
}

#[test]
fn derive_enum_unknown_variant_errors() {
    let bogus = Value::Enum {
        type_name: String::from("Status"),
        variant: String::from("Unknown"),
        fields: vec![],
    };
    let err = Status::from_value(&bogus).unwrap_err();
    match err {
        VmError::TypeError(msg) => assert!(msg.contains("Unknown")),
        other => panic!("expected TypeError, got {:?}", other),
    }
}

// -- Register_fn end-to-end --

fn build_vm(src: &str) -> Vm {
    let tokens = tokenize(src).expect("lex error");
    let program = parse(&tokens).expect("parse error");
    let module = compile(&program).expect("compile error");
    Vm::new(module).unwrap()
}

#[test]
fn register_fn_arity_zero() {
    let mut vm = build_vm("use host::magic_number\nfn main() -> f64 { host::magic_number() }");
    vm.register_fn("host::magic_number", || -> f64 { 42.5 });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => match v {
            Value::Float(f) => assert!((f - 42.5).abs() < 1e-6),
            other => panic!("expected Float, got {:?}", other),
        },
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn register_fn_arity_one() {
    let mut vm = build_vm("use host::double\nfn main() -> i64 { host::double(21) }");
    vm.register_fn("host::double", |x: i64| -> i64 { x * 2 });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn register_fn_arity_two() {
    let mut vm = build_vm("use host::add\nfn main() -> i64 { host::add(3, 4) }");
    vm.register_fn("host::add", |a: i64, b: i64| -> i64 { a + b });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => assert_eq!(v, Value::Int(7)),
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn register_fn_arity_four() {
    let mut vm = build_vm("use host::sum4\nfn main() -> i64 { host::sum4(1, 2, 3, 4) }");
    vm.register_fn("host::sum4", |a: i64, b: i64, c: i64, d: i64| -> i64 {
        a + b + c + d
    });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => assert_eq!(v, Value::Int(10)),
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn register_fn_fallible_propagates_error() {
    let mut vm = build_vm("use host::div\nfn main() -> i64 { host::div(100, 0) }");
    vm.register_fn_fallible("host::div", |a: i64, b: i64| -> Result<i64, VmError> {
        if b == 0 {
            Err(VmError::DivisionByZero)
        } else {
            Ok(a / b)
        }
    });
    let err = vm.call(&[]).unwrap_err();
    match err {
        VmError::DivisionByZero => {}
        other => panic!("expected DivisionByZero, got {:?}", other),
    }
}

#[test]
fn register_fn_with_derived_struct_arg() {
    let mut vm = build_vm(
        "use host::magnitude\n\
         struct Point { x: f64, y: f64 }\n\
         fn main() -> f64 { host::magnitude(Point { x: 3.0, y: 4.0 }) }",
    );
    vm.register_fn("host::magnitude", |p: Point| -> f64 {
        libm::sqrt(p.x * p.x + p.y * p.y)
    });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => match v {
            Value::Float(f) => assert!((f - 5.0).abs() < 1e-9),
            other => panic!("expected Float, got {:?}", other),
        },
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn register_fn_with_derived_struct_return() {
    let mut vm = build_vm(
        "use host::origin\n\
         struct Point { x: f64, y: f64 }\n\
         fn main() -> f64 { host::origin().x }",
    );
    vm.register_fn("host::origin", || -> Point { Point { x: 0.0, y: 0.0 } });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => match v {
            Value::Float(f) => assert!(f.abs() < 1e-9),
            other => panic!("expected Float, got {:?}", other),
        },
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn register_fn_argument_type_mismatch() {
    let mut vm = build_vm("use host::need_int\nfn main() -> i64 { host::need_int(true) }");
    vm.register_fn("host::need_int", |x: i64| -> i64 { x * 2 });
    let err = vm.call(&[]).unwrap_err();
    match err {
        VmError::TypeError(msg) => assert!(msg.contains("i64")),
        other => panic!("expected TypeError, got {:?}", other),
    }
}
