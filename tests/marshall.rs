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
use keleusma::{KeleusmaError, KeleusmaType, RefContext, Value, VmError};

/// Build the canonical arena-flat body of a host value through the exact path
/// the VM applies to a host argument or native result (B28 item 2 step 6B): the
/// arena-less `into_value` is boxed, and `into_arena_canonical` re-packs it flat
/// at the bundled 64-bit runtime widths. These flat-marshalling tests build the
/// flat body the way the runtime does, since the flat representation now
/// requires an arena.
fn flatten_in_arena<T: KeleusmaType<i64, f64>>(v: T, arena: &Arena) -> Value {
    v.into_value()
        .into_arena_canonical(8, 8, arena)
        .expect("canonicalise host value into arena")
}

/// A `RefContext` at the bundled 64-bit runtime widths for decoding an
/// arena-resident flat body through `from_value_ctx`.
fn bundled_ctx(arena: &Arena) -> RefContext<'_> {
    RefContext {
        arena,
        opaques: &[],
        word_bytes: 8,
        float_bytes: 8,
        ref_epoch: arena.epoch(),
    }
}

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

// Nested flat composites (B28 P2 nested inlining): `Pair` is an all-Word
// flat struct and `(i64, i64)` a flat tuple, so `Holder` inlines both into
// one flat byte body.
#[derive(KeleusmaType, Debug, Clone, PartialEq)]
struct Holder {
    p: Pair,
    coords: (i64, i64),
    tag: i64,
}

// A uniformly-flat enum (every variant's payload is flat), so it is padded
// to one fixed body size and may be nested as a flat field (B28 P2).
#[derive(KeleusmaType, Debug, Clone, PartialEq)]
enum Signal {
    Off,
    On(i64),
    Span { lo: i64, hi: i64 },
}

// A flat struct nesting a uniformly-flat enum field (B28 P2).
#[derive(KeleusmaType, Debug, Clone, PartialEq)]
struct Carrier {
    sig: Signal,
    n: i64,
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
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let p = Pair { a: 7, b: 9 };
    // An all-Word struct reaches the flat byte body through the arena: the
    // arena-less `into_value` is boxed and `into_arena_canonical` re-packs it
    // flat, the path the VM applies to a host value (B28 item 2 step 6B).
    let v = flatten_in_arena(p.clone(), &arena);
    assert!(matches!(v, Value::Struct(StructBody::Flat(_))));
    let recovered = Pair::from_value_ctx(&v, &bundled_ctx(&arena)).unwrap();
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
fn derive_nested_flat_struct_and_tuple_roundtrip() {
    use keleusma::bytecode::StructBody;
    // A struct whose fields are themselves flat composites (a flat struct
    // and a flat tuple) inlines them into one flat byte body and reads them
    // back, recursing through the nested layout (B28 P2). Before nested
    // inlining this round-tripped via the boxed path; it must still hold.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let h = Holder {
        p: Pair { a: 11, b: 22 },
        coords: (3, 4),
        tag: 99,
    };
    // `into_arena_canonical` recurses bottom-up, flattening the nested struct
    // and tuple first so the parent packs them into one flat body (B28 item 2
    // step 6B).
    let v = flatten_in_arena(h.clone(), &arena);
    assert!(matches!(v, Value::Struct(StructBody::Flat(_))));
    let recovered = Holder::from_value_ctx(&v, &bundled_ctx(&arena)).unwrap();
    assert_eq!(recovered, h);
}

#[test]
fn derive_uniform_flat_enum_pads_and_roundtrips() {
    use keleusma::bytecode::EnumBody;
    // Every variant of a uniformly-flat enum marshals to a flat body of one
    // fixed size (padded to the largest variant), so nesting it is sound and
    // padding-tolerant equality preserves round-trips (B28 P2).
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    for s in [Signal::Off, Signal::On(7), Signal::Span { lo: 1, hi: 9 }] {
        let v = flatten_in_arena(s.clone(), &arena);
        assert!(matches!(v, Value::Enum(EnumBody::Flat(_))));
        assert_eq!(Signal::from_value_ctx(&v, &bundled_ctx(&arena)).unwrap(), s);
    }
}

#[test]
fn derive_struct_nesting_flat_enum_roundtrips() {
    use keleusma::bytecode::StructBody;
    // A flat struct nesting a uniformly-flat enum field inlines the enum's
    // fixed-size body and reads it back; the host-built slot size matches
    // what the compiler bakes for a script (B28 P2).
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    for sig in [Signal::Off, Signal::On(5), Signal::Span { lo: 2, hi: 8 }] {
        let c = Carrier {
            sig: sig.clone(),
            n: 42,
        };
        // The nested enum is padded to its largest-variant size (the
        // `min_payload` re-flattening hint) so `n` sits at a stable offset
        // regardless of the variant (B28 item 2 step 6B).
        let v = flatten_in_arena(c.clone(), &arena);
        assert!(matches!(v, Value::Struct(StructBody::Flat(_))));
        assert_eq!(
            Carrier::from_value_ctx(&v, &bundled_ctx(&arena)).unwrap(),
            c
        );
    }
}

#[test]
fn derive_struct_wrong_type_name_errors() {
    // A float struct is flat (B28 P3 item 5), and a flat body carries no type
    // name to validate; the type-name check is a property of the boxed decode,
    // so this exercises a host-built boxed struct (a host may pass either
    // representation, and `from_value` accepts both).
    let bogus = Value::Struct(keleusma::bytecode::StructBody::boxed(
        String::from("Square"),
        vec![
            (String::from("x"), Value::Float(1.0)),
            (String::from("y"), Value::Float(2.0)),
        ],
    ));
    let err = Point::from_value(&bogus).unwrap_err();
    match err {
        VmError::TypeError(msg) => assert!(msg.contains("Point")),
        other => panic!("expected TypeError, got {:?}", other),
    }
}

#[test]
fn derive_struct_missing_field_errors() {
    // A missing field is detectable in the boxed decode, which addresses
    // fields by name; the flat body is positional bytes with no field names,
    // so this exercises a host-built boxed struct (B28 P3 item 5).
    let bogus = Value::Struct(keleusma::bytecode::StructBody::boxed(
        String::from("Point"),
        vec![(String::from("x"), Value::Float(1.0))],
    ));
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
    // Every variant's payload is flat-eligible (Float included, B28 P3 item
    // 5), so the enum is uniformly flat and each variant marshals to the flat
    // body, matching the flat access the compiler bakes. A float-bearing
    // variant is flat too and is compared field-wise. Both directions
    // round-trip.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let ctx = bundled_ctx(&arena);

    let active = flatten_in_arena(Status::Active(42), &arena);
    assert!(matches!(active, Value::Enum(EnumBody::Flat(_))));
    assert_eq!(
        Status::from_value_ctx(&active, &ctx).unwrap(),
        Status::Active(42)
    );

    let range = flatten_in_arena(Status::Range { start: 1, end: 10 }, &arena);
    assert!(matches!(range, Value::Enum(EnumBody::Flat(_))));
    assert_eq!(
        Status::from_value_ctx(&range, &ctx).unwrap(),
        Status::Range { start: 1, end: 10 }
    );

    let idle = flatten_in_arena(Status::Idle, &arena);
    assert!(matches!(idle, Value::Enum(EnumBody::Flat(_))));
    assert_eq!(Status::from_value_ctx(&idle, &ctx).unwrap(), Status::Idle);

    let pair = flatten_in_arena(Status::Pair(7, 2.5), &arena);
    assert!(matches!(pair, Value::Enum(EnumBody::Flat(_))));
    assert_eq!(
        Status::from_value_ctx(&pair, &ctx).unwrap(),
        Status::Pair(7, 2.5)
    );
}

#[test]
fn derive_enum_unknown_variant_errors() {
    let bogus = Value::Enum(keleusma::bytecode::EnumBody::boxed(
        String::from("Status"),
        String::from("Unknown"),
        vec![],
    ));
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
    // The native return is field-accessed, so the compiler needs the declared
    // return type to bake the flat field access that matches the flat struct
    // body `into_value` now produces (B28 P3 item 5: a float struct is flat).
    // Without the signature the access bakes the boxed by-name form and
    // mismatches the flat body.
    let mut vm = build_vm(
        "use host::origin() -> Point\n\
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
fn register_fn_returning_word_struct_builds_in_arena() {
    // A native returning an all-`Word` flat struct builds its body directly in
    // the arena through the derived `into_value_ctx` at the module widths (B28
    // P3 item 2, Increment 3; B36), rather than a global-heap `Inline` migrated
    // afterward. The script reads both fields, proving the arena-resident
    // result round-trips on the bundled and narrow-word runtimes alike (the
    // module-width cast keeps the layout the script reads).
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::pair() -> Pair\n\
         struct Pair { a: Word, b: Word }\n\
         fn main() -> Word { host::pair().a + host::pair().b }",
        &arena,
    );
    vm.register_fn("host::pair", || -> Pair { Pair { a: 3, b: 4 } });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => assert_eq!(v, Value::Int(7)),
        other => panic!("expected Int(7), got {:?}", other),
    }
}

#[test]
fn register_fn_returning_nested_struct_builds_in_arena() {
    // A native returning a nested flat struct (a struct field and a tuple
    // field) builds its body directly in the arena: the parent is packed
    // through `struct_in_arena`, and the nested `Pair` and tuple are resolved
    // and copied into the parent's single arena allocation, so the arena holds
    // exactly one body (B28 P3 item 2, Increment 3). The script reads the
    // nested fields, proving the layout and residence round-trip.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::make() -> Holder\n\
         struct Pair { a: Word, b: Word }\n\
         struct Holder { p: Pair, coords: (Word, Word), tag: Word }\n\
         fn main() -> Word {\n\
             host::make().p.a + host::make().p.b\n\
             + host::make().coords.0 + host::make().coords.1\n\
             + host::make().tag\n\
         }",
        &arena,
    );
    vm.register_fn("host::make", || -> Holder {
        Holder {
            p: Pair { a: 1, b: 2 },
            coords: (3, 4),
            tag: 5,
        }
    });
    match vm.call(&[]).unwrap() {
        VmState::Finished(v) => assert_eq!(v, Value::Int(15)),
        other => panic!("expected Int(15), got {:?}", other),
    }
}

#[test]
fn untyped_native_composite_equality_faults_not_silently_wrong() {
    // An unsignatured native returns a composite the compiler cannot type, so
    // it cannot emit the field-wise comparison; the value flattens and reaches
    // the runtime CmpEq. Rather than byte-blob-compare the flat body (which
    // would silently mishandle IEEE floats), the VM faults with an actionable
    // message (B28 P3 item 5: resolving the documented residual).
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::origin\n\
         struct Point { x: Float, y: Float }\n\
         fn main() -> bool { host::origin() == host::origin() }",
        &arena,
    );
    vm.register_fn("host::origin", || -> Point { Point { x: 0.0, y: 0.0 } });
    let err = vm.call(&[]).unwrap_err();
    match err {
        VmError::TypeError(msg) => assert!(
            msg.contains("flat composite") || msg.contains("signature"),
            "fault should explain the missing type/signature, got: {msg}"
        ),
        other => panic!("expected a TypeError fault, got {:?}", other),
    }
}

#[test]
fn signed_native_composite_equality_compiles_field_wise() {
    // With a declared return type the compiler recovers the composite type and
    // emits the field-wise comparison, which is IEEE-correct on the flat body.
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = build_vm(
        "use host::origin() -> Point\n\
         struct Point { x: Float, y: Float }\n\
         fn main() -> bool { host::origin() == host::origin() }",
        &arena,
    );
    vm.register_fn("host::origin", || -> Point { Point { x: 0.0, y: 0.0 } });
    match vm.call(&[]).unwrap() {
        VmState::Finished(Value::Bool(b)) => assert!(b, "two equal origins compare equal"),
        other => panic!("expected Bool(true), got {:?}", other),
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
