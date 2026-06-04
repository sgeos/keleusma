//! Integration tests for the static marshalling layer.
//!
//! These tests exercise the `#[derive(KeleusmaType)]` macro and the
//! ergonomic `register_fn` and `register_fn_fallible` registration API.
//!
//! Gated on both the `compile` and `verify` features because the
//! suite drives the full pipeline from source through VM execution.
//! With either feature disabled the test file compiles to an empty
//! module.

#![cfg(all(feature = "compile", feature = "verify", feature = "floats"))]

extern crate alloc;

use alloc::string::String;
use alloc::vec;

use keleusma::Arena;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{KeleusmaError, KeleusmaType, Value, VmError};

// -- Derive on structs --

#[derive(KeleusmaType, Debug, Clone, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

// All-`Word` fields, so this struct marshals to the flat byte body (B28).
#[derive(KeleusmaType, Debug, Clone, PartialEq)]
struct Pair {
    a: i64,
    b: i64,
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
    let v: Value = p.clone().into_value();
    let recovered = Point::from_value(&v).unwrap();
    assert_eq!(recovered, p);
}

#[test]
fn derive_flat_struct_roundtrips_through_flat_body() {
    use keleusma::bytecode::StructBody;
    let p = Pair { a: 7, b: 9 };
    let v: Value = p.clone().into_value();
    // An all-Word struct marshals to the flat byte body (B28 P2).
    assert!(matches!(v, Value::Struct(StructBody::Flat(_))));
    let recovered = Pair::from_value(&v).unwrap();
    assert_eq!(recovered, p);
}

#[test]
fn derive_nested_struct_roundtrip() {
    let f = Frame {
        origin: Point { x: 1.0, y: 2.0 },
        width: 100.0,
        height: 50.0,
    };
    let v: Value = f.clone().into_value();
    let recovered = Frame::from_value(&v).unwrap();
    assert_eq!(recovered, f);
}

#[test]
fn derive_struct_wrong_type_name_errors() {
    let bogus = Value::struct_value(
        String::from("Square"),
        vec![
            (String::from("x"), Value::Float(1.0)),
            (String::from("y"), Value::Float(2.0)),
        ],
    );
    let err = Point::from_value(&bogus).unwrap_err();
    match err {
        VmError::TypeError(msg) => assert!(msg.contains("Point")),
        other => panic!("expected TypeError, got {:?}", other),
    }
}

#[test]
fn derive_struct_missing_field_errors() {
    let bogus = Value::struct_value(
        String::from("Point"),
        vec![(String::from("x"), Value::Float(1.0))],
    );
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
    let v: Value = s.clone().into_value();
    let recovered = Status::from_value(&v).unwrap();
    assert_eq!(recovered, s);
}

#[test]
fn derive_enum_tuple_variant_one_field() {
    let s = Status::Active(42);
    let v: Value = s.clone().into_value();
    let recovered = Status::from_value(&v).unwrap();
    assert_eq!(recovered, s);
}

#[test]
fn derive_enum_tuple_variant_two_fields() {
    let s = Status::Pair(7, 2.5);
    let v: Value = s.clone().into_value();
    let recovered = Status::from_value(&v).unwrap();
    assert_eq!(recovered, s);
}

#[test]
fn derive_enum_struct_variant() {
    let s = Status::Range { start: 1, end: 10 };
    let v: Value = s.clone().into_value();
    let recovered = Status::from_value(&v).unwrap();
    assert_eq!(recovered, s);
}

#[test]
fn derive_enum_marshals_flat_per_variant() {
    use keleusma::bytecode::EnumBody;
    // A scalar-payload variant marshals to the flat body (B28 P2),
    // matching the flat access the compiler bakes for it; a float-bearing
    // variant stays boxed. Both directions round-trip.
    let active = Status::Active(42).into_value();
    assert!(matches!(active, Value::Enum(EnumBody::Flat(_))));
    assert_eq!(Status::from_value(&active).unwrap(), Status::Active(42));

    let range = Status::Range { start: 1, end: 10 }.into_value();
    assert!(matches!(range, Value::Enum(EnumBody::Flat(_))));
    assert_eq!(
        Status::from_value(&range).unwrap(),
        Status::Range { start: 1, end: 10 }
    );

    let idle = Status::Idle.into_value();
    assert!(matches!(idle, Value::Enum(EnumBody::Flat(_))));
    assert_eq!(Status::from_value(&idle).unwrap(), Status::Idle);

    let pair = Status::Pair(7, 2.5).into_value();
    assert!(matches!(pair, Value::Enum(EnumBody::Boxed { .. })));
    assert_eq!(Status::from_value(&pair).unwrap(), Status::Pair(7, 2.5));
}

#[test]
fn derive_enum_unknown_variant_errors() {
    let bogus = Value::Enum(keleusma::bytecode::EnumBody::Boxed {
        type_name: String::from("Status"),
        variant: String::from("Unknown"),
        fields: vec![],
    });
    let err = Status::from_value(&bogus).unwrap_err();
    match err {
        VmError::TypeError(msg) => assert!(msg.contains("Unknown")),
        other => panic!("expected TypeError, got {:?}", other),
    }
}

// -- Register_fn end-to-end --

fn build_vm<'arena>(src: &str, arena: &'arena Arena) -> Vm<'static, 'arena> {
    let tokens = tokenize(src).expect("lex error");
    let program = parse(&tokens).expect("parse error");
    let module = compile(&program).expect("compile error");
    Vm::new(module, arena).unwrap()
}

#[test]
fn register_fn_arity_zero() {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::magic_number\nfn main() -> Float { host::magic_number() }",
        &arena,
    );
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
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::double\nfn main() -> Word { host::double(21) }",
        &arena,
    );
    vm.register_fn("host::double", |x: i64| -> i64 { x * 2 });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn register_fn_arity_two() {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::add\nfn main() -> Word { host::add(3, 4) }",
        &arena,
    );
    vm.register_fn("host::add", |a: i64, b: i64| -> i64 { a + b });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => assert_eq!(v, Value::Int(7)),
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn register_fn_arity_four() {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::sum4\nfn main() -> Word { host::sum4(1, 2, 3, 4) }",
        &arena,
    );
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
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::div\nfn main() -> Word { host::div(100, 0) }",
        &arena,
    );
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

// -- B35 P7: native-error `error(code)` construct --

#[test]
fn native_error_arm_binds_code() {
    // A fallible native reports a Word error code; the `error(code)`
    // arm catches it and binds the code.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::risky\n\
         fn main() -> Word { host::risky(0) { ok(v) => v, error(code) => code } }",
        &arena,
    );
    vm.register_fn_fallible("host::risky", |x: i64| -> Result<i64, VmError> {
        if x == 0 {
            Err(VmError::NativeErrorCode {
                code: 42,
                message: String::from("boom"),
            })
        } else {
            Ok(x)
        }
    });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn native_error_ok_path() {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::risky\n\
         fn main() -> Word { host::risky(7) { ok(v) => v, error(code) => code } }",
        &arena,
    );
    vm.register_fn_fallible("host::risky", |x: i64| -> Result<i64, VmError> {
        if x == 0 {
            Err(VmError::NativeErrorCode {
                code: 42,
                message: String::from("boom"),
            })
        } else {
            Ok(x)
        }
    });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => assert_eq!(v, Value::Int(7)),
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn native_error_unhandled_propagates() {
    // With no `error` arm the call is not reified, so the native
    // failure propagates to the host unchanged.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::risky\n\
         fn main() -> Word { host::risky(0) { ok(v) => v } }",
        &arena,
    );
    vm.register_fn_fallible("host::risky", |x: i64| -> Result<i64, VmError> {
        if x == 0 {
            Err(VmError::NativeErrorCode {
                code: 42,
                message: String::from("boom"),
            })
        } else {
            Ok(x)
        }
    });
    match vm.call(&[]).unwrap_err() {
        VmError::NativeErrorCode { code, .. } => assert_eq!(code, 42),
        other => panic!("expected NativeErrorCode, got {:?}", other),
    }
}

#[test]
fn native_error_message_only_reifies_sentinel() {
    // A message-only native error has no code; the construct reifies
    // it to the sentinel -1.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::risky\n\
         fn main() -> Word { host::risky(0) { ok(v) => v, error(code) => code } }",
        &arena,
    );
    vm.register_fn_fallible("host::risky", |_x: i64| -> Result<i64, VmError> {
        Err(VmError::NativeError(String::from("plain message")))
    });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => assert_eq!(v, Value::Int(-1)),
        other => panic!("expected finished, got {:?}", other),
    }
}

// A host error type whose discriminants are the Word error codes the
// script side observes. The derive generates `From<HostErr> for
// VmError` producing a `NativeErrorCode`.
#[derive(KeleusmaError, Debug, Clone, Copy)]
#[allow(dead_code)] // `NotFound` documents the discriminant mapping.
enum HostErr {
    NotFound = 1,
    Forbidden = 3,
}

#[test]
fn keleusma_error_derive_maps_discriminant_to_code() {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::lookup\n\
         fn main() -> Word { host::lookup(0) { ok(v) => v, error(code) => code } }",
        &arena,
    );
    vm.register_fn_fallible("host::lookup", |key: i64| -> Result<i64, VmError> {
        if key == 0 {
            // `HostErr::Forbidden` carries discriminant 3; the derive
            // converts it to `NativeErrorCode { code: 3, .. }`.
            Err(HostErr::Forbidden.into())
        } else {
            Ok(key)
        }
    });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => assert_eq!(v, Value::Int(3)),
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn register_fn_with_derived_struct_arg() {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::magnitude\n\
         struct Point { x: Float, y: Float }\n\
         fn main() -> Float { host::magnitude(Point { x: 3.0, y: 4.0 }) }",
        &arena,
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
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::origin\n\
         struct Point { x: Float, y: Float }\n\
         fn main() -> Float { host::origin().x }",
        &arena,
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
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::need_int\nfn main() -> Word { host::need_int(true) }",
        &arena,
    );
    vm.register_fn("host::need_int", |x: i64| -> i64 { x * 2 });
    let err = vm.call(&[]).unwrap_err();
    match err {
        VmError::TypeError(msg) => assert!(msg.contains("Word")),
        other => panic!("expected TypeError, got {:?}", other),
    }
}
