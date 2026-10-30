//! A host composite input is checked against its declared flat kind and size.
//!
//! # The defect
//!
//! The VM packs a host-built tuple, struct or array argument in the host's
//! field order, and the script reads it at offsets baked for the declared
//! order. A body of another size was read by the wrong layout, so a field of
//! the wrong width or an extra field silently shifted every later field. A
//! defect report from another session, `volaw-daw-b2`, reproduced on `v0.2.3`.
//! The operator ruled on 2026-10-30 for a documented host contract with a size
//! check now, question B2, option (b).
//!
//! # What is established
//!
//! An entry argument and a resume value whose packed body differs in kind or
//! byte length from the chunk signature are refused with a `TypeError` before
//! any instruction runs, and a scalar where a composite is declared is refused.
//! The matching value is admitted.
//!
//! # What is NOT established, and is pinned
//!
//! Field order. Two fields of equal width in the opposite order pack to the
//! same size, so they pass, and the script reads each field from the other's
//! position. The wire carries each body's kind and size and not its field
//! layout. Closing it needs field layouts on the wire, option (c) of the same
//! question, which awaits the operator. The pin below uses two `Word` fields,
//! because the same reorder over a `Text` field and a `Word` field would make
//! the VM rebuild a string from a word that is not a pointer, which is not
//! something a test may execute.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::vm::{
    DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState, required_persistent_capacity_for,
};
use keleusma::{Arena, Value};

fn module(source: &str) -> keleusma::bytecode::Module {
    let tokens = keleusma::lexer::tokenize(source).expect("lex");
    keleusma::compiler::compile(&keleusma::parser::parse(&tokens).expect("parse"))
        .unwrap_or_else(|e| panic!("{source}\n{}", e.message))
}

fn arena_for(m: &keleusma::bytecode::Module) -> Arena {
    let need = required_persistent_capacity_for(m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    arena
}

fn call(source: &str, arg: Value) -> Result<VmState, VmError> {
    let m = module(source);
    let arena = arena_for(&m);
    let mut vm = Vm::new(m, &arena).expect("verifies");
    vm.call(&[arg])
}

fn point(fields: &[(&str, Value)]) -> Value {
    Value::struct_value(
        "P".into(),
        fields
            .iter()
            .map(|(n, v)| ((*n).into(), v.clone()))
            .collect(),
    )
}

fn assert_shape_refusal(result: Result<VmState, VmError>, what: &str) {
    match result {
        Err(VmError::TypeError(m)) => assert!(
            m.contains("field order and field types"),
            "{what}: refused for another reason: {m}"
        ),
        other => panic!("{what}: not refused by the shape check: {other:?}"),
    }
}

const WORDS: &str = "struct P { a: Word, b: Word }\nfn main(p: P) -> Word { p.a * 10 + p.b }";
const WORD_BYTE: &str = "struct P { a: Word, b: Byte }\nfn main(p: P) -> Word { p.a }";

#[test]
fn a_matching_struct_argument_is_admitted() {
    let p = point(&[("a", Value::Int(4)), ("b", Value::Int(2))]);
    assert!(matches!(
        call(WORDS, p),
        Ok(VmState::Finished(Value::Int(42)))
    ));
    let p = point(&[("a", Value::Int(7)), ("b", Value::Byte(1))]);
    assert!(matches!(
        call(WORD_BYTE, p),
        Ok(VmState::Finished(Value::Int(7)))
    ));
}

#[test]
fn a_struct_argument_with_an_extra_field_is_refused() {
    let p = point(&[
        ("a", Value::Int(4)),
        ("b", Value::Int(2)),
        ("c", Value::Int(9)),
    ]);
    assert_shape_refusal(call(WORDS, p), "an extra field");
}

#[test]
fn a_struct_argument_with_a_field_of_another_width_is_refused() {
    // `b` is declared `Byte`, and the host passes a `Word`.
    let p = point(&[("a", Value::Int(7)), ("b", Value::Int(1))]);
    assert_shape_refusal(call(WORD_BYTE, p), "a wider field");
}

#[test]
fn a_scalar_where_a_struct_is_declared_is_refused() {
    assert_shape_refusal(call(WORDS, Value::Int(3)), "a scalar for a struct");
}

#[test]
fn tuple_and_array_arguments_of_another_size_are_refused() {
    let tuple = "fn main(t: (Word, Word)) -> Word { t.0 }";
    assert!(matches!(
        call(tuple, Value::tuple(vec![Value::Int(1), Value::Int(2)])),
        Ok(VmState::Finished(Value::Int(1)))
    ));
    assert_shape_refusal(
        call(
            tuple,
            Value::tuple(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
        ),
        "a longer tuple",
    );
    let array = "fn main(a: [Word; 3]) -> Word { a[0] }";
    assert!(matches!(
        call(
            array,
            Value::array(vec![Value::Int(5), Value::Int(6), Value::Int(7)])
        ),
        Ok(VmState::Finished(Value::Int(5)))
    ));
    assert_shape_refusal(
        call(array, Value::array(vec![Value::Int(5), Value::Int(6)])),
        "a shorter array",
    );
}

#[test]
fn a_resume_value_of_another_size_is_refused() {
    let src =
        "struct P { a: Word, b: Word }\nloop main(p: P) -> Word { let q = yield p.a; yield q.b }";
    let m = module(src);
    let arena = arena_for(&m);
    let mut vm = Vm::new(m, &arena).expect("verifies");
    let first = point(&[("a", Value::Int(1)), ("b", Value::Int(2))]);
    assert!(matches!(
        vm.call(&[first]),
        Ok(VmState::Yielded(Value::Int(1)))
    ));
    let bad = point(&[
        ("a", Value::Int(1)),
        ("b", Value::Int(2)),
        ("c", Value::Int(3)),
    ]);
    assert_shape_refusal(vm.resume(bad), "a resume value with an extra field");
}

/// **The residual.** Two `Word` fields in the opposite order pass the check and
/// are read crosswise. If this starts failing because the value is refused,
/// field layouts reached the wire, and this pin should record that instead.
#[test]
fn a_reordered_struct_of_equal_width_fields_is_not_detected() {
    let swapped = point(&[("b", Value::Int(2)), ("a", Value::Int(4))]);
    assert!(
        matches!(call(WORDS, swapped), Ok(VmState::Finished(Value::Int(24)))),
        "the reorder was detected or read otherwise"
    );
}
