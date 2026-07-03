#![cfg(all(feature = "compile", feature = "verify"))]
//! `Multiword<N>` fixed-width multi-word integer, phase 1: the type,
//! construction from a tuple literal, and digit indexing (B19). The
//! value is represented as a flat little-endian array of N words, so a
//! `Multiword<N>` built from `(d0, d1, ..., d_{N-1}) as Multiword<N>`
//! indexes to its digits with `m[i]`, digit 0 being least significant.

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

fn run_to_int(src: &str) -> i64 {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]).expect("call") {
        VmState::Finished(Value::Int(n)) => n,
        other => panic!("expected a finished integer, got {:?}", other),
    }
}

#[test]
fn multiword_construct_and_index_digit_zero() {
    // Digit 0 is the least significant word.
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (42, 7, 0, 0) as Multiword<4>; m[0] }"),
        42
    );
}

#[test]
fn multiword_index_higher_digits() {
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (42, 7, 3, 9) as Multiword<4>; m[1] }"),
        7
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (42, 7, 3, 9) as Multiword<4>; m[3] }"),
        9
    );
}

#[test]
fn multiword_is_a_first_class_parameter_type() {
    // `Multiword<N>` appears in a function signature and is passed by
    // value like any other type.
    let src = "fn first(m: Multiword<4>) -> Word { m[0] }\n\
               fn main() -> Word { let m = (99, 0, 0, 0) as Multiword<4>; first(m) }";
    assert_eq!(run_to_int(src), 99);
}

#[test]
fn multiword_two_word_digits_sum() {
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (100, 200) as Multiword<2>; m[0] + m[1] }"),
        300
    );
}

#[test]
fn multiword_construct_from_non_literal_tuple() {
    // A tuple bound to a variable also casts to Multiword<N>; the
    // compiler stashes it and extracts each digit.
    let src = "fn main() -> Word { let t = (11, 22, 33, 44); let m = t as Multiword<4>; m[2] }";
    assert_eq!(run_to_int(src), 33);
}

#[test]
fn multiword_turbofish_constructor() {
    // The Multiword::<N>(...) constructor form is equivalent to the
    // tuple cast.
    assert_eq!(
        run_to_int("fn main() -> Word { let m = Multiword::<4>(5, 6, 7, 8); m[1] }"),
        6
    );
}

#[test]
fn multiword_single_word_constructor() {
    // Multiword<1> through the constructor form, which the tuple cast
    // cannot express since a one-element tuple is not surface syntax.
    assert_eq!(
        run_to_int("fn main() -> Word { let m = Multiword::<1>(77); m[0] }"),
        77
    );
}

// --- Phase 2: addition and subtraction with correct two's-complement
// unsigned carry and borrow (B19). ---

#[test]
fn multiword_add_no_carry() {
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (100, 200) as Multiword<2>; let b = (50, 25) as Multiword<2>; let s = a + b; s[0] }"),
        150
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (100, 200) as Multiword<2>; let b = (50, 25) as Multiword<2>; let s = a + b; s[1] }"),
        225
    );
}

#[test]
fn multiword_add_unsigned_carry_propagates() {
    // (-1, 0) is unsigned 2^64 - 1 in the low limb; adding 1 carries
    // into the high limb, giving (0, 1). This is the correct unsigned
    // carry, which the signed-overflow flag does not provide.
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (-1, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; let s = a + b; s[0] }"),
        0
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (-1, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; let s = a + b; s[1] }"),
        1
    );
}

#[test]
fn multiword_add_no_spurious_signed_carry() {
    // The signed-overflow counterexample. a = (Word::MAX, 0) is the
    // integer 2^63 - 1, so a + 1 = 2^63, correctly (Word::MIN, 0). A
    // naive signed-flag cascade would wrongly propagate a carry and
    // give (Word::MIN, 1); the high limb MUST be 0.
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (9223372036854775807, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; let s = a + b; s[1] }"),
        0
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (9223372036854775807, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; let s = a + b; s[0] }"),
        i64::MIN
    );
}

#[test]
fn multiword_sub_no_borrow() {
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (150, 225) as Multiword<2>; let b = (50, 25) as Multiword<2>; let d = a - b; d[0] }"),
        100
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (150, 225) as Multiword<2>; let b = (50, 25) as Multiword<2>; let d = a - b; d[1] }"),
        200
    );
}

#[test]
fn multiword_sub_borrow_propagates() {
    // (0, 5) - (1, 0): the low limb underflows, borrowing from the high
    // limb, giving low = -1 (all ones) and high = 5 - 1 = 4.
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (0, 5) as Multiword<2>; let b = (1, 0) as Multiword<2>; let d = a - b; d[0] }"),
        -1
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (0, 5) as Multiword<2>; let b = (1, 0) as Multiword<2>; let d = a - b; d[1] }"),
        4
    );
}

#[test]
fn multiword_four_word_add_carry_chain() {
    // A carry that ripples across multiple limbs: (-1, -1, -1, 0) is
    // 2^192 - 1 in the low three limbs; adding 1 clears them and sets
    // the fourth limb to 1.
    let src = "fn main() -> Word { \
        let a = (-1, -1, -1, 0) as Multiword<4>; \
        let b = (1, 0, 0, 0) as Multiword<4>; \
        let s = a + b; \
        s[0] + s[1] + s[2] + s[3] }";
    // s = (0, 0, 0, 1) -> sum of digits is 1.
    assert_eq!(run_to_int(src), 1);
}

// --- Fixed-point form: Multiword<N, F> carries F fractional bits over
// the same N-word layout; Multiword<N> is Multiword<N, 0>, the integer
// case (B19). ---

fn compile_fails(src: &str) -> bool {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).is_err()
}

#[test]
fn multiword_fixed_point_annotation_constructs_and_indexes() {
    // The fraction-bit count is a type annotation over the same layout,
    // so construction and digit indexing behave as for the integer form.
    assert_eq!(
        run_to_int("fn main() -> Word { let m = Multiword::<2, 32>(7, 3); m[1] }"),
        3
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (7, 3) as Multiword<2, 32>; m[0] }"),
        7
    );
}

#[test]
fn multiword_fixed_point_add_same_scale() {
    // Adding two same-scale fixed-point values is the integer add of the
    // underlying words (phase 2 is scale-independent).
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (100, 0) as Multiword<2, 16>; let b = (50, 0) as Multiword<2, 16>; let s = a + b; s[0] }"),
        150
    );
}

#[test]
fn multiword_different_scales_do_not_mix() {
    // Multiword<2> (integer, F = 0) and Multiword<2, 16> are distinct
    // types and cannot be combined without an explicit cast.
    assert!(compile_fails(
        "fn main() -> Word { let a = (1, 0) as Multiword<2>; let b = (1, 0) as Multiword<2, 16>; let s = a + b; s[0] }"
    ));
}
