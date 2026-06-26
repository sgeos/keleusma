#![cfg(all(feature = "compile", feature = "verify"))]
//! Flat opaque elements in tuples and arrays (B28 P3 item 3).
//!
//! An opaque element of a tuple or array is now flat: the VM interns it to a
//! one-word registry index packed into the arena body, and access resolves
//! the index back to the host reference. A text element stays boxed (to
//! preserve the KStr lifecycle), so a reference-bearing tuple flattens only
//! its opaque elements. The native carries a declared signature so the
//! compiler recovers the element type and bakes flat access that agrees with
//! the value-driven construction.

extern crate alloc;

use alloc::sync::Arc;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, HostOpaque, Value, host_arc};

struct Gauge(i64);
impl HostOpaque for Gauge {
    fn type_name(&self) -> &'static str {
        "Handle"
    }
}

fn run(src: &str) -> Value {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_native("make_handle", |_| Ok(Value::Opaque(host_arc(Gauge(100)))));
    vm.register_fn("handle_val", |h: Arc<dyn HostOpaque>| -> i64 {
        h.as_ref()
            .downcast_ref::<Gauge>()
            .map(|g| g.0)
            .unwrap_or(-1)
    });
    match vm.call(&[]).expect("call") {
        VmState::Finished(v) => v,
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn tuple_with_opaque_element_flattens_and_resolves() {
    // t.0 is the opaque (a one-word index in the flat body); t.1 is the Word
    // following it. Reading both proves the flat layout packs the opaque and
    // the trailing scalar at the right offsets, and that the index resolves.
    let src = "use make_handle() -> Handle\n\
               use handle_val(Handle) -> Word\n\
               fn main() -> Word { let t = (make_handle(), 7); handle_val(t.0) + t.1 }";
    assert_eq!(run(src), Value::Int(107));
}

#[test]
fn array_of_opaque_flattens_and_indexes() {
    // A homogeneous array of opaque: each element is a one-word index. Index
    // 1 resolves to the second handle.
    let src = "use make_handle() -> Handle\n\
               use handle_val(Handle) -> Word\n\
               fn main() -> Word { let a = [make_handle(), make_handle()]; handle_val(a[1]) }";
    assert_eq!(run(src), Value::Int(100));
}

#[test]
fn native_returning_word_text_tuple_flattens_and_destructures() {
    // B37 regression. An unsignatured native that returns a `(Word, Text)`
    // tuple builds a boxed body with a `StaticStr` element (no arena at
    // `Value::tuple`). The compiler bakes flat access for the declared
    // `(Word, Text)` return, since `Text` is a flat field at the host word
    // width. Before the native-result canonicalisation promoted the
    // `StaticStr` field to an arena `KStr`, the boxed body mismatched the flat
    // access and raised `InvalidBytecode("GetTupleField operand form does not
    // match tuple body")`. Now the result flattens, both fields read back, and
    // the text resolves through the marshalling boundary.
    let src = "use cmd() -> (Word, Text)\n\
               use tlen(Text) -> Word\n\
               fn main() -> Word { let t = cmd(); t.0 + tlen(t.1) }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_native("cmd", |_| {
        Ok(Value::tuple(alloc::vec![
            Value::Int(5),
            Value::StaticStr(alloc::string::String::from("abc")),
        ]))
    });
    vm.register_fn("tlen", |s: alloc::string::String| -> i64 { s.len() as i64 });
    match vm.call(&[]).expect("call") {
        VmState::Finished(v) => assert_eq!(v, Value::Int(8)),
        other => panic!("expected finished, got {:?}", other),
    }
}

#[test]
fn tuple_with_opaque_and_trailing_scalars_offsets() {
    // The opaque occupies one word; the two trailing scalars must read back
    // at their post-opaque offsets. The asymmetric weight on `t.1`
    // distinguishes the two trailing offsets, so a swap would change the
    // result. The combined value is kept within the 8-bit word range
    // (`100 + 6 + 4 = 110 <= 127`) so the assertion holds under
    // narrow-word builds (for example `--features narrow-word-8`), where a
    // sum of 134 would correctly overflow to -122 and mask the offset check.
    let src = "use make_handle() -> Handle\n\
               use handle_val(Handle) -> Word\n\
               fn main() -> Word { \
                   let t = (make_handle(), 3, 4); \
                   handle_val(t.0) + t.1 * 2 + t.2 \
               }";
    assert_eq!(run(src), Value::Int(100 + 3 * 2 + 4));
}
