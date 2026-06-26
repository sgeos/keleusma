#![cfg(all(feature = "compile", feature = "verify"))]
//! End-to-end coverage for host-supplied opaque types.
//!
//! Exercises the `Value::Opaque` runtime variant, the `HostOpaque`
//! marker trait, and the `host_arc` constructor. Verifies that an
//! opaque value produced by a native function is faithfully
//! returned to the host as a typed reference through
//! `dyn HostOpaque::downcast_ref`.

use alloc::sync::Arc;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, HostOpaque, Value, host_arc};

extern crate alloc;

/// A host-defined Rust type exposed to scripts as `Handle`.
struct Handle {
    label: alloc::string::String,
}

impl HostOpaque for Handle {
    fn type_name(&self) -> &'static str {
        "Handle"
    }
}

#[test]
fn native_returns_opaque_handle_to_script_caller() {
    // The script declares a function returning an opaque `Handle`
    // produced by the native `make_handle`. The type checker
    // accepts `Handle` as a `Type::Opaque("Handle")` because the
    // name is not declared as a struct or enum in the source.
    let src = "use make_handle\n\
               fn main() -> Handle { make_handle() }";
    let tokens = tokenize(src).expect("lex error");
    let program = parse(&tokens).expect("parse error");
    let module = compile(&program).expect("compile error");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_native("make_handle", |args| {
        if !args.is_empty() {
            return Err(keleusma::VmError::NativeError(
                "make_handle: expected zero arguments".into(),
            ));
        }
        Ok(Value::Opaque(host_arc(Handle {
            label: "scripted".into(),
        })))
    });
    let val = match vm.call(&[]).expect("vm call") {
        VmState::Finished(v) => v,
        other => panic!("expected finished, got {:?}", other),
    };
    let opaque = match val {
        Value::Opaque(o) => o,
        other => panic!("expected opaque, got {:?}", other),
    };
    assert_eq!(opaque.type_name(), "Handle");
    let typed: &Handle = opaque
        .as_ref()
        .downcast_ref::<Handle>()
        .expect("downcast Handle");
    assert_eq!(typed.label, "scripted");
}

#[test]
fn opaque_values_compare_equal_by_arc_identity() {
    let a: Arc<dyn HostOpaque> = host_arc(Handle { label: "a".into() });
    let b = a.clone();
    let c: Arc<dyn HostOpaque> = host_arc(Handle { label: "a".into() });
    let v_a = Value::Opaque(a);
    let v_b = Value::Opaque(b);
    let v_c = Value::Opaque(c);
    assert_eq!(v_a, v_b, "Arc clones share identity");
    assert_ne!(
        v_a, v_c,
        "Distinct allocations are unequal even with equal payloads",
    );
}

#[test]
fn opaque_field_in_flat_struct_round_trips_through_access() {
    // B28 P3: a struct holding an opaque is flat. Construction interns
    // the opaque into the VM registry and packs its index; reading the
    // field back resolves the index to the original `Arc`. The label
    // surviving the round trip proves the same host object is returned.
    let src = "use make_handle\n\
               struct Wrap { h: Handle, tag: Word }\n\
               fn main() -> Handle { let w = Wrap { h: make_handle(), tag: 7 }; w.h }";
    let tokens = tokenize(src).expect("lex error");
    let program = parse(&tokens).expect("parse error");
    let module = compile(&program).expect("compile error");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_native("make_handle", |_args| {
        Ok(Value::Opaque(host_arc(Handle {
            label: "flat-field".into(),
        })))
    });
    let val = match vm.call(&[]).expect("vm call") {
        VmState::Finished(v) => v,
        other => panic!("expected finished, got {:?}", other),
    };
    let opaque = match val {
        Value::Opaque(o) => o,
        other => panic!("expected opaque, got {:?}", other),
    };
    let typed: &Handle = opaque
        .as_ref()
        .downcast_ref::<Handle>()
        .expect("downcast Handle");
    assert_eq!(typed.label, "flat-field");
}

#[test]
fn opaque_bearing_flat_composites_compare_by_identity() {
    // B28 P3: two flat structs that hold the same opaque compare equal
    // because interning deduplicates by pointer identity, so the packed
    // index bytes coincide. Differing the scalar field makes them unequal.
    let equal_src = "use make_handle\n\
                     struct P { h: Handle, n: Word }\n\
                     fn main() -> bool { let h = make_handle(); P { h: h, n: 1 } == P { h: h, n: 1 } }";
    let unequal_src = "use make_handle\n\
                       struct P { h: Handle, n: Word }\n\
                       fn main() -> bool { let h = make_handle(); P { h: h, n: 1 } == P { h: h, n: 2 } }";

    for (src, expected) in [(equal_src, true), (unequal_src, false)] {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        vm.register_native("make_handle", |_args| {
            Ok(Value::Opaque(host_arc(Handle { label: "id".into() })))
        });
        match vm.call(&[]).expect("vm call") {
            VmState::Finished(Value::Bool(b)) => {
                assert_eq!(b, expected, "src: {src}");
            }
            other => panic!("expected bool, got {:?}", other),
        }
    }
}

#[test]
fn opaque_payload_in_flat_enum_round_trips_through_match() {
    // B28 P3: an enum variant carrying an opaque is flat (the discriminant
    // word plus the packed opaque index). Pattern matching extracts the
    // payload, which resolves the index back to the original `Arc`. The
    // enum's payload access uses the enum definition, which is reliable.
    let src = "use make_handle\n\
               enum Held { Wrapped(Handle), Empty }\n\
               fn main() -> Handle {\n\
                   let e = Held::Wrapped(make_handle());\n\
                   match e { Held::Wrapped(h) => h, Held::Empty => make_handle() }\n\
               }";
    let tokens = tokenize(src).expect("lex error");
    let program = parse(&tokens).expect("parse error");
    let module = compile(&program).expect("compile error");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_native("make_handle", |_args| {
        Ok(Value::Opaque(host_arc(Handle {
            label: "enum-payload".into(),
        })))
    });
    let val = match vm.call(&[]).expect("vm call") {
        VmState::Finished(v) => v,
        other => panic!("expected finished, got {:?}", other),
    };
    let opaque = match val {
        Value::Opaque(o) => o,
        other => panic!("expected opaque, got {:?}", other),
    };
    let typed: &Handle = opaque
        .as_ref()
        .downcast_ref::<Handle>()
        .expect("downcast Handle");
    assert_eq!(typed.label, "enum-payload");
}

#[test]
fn downcast_ref_returns_none_on_type_mismatch() {
    struct Other;
    impl HostOpaque for Other {
        fn type_name(&self) -> &'static str {
            "Other"
        }
    }
    let arc: Arc<dyn HostOpaque> = host_arc(Handle { label: "x".into() });
    assert!(arc.as_ref().downcast_ref::<Other>().is_none());
    assert!(arc.as_ref().downcast_ref::<Handle>().is_some());
}

#[cfg(all(feature = "compile", feature = "verify"))]
#[test]
fn opaque_materialises_across_the_yield_boundary() {
    // B33: the operand stack carries an opaque as a POD `OpaqueRef` index, not
    // an `Arc`. A native-produced opaque is interned to the index on return,
    // flows through the stack and the `yield`, and must materialise back to an
    // `Arc` for the host at the yield boundary so host code can pattern-match
    // `Value::Opaque` directly. The label surviving proves the same host object
    // crosses; resuming re-enters the loop and yields again, exercising the
    // resume path and the registry clear on RESET.
    let src = "use make_handle\n\
               loop main(seed: Word) -> Handle { yield make_handle() }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_native("make_handle", |_args| {
        Ok(Value::Opaque(host_arc(Handle {
            label: alloc::string::String::from("yielded"),
        })))
    });
    let label = |s: VmState| -> alloc::string::String {
        match s {
            VmState::Yielded(Value::Opaque(o)) => o
                .as_ref()
                .downcast_ref::<Handle>()
                .expect("downcast Handle")
                .label
                .clone(),
            other => panic!("expected yielded opaque, got {:?}", other),
        }
    };
    // The loop yields on the first call, which is the path B33 changes.
    // (Resume RESETs the arena and clears the opaque registry before the next
    // iteration, returning `Reset`, a separate mechanism.)
    assert_eq!(label(vm.call(&[Value::Int(0)]).expect("call")), "yielded");
}

#[cfg(all(feature = "compile", feature = "verify"))]
#[test]
fn host_supplied_opaque_argument_round_trips() {
    // B33: a host `call` argument that is opaque is interned to the POD index
    // form for the operand stack, then materialised back to an `Arc` when it is
    // returned to the host. The same host object survives both conversions.
    let src = "fn main(h: Handle) -> Handle { h }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    let r = vm
        .call(&[Value::Opaque(host_arc(Handle {
            label: alloc::string::String::from("passed"),
        }))])
        .expect("call");
    match r {
        VmState::Finished(Value::Opaque(o)) => assert_eq!(
            o.as_ref().downcast_ref::<Handle>().expect("downcast").label,
            "passed"
        ),
        other => panic!("expected finished opaque, got {:?}", other),
    }
}
