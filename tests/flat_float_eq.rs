#![cfg(all(feature = "compile", feature = "verify", feature = "floats"))]
//! IEEE-correct equality for float-bearing composites (B28 P3 item 5).
//!
//! A composite that carries a `Float` field must compare equal with IEEE
//! semantics. The IEEE rules diverge from a raw-byte blob comparison in
//! exactly two places: positive and negative zero share a value but differ in
//! their byte patterns, and a `NaN` shares its byte pattern with itself but is
//! never equal to any value including itself. A byte-blob comparison reports
//! `(+0.0,) != (-0.0,)` and `(NaN,) == (NaN,)`, both wrong.
//!
//! These tests pass today because every float-bearing composite is boxed (a
//! `Vec<Value>` compared by the derived `PartialEq`, which compares each float
//! field with IEEE `==` and is therefore correct). They are simultaneously
//! the executable specification for B28 P3 item 5, which flattens
//! float-bearing composites into raw arena bytes: a byte-blob comparison of a
//! flat body would fail these assertions, so the flattening work must replace
//! the byte-blob composite equality with a compiler-emitted field-wise
//! comparison (the keystone "Phase A") for every composite kind that can
//! transitively carry a float. See `docs/process/REVERSE_PROMPT.md` and the
//! preserved work-in-progress at `docs/process/attic/b28-p3-item5-phaseA.patch`.

extern crate alloc;

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

/// Compile and run `src`, registering float-producing natives so the test
/// controls the exact bit patterns (`+0.0`, `-0.0`, `NaN`, `1.0`) that drive
/// the IEEE corner cases, rather than relying on literal lexing of `-0.0`.
fn run_bool(src: &str) -> bool {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_fn("pos_zero", || -> f64 { 0.0 });
    vm.register_fn("neg_zero", || -> f64 { -0.0 });
    vm.register_fn("nan", || -> f64 { f64::NAN });
    vm.register_fn("one", || -> f64 { 1.0 });
    match vm.call(&[]).expect("call") {
        VmState::Finished(Value::Bool(b)) => b,
        other => panic!("expected finished Bool, got {:?}", other),
    }
}

#[test]
fn struct_positive_and_negative_zero_compare_equal() {
    // IEEE: +0.0 == -0.0. A byte-blob compare would report the structs unequal
    // because the sign bit differs.
    let src = "use pos_zero() -> Float\n\
               use neg_zero() -> Float\n\
               struct FPair { a: Float, b: Float }\n\
               fn main() -> bool {\n\
                   let p = FPair { a: pos_zero(), b: pos_zero() };\n\
                   let q = FPair { a: neg_zero(), b: pos_zero() };\n\
                   p == q\n\
               }";
    assert!(run_bool(src), "+0.0 and -0.0 fields must compare equal");
}

#[test]
fn struct_with_nan_field_is_not_equal_to_itself() {
    // IEEE: NaN != NaN. A byte-blob compare would report the struct equal to
    // itself because the bit patterns are identical.
    let src = "use nan() -> Float\n\
               struct FBox { v: Float }\n\
               fn main() -> bool {\n\
                   let p = FBox { v: nan() };\n\
                   p == p\n\
               }";
    assert!(
        !run_bool(src),
        "a struct with a NaN field must not equal itself"
    );
}

#[test]
fn struct_ordinary_float_fields_compare_equal() {
    let src = "use one() -> Float\n\
               struct FBox { v: Float }\n\
               fn main() -> bool {\n\
                   let p = FBox { v: one() };\n\
                   let q = FBox { v: one() };\n\
                   p == q\n\
               }";
    assert!(
        run_bool(src),
        "equal ordinary float fields must compare equal"
    );
}

#[test]
fn struct_distinct_float_fields_compare_unequal() {
    let src = "use one() -> Float\n\
               use pos_zero() -> Float\n\
               struct FBox { v: Float }\n\
               fn main() -> bool {\n\
                   let p = FBox { v: one() };\n\
                   let q = FBox { v: pos_zero() };\n\
                   p == q\n\
               }";
    assert!(!run_bool(src), "distinct float fields must compare unequal");
}

#[test]
fn struct_mixed_scalar_and_float_fields_compare_equal() {
    // A flat body that interleaves a non-float scalar with a float field must
    // still compare field-wise; the Int field exercises the scalar arm and the
    // Float field the IEEE arm in the same body.
    let src = "use neg_zero() -> Float\n\
               use pos_zero() -> Float\n\
               struct Mixed { n: Word, f: Float }\n\
               fn main() -> bool {\n\
                   let p = Mixed { n: 5, f: pos_zero() };\n\
                   let q = Mixed { n: 5, f: neg_zero() };\n\
                   p == q\n\
               }";
    assert!(
        run_bool(src),
        "equal Int and +0.0/-0.0 fields must compare equal"
    );
}

#[test]
fn struct_mixed_unequal_scalar_compares_unequal() {
    let src = "use one() -> Float\n\
               struct Mixed { n: Word, f: Float }\n\
               fn main() -> bool {\n\
                   let p = Mixed { n: 5, f: one() };\n\
                   let q = Mixed { n: 6, f: one() };\n\
                   p == q\n\
               }";
    assert!(
        !run_bool(src),
        "an unequal Int field must make the structs unequal"
    );
}

#[test]
fn tuple_positive_and_negative_zero_compare_equal() {
    // A tuple carrying a float: its equality must be IEEE-correct (today the
    // tuple is boxed and the derived comparison delivers this; the flattening
    // work must preserve it field-wise).
    let src = "use pos_zero() -> Float\n\
               use neg_zero() -> Float\n\
               fn main() -> bool {\n\
                   let p = (pos_zero(), 7);\n\
                   let q = (neg_zero(), 7);\n\
                   p == q\n\
               }";
    assert!(run_bool(src), "tuple +0.0/-0.0 elements must compare equal");
}

#[test]
fn tuple_with_nan_is_not_equal_to_itself() {
    let src = "use nan() -> Float\n\
               fn main() -> bool {\n\
                   let p = (nan(), 7);\n\
                   p == p\n\
               }";
    assert!(
        !run_bool(src),
        "a tuple with a NaN element must not equal itself"
    );
}

#[test]
fn array_positive_and_negative_zero_compare_equal() {
    // An array carrying floats: its equality must be IEEE-correct (today the
    // array is boxed; the flattening work must preserve it field-wise).
    let src = "use pos_zero() -> Float\n\
               use neg_zero() -> Float\n\
               fn main() -> bool {\n\
                   let p = [pos_zero(), pos_zero()];\n\
                   let q = [neg_zero(), neg_zero()];\n\
                   p == q\n\
               }";
    assert!(run_bool(src), "array +0.0/-0.0 elements must compare equal");
}

#[test]
fn array_with_nan_is_not_equal_to_itself() {
    let src = "use nan() -> Float\n\
               use one() -> Float\n\
               fn main() -> bool {\n\
                   let p = [one(), nan()];\n\
                   p == p\n\
               }";
    assert!(
        !run_bool(src),
        "an array with a NaN element must not equal itself"
    );
}

#[test]
fn nested_struct_float_compares_field_wise() {
    // The outer struct nests a float-bearing inner struct; equality must
    // descend into it (today via the boxed derived comparison, later via the
    // recursive field-wise comparison). The trailing Int adds a scalar field.
    let src = "use pos_zero() -> Float\n\
               use neg_zero() -> Float\n\
               struct Inner { v: Float }\n\
               struct Outer { i: Inner, n: Word }\n\
               fn main() -> bool {\n\
                   let p = Outer { i: Inner { v: pos_zero() }, n: 1 };\n\
                   let q = Outer { i: Inner { v: neg_zero() }, n: 1 };\n\
                   p == q\n\
               }";
    assert!(
        run_bool(src),
        "a nested float field must compare equal field-wise"
    );
}

#[test]
fn nested_struct_distinct_inner_float_compares_unequal() {
    let src = "use one() -> Float\n\
               use pos_zero() -> Float\n\
               struct Inner { v: Float }\n\
               struct Outer { i: Inner, n: Word }\n\
               fn main() -> bool {\n\
                   let p = Outer { i: Inner { v: one() }, n: 1 };\n\
                   let q = Outer { i: Inner { v: pos_zero() }, n: 1 };\n\
                   p == q\n\
               }";
    assert!(
        !run_bool(src),
        "a distinct nested float field must compare unequal"
    );
}

#[test]
fn enum_same_variant_positive_and_negative_zero_compare_equal() {
    // A float-bearing enum is compared by variant dispatch: same variant,
    // then payload field-wise. +0.0 and -0.0 in the payload must compare
    // equal.
    let src = "use pos_zero() -> Float\n\
               use neg_zero() -> Float\n\
               enum Shape { Dot, Circle(Float) }\n\
               fn main() -> bool {\n\
                   let p = Shape::Circle(pos_zero());\n\
                   let q = Shape::Circle(neg_zero());\n\
                   p == q\n\
               }";
    assert!(run_bool(src), "enum payload +0.0/-0.0 must compare equal");
}

#[test]
fn enum_nan_payload_is_not_equal_to_itself() {
    let src = "use nan() -> Float\n\
               enum Shape { Dot, Circle(Float) }\n\
               fn main() -> bool {\n\
                   let p = Shape::Circle(nan());\n\
                   p == p\n\
               }";
    assert!(
        !run_bool(src),
        "an enum with a NaN payload must not equal itself"
    );
}

#[test]
fn enum_distinct_variants_compare_unequal() {
    let src = "use one() -> Float\n\
               enum Shape { Dot, Circle(Float) }\n\
               fn main() -> bool {\n\
                   let p = Shape::Circle(one());\n\
                   let q = Shape::Dot;\n\
                   p == q\n\
               }";
    assert!(
        !run_bool(src),
        "distinct enum variants must compare unequal"
    );
}

#[test]
fn enum_same_variant_distinct_floats_compare_unequal() {
    let src = "use one() -> Float\n\
               use pos_zero() -> Float\n\
               enum Shape { Dot, Circle(Float) }\n\
               fn main() -> bool {\n\
                   let p = Shape::Circle(one());\n\
                   let q = Shape::Circle(pos_zero());\n\
                   p == q\n\
               }";
    assert!(
        !run_bool(src),
        "an enum payload with distinct floats must compare unequal"
    );
}

#[test]
fn enum_two_float_payload_fields_compare_equal() {
    // A two-field float payload exercises the per-field payload loop.
    let src = "use pos_zero() -> Float\n\
               use neg_zero() -> Float\n\
               use one() -> Float\n\
               enum Shape { Dot, Box(Float, Float) }\n\
               fn main() -> bool {\n\
                   let p = Shape::Box(pos_zero(), one());\n\
                   let q = Shape::Box(neg_zero(), one());\n\
                   p == q\n\
               }";
    assert!(
        run_bool(src),
        "equal two-field float payloads must compare equal"
    );
}

#[test]
fn enum_carrying_float_struct_payload_compares_field_wise() {
    // The byte-blob hole: an enum whose payload is a float-bearing struct.
    // Equality must descend through the enum payload into the struct and
    // compare its float field with IEEE semantics (+0.0 == -0.0), not by a
    // byte blob. This is the case the flattening work would otherwise break.
    let src = "use pos_zero() -> Float\n\
               use neg_zero() -> Float\n\
               struct P { x: Float }\n\
               enum E { None, Some(P) }\n\
               fn main() -> bool {\n\
                   let a = E::Some(P { x: pos_zero() });\n\
                   let b = E::Some(P { x: neg_zero() });\n\
                   a == b\n\
               }";
    assert!(
        run_bool(src),
        "an enum carrying a float struct must compare field-wise into the struct"
    );
}

#[test]
fn enum_carrying_float_struct_distinct_compares_unequal() {
    let src = "use one() -> Float\n\
               use pos_zero() -> Float\n\
               struct P { x: Float }\n\
               enum E { None, Some(P) }\n\
               fn main() -> bool {\n\
                   let a = E::Some(P { x: one() });\n\
                   let b = E::Some(P { x: pos_zero() });\n\
                   a == b\n\
               }";
    assert!(
        !run_bool(src),
        "a distinct float struct inside an enum payload must compare unequal"
    );
}
