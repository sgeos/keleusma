#![cfg(all(feature = "compile", feature = "verify"))]
//! Composite construction builds flat bodies directly in the arena
//! (B28 P3 item 5 C-residual 3b).
//!
//! The VM `Op::NewComposite` flat path packs a freshly constructed tuple,
//! array, struct, or enum straight into the arena's top ephemeral head with no
//! intermediate global-heap `Inline` scratch and no per-operand `materialized`
//! (Arena -> heap `Inline`) read-back. A nested child is inlined by resolving
//! its arena bytes and copying them into the parent's destination in place.
//! These tests exercise that path through nesting (struct in struct, struct in
//! tuple, array of structs) and confirm the packed body reads back
//! field-for-field, so the in-place nested inlining preserves offsets.

extern crate alloc;

use keleusma::Arena;
use keleusma::bytecode::{EnumBody, StructBody, TupleBody, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};

fn run(src: &str) -> (Value, Arena) {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    // Scope the VM so its `&arena` borrow ends before the arena is moved out.
    // The returned value holds raw arena handles, not a lifetime borrow, and
    // the arena's heap buffer keeps its address when the owner is moved, so the
    // handles stay valid for the host's read-before-resume decode.
    let v = {
        let mut vm = Vm::new(m, &arena).expect("verify");
        match vm.call(&[]).expect("call") {
            VmState::Finished(v) => v,
            other => panic!("expected finished, got {:?}", other),
        }
    };
    (v, arena)
}

fn run_word(src: &str) -> i64 {
    let (v, _arena) = run(src);
    match v {
        Value::Int(n) => n,
        other => panic!("expected Int, got {:?}", other),
    }
}

#[test]
fn nested_struct_in_struct_constructs_flat_in_arena() {
    // An outer struct whose field is itself a struct: the inner struct is
    // constructed and migrated to the arena first, then the outer construction
    // resolves the inner arena body and inlines it. The result must be a flat,
    // arena-resident body and the nested field reads must round-trip.
    let src = "struct Inner { a: Word, b: Word }\n\
               struct Outer { inner: Inner, c: Word }\n\
               fn main() -> Outer { Outer { inner: Inner { a: 1, b: 2 }, c: 3 } }";
    let (v, arena) = run(src);
    match &v {
        Value::Struct(StructBody::Flat(fc)) => {
            assert!(
                fc.is_valid(&arena),
                "outer struct body must be arena-resident and valid"
            );
        }
        other => panic!("expected flat struct body, got {:?}", other),
    }
}

#[test]
fn nested_struct_in_struct_fields_round_trip() {
    let src = "struct Inner { a: Word, b: Word }\n\
               struct Outer { inner: Inner, c: Word }\n\
               fn main() -> Word { \
                   let o = Outer { inner: Inner { a: 10, b: 20 }, c: 30 }; \
                   o.inner.a + o.inner.b + o.c }";
    assert_eq!(run_word(src), 60);
}

#[test]
fn struct_in_tuple_round_trips() {
    let src = "struct P { x: Word, y: Word }\n\
               fn main() -> Word { \
                   let t = (P { x: 4, y: 5 }, 6); \
                   t.0.x + t.0.y + t.1 }";
    assert_eq!(run_word(src), 15);
}

#[test]
fn array_of_structs_round_trips() {
    let src = "struct P { x: Word, y: Word }\n\
               fn main() -> Word { \
                   let a = [P { x: 1, y: 2 }, P { x: 3, y: 4 }]; \
                   a[0].x + a[0].y + a[1].x + a[1].y }";
    assert_eq!(run_word(src), 10);
}

#[test]
fn deeply_nested_tuple_round_trips() {
    // Three levels of nesting exercise repeated in-place inlining of an
    // arena child into an arena parent. Intermediate bindings avoid the
    // `.0.0` token sequence, which the lexer reads as a float literal.
    let src = "fn main() -> Word { \
                   let t = (((1, 2), 3), 4); \
                   let a = t.0; \
                   let b = a.0; \
                   b.0 + b.1 + a.1 + t.1 }";
    assert_eq!(run_word(src), 10);
}

#[test]
fn enum_with_nested_struct_payload_is_flat() {
    let src = "struct P { x: Word, y: Word }\n\
               enum E { Empty, Pair(P) }\n\
               fn main() -> E { E::Pair(P { x: 7, y: 8 }) }";
    let (v, arena) = run(src);
    match &v {
        Value::Enum(EnumBody::Flat(fc)) => {
            assert!(fc.is_valid(&arena), "enum body must be arena-resident");
        }
        other => panic!("expected flat enum body, got {:?}", other),
    }
}

#[test]
fn scalar_tuple_constructs_flat() {
    // A purely scalar tuple has no nested children; it still packs directly
    // into the arena rather than through an owned Inline body.
    let src = "fn main() -> (Word, Word, Word) { (1, 2, 3) }";
    let (v, arena) = run(src);
    match &v {
        Value::Tuple(TupleBody::Flat(fc)) => {
            assert!(fc.is_valid(&arena), "tuple body must be arena-resident");
        }
        other => panic!("expected flat tuple body, got {:?}", other),
    }
}
