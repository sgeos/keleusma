#![cfg(all(feature = "compile", feature = "verify"))]
//! Host-boundary decode of a flat composite's reference fields (B28 P3).
//!
//! A script builds a struct with a `Text` field (and, separately, an
//! opaque field). The struct is flat, so the `Text` field is a two-word
//! `(ptr, len)` arena reference and the opaque field a registry index.
//! `Vm::decode::<T>` resolves both through the VM's arena and opaque
//! registry into a host `#[derive(KeleusmaType)]` type.

extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, HostOpaque, KeleusmaType, Value, host_arc};

#[derive(KeleusmaType)]
struct Greeting {
    msg: String,
    n: i64,
}

#[test]
fn decode_flat_struct_with_text_field() {
    let src = "struct Greeting { msg: Text, n: Word }\n\
               fn main() -> Greeting { Greeting { msg: \"hi\", n: 5 } }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    let val = match vm.call(&[]).expect("call") {
        VmState::Finished(v) => v,
        other => panic!("expected finished, got {:?}", other),
    };
    let g: Greeting = vm.decode(&val).expect("decode");
    assert_eq!(g.msg, "hi");
    assert_eq!(g.n, 5);
}

struct Handle {
    label: String,
}
impl HostOpaque for Handle {
    fn type_name(&self) -> &'static str {
        "Handle"
    }
}

#[derive(KeleusmaType)]
struct Carrier {
    h: Arc<dyn HostOpaque>,
    n: i64,
}

#[test]
fn decode_flat_struct_with_opaque_field() {
    let src = "use make_handle\n\
               struct Carrier { h: Handle, n: Word }\n\
               fn main() -> Carrier { Carrier { h: make_handle(), n: 9 } }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_native("make_handle", |_args| {
        Ok(Value::Opaque(host_arc(Handle {
            label: "decoded".into(),
        })))
    });
    let val = match vm.call(&[]).expect("call") {
        VmState::Finished(v) => v,
        other => panic!("expected finished, got {:?}", other),
    };
    let c: Carrier = vm.decode(&val).expect("decode");
    assert_eq!(c.n, 9);
    let typed: &Handle = c.h.as_ref().downcast_ref::<Handle>().expect("downcast");
    assert_eq!(typed.label, "decoded");
}

#[derive(KeleusmaType, PartialEq, Debug)]
struct Pair {
    a: i64,
    b: i64,
}

#[test]
fn decode_arena_resident_flat_struct() {
    // The read-before-resume keystone (B28 P3 item 5 C3): a composite whose
    // flat body lives in the arena (the state a yielded/returned value is in
    // once boundary materialisation is removed) must decode through the
    // arena-aware `resolve` path. Before this change the derived decode called
    // `FlatComposite::as_bytes`, which panics on an arena body; this test
    // would panic without the keystone and passes with it.
    use keleusma::bytecode::{GenericValue, StructBody};
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let v = Value::struct_value(
        "Pair".into(),
        vec![
            ("a".into(), GenericValue::Int(3)),
            ("b".into(), GenericValue::Int(4)),
        ],
    );
    // Migrate the flat body into the arena's top region.
    let v = v.into_arena_body(&arena).expect("migrate to arena");
    assert!(
        matches!(&v, GenericValue::Struct(StructBody::Flat(_))),
        "expected a flat struct body"
    );
    let ctx = keleusma::RefContext {
        arena: &arena,
        opaques: &[],
        word_bytes: 8,
        float_bytes: 8,
        ref_epoch: arena.epoch(),
    };
    let p = Pair::from_value_ctx(&v, &ctx).expect("decode arena-resident flat struct");
    assert_eq!(p, Pair { a: 3, b: 4 });
}

#[test]
fn native_receives_struct_with_text_field() {
    // A register_fn native takes a struct argument with a Text field; the
    // native-argument marshalling resolves the flat Text field through the
    // VM context (B28 P3).
    let src = "use greet_len\n\
               struct Greeting { msg: Text, n: Word }\n\
               fn main() -> Word { greet_len(Greeting { msg: \"hello\", n: 2 }) }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_fn("greet_len", |g: Greeting| -> i64 {
        g.msg.len() as i64 + g.n
    });
    match vm.call(&[]).expect("call") {
        VmState::Finished(Value::Int(n)) => assert_eq!(n, 7),
        other => panic!("expected Int(7), got {:?}", other),
    }
}
