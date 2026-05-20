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
