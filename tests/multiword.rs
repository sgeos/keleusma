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
        run_to_int(
            "fn main() -> Word { let a = (100, 200) as Multiword<2>; let b = (50, 25) as Multiword<2>; let s = a + b; s[0] }"
        ),
        150
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (100, 200) as Multiword<2>; let b = (50, 25) as Multiword<2>; let s = a + b; s[1] }"
        ),
        225
    );
}

#[test]
fn multiword_add_unsigned_carry_propagates() {
    // (-1, 0) is unsigned 2^64 - 1 in the low limb; adding 1 carries
    // into the high limb, giving (0, 1). This is the correct unsigned
    // carry, which the signed-overflow flag does not provide.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (-1, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; let s = a + b; s[0] }"
        ),
        0
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (-1, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; let s = a + b; s[1] }"
        ),
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
        run_to_int(
            "fn main() -> Word { let a = (9223372036854775807, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; let s = a + b; s[1] }"
        ),
        0
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (9223372036854775807, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; let s = a + b; s[0] }"
        ),
        i64::MIN
    );
}

#[test]
fn multiword_sub_no_borrow() {
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (150, 225) as Multiword<2>; let b = (50, 25) as Multiword<2>; let d = a - b; d[0] }"
        ),
        100
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (150, 225) as Multiword<2>; let b = (50, 25) as Multiword<2>; let d = a - b; d[1] }"
        ),
        200
    );
}

#[test]
fn multiword_sub_borrow_propagates() {
    // (0, 5) - (1, 0): the low limb underflows, borrowing from the high
    // limb, giving low = -1 (all ones) and high = 5 - 1 = 4.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, 5) as Multiword<2>; let b = (1, 0) as Multiword<2>; let d = a - b; d[0] }"
        ),
        -1
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, 5) as Multiword<2>; let b = (1, 0) as Multiword<2>; let d = a - b; d[1] }"
        ),
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

// --- Phase 1 boundary: a tuple casts to Multiword<N> only when it has
// exactly N Word elements. A wrong arity or a non-Word element is
// rejected at compile time (B19). ---

#[test]
fn multiword_cast_rejects_wrong_tuple_arity() {
    // Too few and too many tuple elements are both rejected.
    assert!(compile_fails(
        "fn main() -> Word { let m = (1, 2, 3) as Multiword<4>; m[0] }"
    ));
    assert!(compile_fails(
        "fn main() -> Word { let m = (1, 2, 3) as Multiword<2>; m[0] }"
    ));
    // The turbofish constructor desugars to the same cast, so a wrong
    // argument count is rejected too.
    assert!(compile_fails(
        "fn main() -> Word { let m = Multiword::<4>(1, 2, 3); m[0] }"
    ));
}

#[test]
fn multiword_cast_rejects_non_word_element() {
    // A Float element cannot pack into a Multiword word.
    assert!(compile_fails(
        "fn main() -> Word { let m = (1, 2.0) as Multiword<2>; m[0] }"
    ));
}

// A program that compiles and verifies but whose execution traps
// returns an error from `call`. Used for the division-by-zero test.
fn run_traps(src: &str) -> bool {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.call(&[]).is_err()
}

fn run_to_bool(src: &str) -> bool {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]).expect("call") {
        VmState::Finished(Value::Bool(b)) => b,
        other => panic!("expected a finished bool, got {:?}", other),
    }
}

// --- Phase 2, remainder: the six comparison operators. A Multiword<N>
// is little-endian two's complement, so ordering is decided by the most
// significant differing limb, the top limb signed and the lower limbs
// unsigned (B19). ---

#[test]
fn multiword_eq_and_ne() {
    assert!(run_to_bool(
        "fn main() -> bool { let a = (5, 7) as Multiword<2>; let b = (5, 7) as Multiword<2>; a == b }"
    ));
    assert!(!run_to_bool(
        "fn main() -> bool { let a = (5, 7) as Multiword<2>; let b = (5, 8) as Multiword<2>; a == b }"
    ));
    assert!(run_to_bool(
        "fn main() -> bool { let a = (5, 7) as Multiword<2>; let b = (6, 7) as Multiword<2>; a != b }"
    ));
    assert!(!run_to_bool(
        "fn main() -> bool { let a = (5, 7) as Multiword<2>; let b = (5, 7) as Multiword<2>; a != b }"
    ));
}

#[test]
fn multiword_ordering_decided_by_high_limb() {
    // The most significant limb dominates regardless of the low limb.
    assert!(run_to_bool(
        "fn main() -> bool { let a = (100, 1) as Multiword<2>; let b = (0, 2) as Multiword<2>; a < b }"
    ));
    assert!(run_to_bool(
        "fn main() -> bool { let a = (0, 2) as Multiword<2>; let b = (100, 1) as Multiword<2>; a > b }"
    ));
}

#[test]
fn multiword_ordering_low_limb_is_unsigned() {
    // High limbs equal, so the low limb decides, and it is unsigned. The
    // low limb -1 is 2^64 - 1 unsigned, so (-1, 0) is the larger value.
    // A signed low-limb compare would wrongly rank -1 below 1.
    assert!(run_to_bool(
        "fn main() -> bool { let a = (-1, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; a > b }"
    ));
    assert!(!run_to_bool(
        "fn main() -> bool { let a = (-1, 0) as Multiword<2>; let b = (1, 0) as Multiword<2>; a < b }"
    ));
    // Same high limb, plain unsigned low compare.
    assert!(run_to_bool(
        "fn main() -> bool { let a = (5, 3) as Multiword<2>; let b = (9, 3) as Multiword<2>; a < b }"
    ));
}

#[test]
fn multiword_ordering_high_limb_is_signed() {
    // (0, -1) is high limb -1, a negative value near -2^64; it must rank
    // below zero. A signed top-limb compare gives this; an unsigned one
    // would rank -1 as the largest high limb and invert the order.
    assert!(run_to_bool(
        "fn main() -> bool { let a = (0, -1) as Multiword<2>; let b = (0, 0) as Multiword<2>; a < b }"
    ));
    assert!(run_to_bool(
        "fn main() -> bool { let a = (0, 0) as Multiword<2>; let b = (0, -1) as Multiword<2>; a > b }"
    ));
}

#[test]
fn multiword_le_and_ge_include_equality() {
    assert!(run_to_bool(
        "fn main() -> bool { let a = (5, 7) as Multiword<2>; let b = (5, 7) as Multiword<2>; a <= b }"
    ));
    assert!(run_to_bool(
        "fn main() -> bool { let a = (5, 7) as Multiword<2>; let b = (5, 7) as Multiword<2>; a >= b }"
    ));
    assert!(!run_to_bool(
        "fn main() -> bool { let a = (5, 7) as Multiword<2>; let b = (4, 7) as Multiword<2>; a <= b }"
    ));
    assert!(run_to_bool(
        "fn main() -> bool { let a = (5, 7) as Multiword<2>; let b = (4, 7) as Multiword<2>; a >= b }"
    ));
}

#[test]
fn multiword_four_word_ordering() {
    // The two values differ only in the third limb; that limb decides.
    let base = "let a = (9, 9, 5, 0) as Multiword<4>; let b = (0, 0, 6, 0) as Multiword<4>;";
    assert!(run_to_bool(&alloc_src(base, "a < b")));
    assert!(!run_to_bool(&alloc_src(base, "a > b")));
    assert!(run_to_bool(&alloc_src(base, "a != b")));
}

fn alloc_src(bindings: &str, expr: &str) -> String {
    format!("fn main() -> bool {{ {} {} }}", bindings, expr)
}

#[test]
fn multiword_fixed_point_compare_same_scale() {
    // Comparison is scale-independent: same-scale fixed-point values
    // compare by their underlying words.
    assert!(run_to_bool(
        "fn main() -> bool { let a = (100, 0) as Multiword<2, 16>; let b = (50, 0) as Multiword<2, 16>; a > b }"
    ));
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
        run_to_int(
            "fn main() -> Word { let a = (100, 0) as Multiword<2, 16>; let b = (50, 0) as Multiword<2, 16>; let s = a + b; s[0] }"
        ),
        150
    );
}

// --- Phase 3a: integer multiply (F = 0). The result is the low N words
// of the two's-complement product, computed as an unsigned schoolbook
// product with a signed-to-unsigned high-word correction per digit
// product (B19). ---

#[test]
fn multiword_mul_small_no_carry() {
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (6, 0) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a * b; s[0] }"
        ),
        42
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (6, 0) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a * b; s[1] }"
        ),
        0
    );
}

#[test]
fn multiword_mul_cross_term_into_high_word() {
    // (3, 5) is 5 * 2^64 + 3; times 7 is 35 * 2^64 + 21, so the high
    // digit's partial product lands in result word 1.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (3, 5) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a * b; s[0] }"
        ),
        21
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (3, 5) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a * b; s[1] }"
        ),
        35
    );
}

#[test]
fn multiword_mul_high_word_carry_from_digit_product() {
    // 5_000_000_000 squared is 2.5e19, which exceeds 2^64, so the low
    // digit product carries a 1 into result word 1. Low word is
    // 25e18 mod 2^64 = 6553255926290448384.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (5000000000, 0) as Multiword<2>; let b = (5000000000, 0) as Multiword<2>; let s = a * b; s[0] }"
        ),
        6553255926290448384
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (5000000000, 0) as Multiword<2>; let b = (5000000000, 0) as Multiword<2>; let s = a * b; s[1] }"
        ),
        1
    );
}

#[test]
fn multiword_mul_unsigned_high_correction() {
    // The low word -1 is 2^64 - 1 unsigned. Multiplying (-1, 0) by
    // (2, 0) gives the unsigned product (2^64 - 1) * 2 = 2^65 - 2, whose
    // low two words are (-2, 1). This requires the signed-to-unsigned
    // high-word correction: the raw signed high of (-1) * 2 is -1, which
    // would give a wrong high word; the correction yields the right high
    // word 1, and the low word is -2.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (-1, 0) as Multiword<2>; let b = (2, 0) as Multiword<2>; let s = a * b; s[0] }"
        ),
        -2
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (-1, 0) as Multiword<2>; let b = (2, 0) as Multiword<2>; let s = a * b; s[1] }"
        ),
        1
    );
}

#[test]
fn multiword_mul_negative_value() {
    // (-1, -1) is the two's-complement value -1; times 2 is -2, whose
    // two-word representation is (-2, -1). Exercises the multiply on a
    // genuinely negative multi-word value.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (-1, -1) as Multiword<2>; let b = (2, 0) as Multiword<2>; let s = a * b; s[0] }"
        ),
        -2
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (-1, -1) as Multiword<2>; let b = (2, 0) as Multiword<2>; let s = a * b; s[1] }"
        ),
        -1
    );
}

#[test]
fn multiword_mul_identity_and_zero() {
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (123, 456) as Multiword<2>; let b = (1, 0) as Multiword<2>; let s = a * b; s[0] + s[1] }"
        ),
        579
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (123, 456) as Multiword<2>; let b = (0, 0) as Multiword<2>; let s = a * b; s[0] + s[1] }"
        ),
        0
    );
}

#[test]
fn multiword_mul_three_word_scalar() {
    // (2, 3, 4) times the scalar (5, 0, 0) multiplies each digit by 5
    // with no carry, giving (10, 15, 20).
    let base =
        "let a = (2, 3, 4) as Multiword<3>; let b = (5, 0, 0) as Multiword<3>; let s = a * b;";
    assert_eq!(
        run_to_int(&format!("fn main() -> Word {{ {} s[0] }}", base)),
        10
    );
    assert_eq!(
        run_to_int(&format!("fn main() -> Word {{ {} s[1] }}", base)),
        15
    );
    assert_eq!(
        run_to_int(&format!("fn main() -> Word {{ {} s[2] }}", base)),
        20
    );
}

#[test]
fn multiword_mul_is_commutative() {
    // a * b and b * a produce the same low word.
    let ab = run_to_int(
        "fn main() -> Word { let a = (7, 11) as Multiword<2>; let b = (13, 3) as Multiword<2>; let s = a * b; s[0] + s[1] }",
    );
    let ba = run_to_int(
        "fn main() -> Word { let a = (7, 11) as Multiword<2>; let b = (13, 3) as Multiword<2>; let s = b * a; s[0] + s[1] }",
    );
    assert_eq!(ab, ba);
}

#[test]
fn multiword_fixed_point_multiply_small_scale() {
    // Q112.16: 1.0 = 2^16 = 65536, 2.0 = 131072, product 2.0 = 131072.
    // The raw product 2^33 is shifted right by F = 16 to 2^17 = 131072.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (65536, 0) as Multiword<2, 16>; let b = (131072, 0) as Multiword<2, 16>; let s = a * b; s[0] }"
        ),
        131072
    );
}

// --- Phase 3b: fixed-point multiply (F > 0). The result is the full
// 2N-word signed product shifted right by F, truncated to N words. Raw
// words are written directly; a Multiword<2, F> value v represents
// (v[1] * 2^64 + v[0]) / 2^F on the default 64-bit runtime (B19). ---

#[test]
fn multiword_fixed_mul_rounds_toward_negative_infinity() {
    // The shift is arithmetic, so a negative product floors rather than
    // truncating toward zero. Q127.1: -1.5 = raw -3, times 0.5 = raw 1,
    // exact product -0.75; the raw product -3 arithmetic-shifted right by
    // 1 is -2 (floor), representing -1.0, not the -1 (round-toward-zero)
    // that would represent -0.5.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (-3, -1) as Multiword<2, 1>; let b = (1, 0) as Multiword<2, 1>; let s = a * b; s[0] }"
        ),
        -2
    );
    // The positive counterpart: 1.5 * 0.5 raw is 3, shifted right by 1 is
    // 1, representing 0.5.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (3, 0) as Multiword<2, 1>; let b = (1, 0) as Multiword<2, 1>; let s = a * b; s[0] }"
        ),
        1
    );
}

#[test]
fn multiword_fixed_mul_single_word() {
    // N = 1 fixed-point multiply. Q56.8 in one word: 1.0 = 256, 2.0 =
    // 512, product 2.0 = 512. The raw product 2^17 is shifted right by
    // F = 8 to 2^9 = 512.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = Multiword::<1, 8>(256); let b = Multiword::<1, 8>(512); let s = a * b; s[0] }"
        ),
        512
    );
}

#[test]
fn multiword_fixed_mul_integer_scale() {
    // Q64.64: a = (0, 2) is 2.0, b = (0, 3) is 3.0, product 6.0 = (0, 6).
    // F = 64 is a whole-word shift (q = 1, r = 0).
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, 2) as Multiword<2, 64>; let b = (0, 3) as Multiword<2, 64>; let s = a * b; s[0] }"
        ),
        0
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, 2) as Multiword<2, 64>; let b = (0, 3) as Multiword<2, 64>; let s = a * b; s[1] }"
        ),
        6
    );
}

#[test]
fn multiword_fixed_mul_bit_scale() {
    // Q96.32: 1.5 = 1.5 * 2^32 = 6442450944, 2.0 = 8589934592, product
    // 3.0 = 3 * 2^32 = 12884901888. F = 32 is a sub-word shift (r = 32).
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (6442450944, 0) as Multiword<2, 32>; let b = (8589934592, 0) as Multiword<2, 32>; let s = a * b; s[0] }"
        ),
        12884901888
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (6442450944, 0) as Multiword<2, 32>; let b = (8589934592, 0) as Multiword<2, 32>; let s = a * b; s[1] }"
        ),
        0
    );
}

#[test]
fn multiword_fixed_mul_fractional_result() {
    // Q96.32: 0.5 = 2^31 = 2147483648, squared is 0.25 = 2^30 =
    // 1073741824. The product's fractional bits are shifted back down.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (2147483648, 0) as Multiword<2, 32>; let b = (2147483648, 0) as Multiword<2, 32>; let s = a * b; s[0] }"
        ),
        1073741824
    );
}

#[test]
fn multiword_fixed_mul_negative() {
    // Q64.64: -2.0 = (0, -2), times 3.0 = (0, 3), product -6.0 = (0, -6).
    // Exercises the product-level signed correction on one negative
    // operand.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, -2) as Multiword<2, 64>; let b = (0, 3) as Multiword<2, 64>; let s = a * b; s[0] }"
        ),
        0
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, -2) as Multiword<2, 64>; let b = (0, 3) as Multiword<2, 64>; let s = a * b; s[1] }"
        ),
        -6
    );
}

#[test]
fn multiword_fixed_mul_both_negative() {
    // Q64.64: (-2.0) * (-3.0) = 6.0 = (0, 6). Both product-level
    // corrections apply and cancel the spurious high bits.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, -2) as Multiword<2, 64>; let b = (0, -3) as Multiword<2, 64>; let s = a * b; s[1] }"
        ),
        6
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, -2) as Multiword<2, 64>; let b = (0, -3) as Multiword<2, 64>; let s = a * b; s[0] }"
        ),
        0
    );
}

#[test]
fn multiword_fixed_mul_at_bound_compiles() {
    // F = N * word_bits = 128 is the maximum admissible fraction-bit
    // count for Multiword<2> on the 64-bit runtime; it must compile.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, 0) as Multiword<2, 128>; let b = (0, 0) as Multiword<2, 128>; let s = a * b; s[0] }"
        ),
        0
    );
}

#[test]
fn multiword_fixed_mul_over_bound_rejected() {
    // F greater than N * word_bits (128 for Multiword<2> on 64-bit) has
    // more fraction bits than the value can hold and is rejected.
    assert!(compile_fails(
        "fn main() -> Word { let a = (1, 0) as Multiword<2, 200>; let b = (1, 0) as Multiword<2, 200>; let s = a * b; s[0] }"
    ));
}

// --- Phase 4a: integer divide and modulo (F = 0). Signed with
// truncation toward zero; the quotient takes the sign of the operand
// exclusive-or and the remainder the sign of the dividend (B19). ---

#[test]
fn multiword_div_and_mod_basic() {
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (100, 0) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a / b; s[0] }"
        ),
        14
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (100, 0) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a % b; s[0] }"
        ),
        2
    );
}

#[test]
fn multiword_div_exact_and_smaller_dividend() {
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (100, 0) as Multiword<2>; let b = (4, 0) as Multiword<2>; let s = a / b; s[0] }"
        ),
        25
    );
    // Dividend smaller than divisor: quotient 0, remainder is the dividend.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (5, 0) as Multiword<2>; let b = (10, 0) as Multiword<2>; let s = a / b; s[0] }"
        ),
        0
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (5, 0) as Multiword<2>; let b = (10, 0) as Multiword<2>; let s = a % b; s[0] }"
        ),
        5
    );
}

#[test]
fn multiword_div_spans_two_words() {
    // 2^64 / 2 = 2^63, which is the bit pattern i64::MIN in the low word
    // with a zero high word. Exercises the division across a word
    // boundary with an unsigned dividend larger than a single word.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, 1) as Multiword<2>; let b = (2, 0) as Multiword<2>; let s = a / b; s[1] }"
        ),
        0
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, 1) as Multiword<2>; let b = (2, 0) as Multiword<2>; let s = a / b; s[0] }"
        ),
        i64::MIN
    );
}

#[test]
fn multiword_div_negative_dividend() {
    // -100 / 7 = -14 (toward zero); -100 % 7 = -2 (sign of dividend).
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (-100, -1) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a / b; s[0] }"
        ),
        -14
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (-100, -1) as Multiword<2>; let b = (7, 0) as Multiword<2>; let s = a % b; s[0] }"
        ),
        -2
    );
}

#[test]
fn multiword_div_negative_divisor() {
    // 100 / -7 = -14; 100 % -7 = 2 (remainder keeps the dividend's sign).
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (100, 0) as Multiword<2>; let b = (-7, -1) as Multiword<2>; let s = a / b; s[0] }"
        ),
        -14
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (100, 0) as Multiword<2>; let b = (-7, -1) as Multiword<2>; let s = a % b; s[0] }"
        ),
        2
    );
}

#[test]
fn multiword_div_both_negative() {
    // -100 / -7 = 14; -100 % -7 = -2.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (-100, -1) as Multiword<2>; let b = (-7, -1) as Multiword<2>; let s = a / b; s[0] }"
        ),
        14
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (-100, -1) as Multiword<2>; let b = (-7, -1) as Multiword<2>; let s = a % b; s[0] }"
        ),
        -2
    );
}

#[test]
fn multiword_div_three_word() {
    // (0, 0, 6) is 6 * 2^128; divided by (3, 0, 0) = 3 gives 2 * 2^128,
    // so the high word of the quotient is 2 and the rest zero.
    let base =
        "let a = (0, 0, 6) as Multiword<3>; let b = (3, 0, 0) as Multiword<3>; let s = a / b;";
    assert_eq!(
        run_to_int(&format!("fn main() -> Word {{ {} s[2] }}", base)),
        2
    );
    assert_eq!(
        run_to_int(&format!("fn main() -> Word {{ {} s[0] }}", base)),
        0
    );
}

#[test]
fn multiword_div_by_zero_traps() {
    // A zero divisor traps at runtime as a division by zero, the same
    // bounded fault as the scalar integer divide.
    assert!(run_traps(
        "fn main() -> Word { let a = (100, 0) as Multiword<2>; let b = (0, 0) as Multiword<2>; let s = a / b; s[0] }"
    ));
    assert!(run_traps(
        "fn main() -> Word { let a = (100, 0) as Multiword<2>; let b = (0, 0) as Multiword<2>; let s = a % b; s[0] }"
    ));
}

// --- Phase 4b: fixed-point divide (F > 0) pre-shifts the dividend left
// by F; fixed-point modulo is the scale-preserving raw remainder (B19).
// Q96.32 raw units: 1.0 = 2^32 = 4294967296 (B19). ---

#[test]
fn multiword_fixed_div_basic() {
    // Q96.32: 6.0 / 2.0 = 3.0. raw 6*2^32 / (2*2^32) pre-shifts to
    // (6*2^32 << 32) / (2*2^32) = 3*2^32 = 12884901888.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (25769803776, 0) as Multiword<2, 32>; let b = (8589934592, 0) as Multiword<2, 32>; let s = a / b; s[0] }"
        ),
        12884901888
    );
}

#[test]
fn multiword_fixed_div_fractional_result() {
    // Q96.32: 1.0 / 2.0 = 0.5. (2^32 << 32) / 2^33 = 2^31 = 2147483648.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (4294967296, 0) as Multiword<2, 32>; let b = (8589934592, 0) as Multiword<2, 32>; let s = a / b; s[0] }"
        ),
        2147483648
    );
}

#[test]
fn multiword_fixed_div_negative() {
    // Q96.32: -6.0 / 2.0 = -3.0 = -12884901888.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (-25769803776, -1) as Multiword<2, 32>; let b = (8589934592, 0) as Multiword<2, 32>; let s = a / b; s[0] }"
        ),
        -12884901888
    );
}

#[test]
fn multiword_fixed_div_whole_word_shift() {
    // Q64.64: 6.0 / 2.0 = 3.0. F = 64 is a whole-word dividend shift;
    // 6.0 = (0, 6), 2.0 = (0, 2), result 3.0 = (0, 3).
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, 6) as Multiword<2, 64>; let b = (0, 2) as Multiword<2, 64>; let s = a / b; s[1] }"
        ),
        3
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, 6) as Multiword<2, 64>; let b = (0, 2) as Multiword<2, 64>; let s = a / b; s[0] }"
        ),
        0
    );
}

#[test]
fn multiword_fixed_mod_keeps_scale() {
    // Q96.32: 5.5 % 2.0 = 1.5. The fixed-point remainder keeps the scale,
    // so it is the raw integer modulo with no shift: 5.5*2^32 mod 2.0*2^32
    // = 1.5*2^32 = 6442450944.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (23622320128, 0) as Multiword<2, 32>; let b = (8589934592, 0) as Multiword<2, 32>; let s = a % b; s[0] }"
        ),
        6442450944
    );
    // Q64.64: 5.0 % 3.0 = 2.0, across the word boundary. (0,5) mod (0,3)
    // = (0,2).
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (0, 5) as Multiword<2, 64>; let b = (0, 3) as Multiword<2, 64>; let s = a % b; s[1] }"
        ),
        2
    );
}

#[test]
fn multiword_fixed_div_by_zero_traps() {
    assert!(run_traps(
        "fn main() -> Word { let a = (4294967296, 0) as Multiword<2, 32>; let b = (0, 0) as Multiword<2, 32>; let s = a / b; s[0] }"
    ));
}

#[test]
fn multiword_nested_operations_do_not_alias_scratch_locals() {
    // Each lowered operation declares its own scratch locals, and
    // declare_local hands out a fresh slot every call, so a nested
    // expression whose subexpressions are themselves multi-word
    // operations must not clobber one another. `(a + b) + c` nests one
    // add inside another, and the surrounding comparison nests two adds
    // under one comparison; both must evaluate cleanly.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { \
             let a = (1, 0) as Multiword<2>; \
             let b = (2, 0) as Multiword<2>; \
             let c = (3, 0) as Multiword<2>; \
             let s = (a + b) + c; s[0] }"
        ),
        6
    );
    // Two independent sums compared in one expression.
    assert!(run_to_bool(
        "fn main() -> bool { \
         let a = (1, 0) as Multiword<2>; \
         let b = (2, 0) as Multiword<2>; \
         let c = (3, 0) as Multiword<2>; \
         (a + b) < (a + c) }"
    ));
}

#[test]
fn multiword_different_scales_do_not_mix() {
    // Multiword<2> (integer, F = 0) and Multiword<2, 16> are distinct
    // types and cannot be combined without an explicit cast.
    assert!(compile_fails(
        "fn main() -> Word { let a = (1, 0) as Multiword<2>; let b = (1, 0) as Multiword<2, 16>; let s = a + b; s[0] }"
    ));
}
