// This suite pins `Multiword<N, F>` results at the default 64-bit host
// word width, so it is excluded when a `narrow-word`/`narrow-address`/
// `narrow-float` feature lowers `Target::host()` to a narrower width (the
// "narrowest wins" rule of the framing-width features). Under a narrow
// width the compiler lowers multi-word arithmetic to that width and these
// 64-bit-word expectations no longer hold; narrow-width multi-word
// coverage lives in `tests/narrow_vm.rs`. Without this guard, enabling
// every feature at once (for example `cargo test --all-features`, which
// turns on `narrow-word-8`) compiles this suite at an 8-bit word and it
// fails as a false negative.
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32",
        feature = "narrow-address-8",
        feature = "narrow-address-16",
        feature = "narrow-address-32",
        feature = "narrow-float-32"
    ))
))]
//! `Multiword<N>` fixed-width multi-word integer, phase 1: the type,
//! construction from a tuple literal, and digit indexing (B19). The
//! value is represented as a flat little-endian array of N words, so a
//! `Multiword<N>` built from `(d0, d1, ..., d_{N-1}) as Multiword<N>`
//! indexes to its digits with `m[i]`, digit 0 being least significant.

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify::wcet_whole_chunk;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, auto_arena_capacity_for};
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

fn run_to_byte(src: &str) -> u8 {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[]).expect("call") {
        VmState::Finished(Value::Byte(b)) => b,
        other => panic!("expected a finished byte, got {:?}", other),
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
    // (6*2^32 lsl 32) / (2*2^32) = 3*2^32 = 12884901888.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (25769803776, 0) as Multiword<2, 32>; let b = (8589934592, 0) as Multiword<2, 32>; let s = a / b; s[0] }"
        ),
        12884901888
    );
}

#[test]
fn multiword_fixed_div_fractional_result() {
    // Q96.32: 1.0 / 2.0 = 0.5. (2^32 lsl 32) / 2^33 = 2^31 = 2147483648.
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
fn multiword_fixed_div_truncates_toward_zero() {
    // The fixed-point divide truncates toward zero, matching the scalar
    // Fixed divide (which computes (x lsl F) / y with Rust's truncating
    // division), not toward negative infinity. Q60.4: -1.0 = raw -16,
    // 3.0 = raw 48, exact ratio -1/3 = -0.333. The raw result is
    // (16 lsl 4) / 48 = 256 / 48 = 5 in magnitude, sign-reapplied to -5,
    // representing -0.3125, the truncated-toward-zero value; a floor
    // would give -6 (-0.375).
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = Multiword::<1, 4>(0 - 16); let b = Multiword::<1, 4>(48); let s = a / b; s[0] }"
        ),
        -5
    );
    // The positive counterpart truncates the same way: 1.0 / 3.0 raw is
    // 5, representing 0.3125.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = Multiword::<1, 4>(16); let b = Multiword::<1, 4>(48); let s = a / b; s[0] }"
        ),
        5
    );
}

// --- Phase 5: shift operators, assembly-mnemonic keyword shifts. `lsl` logical left,
// `asl` arithmetic left (value x * 2^k), `lsr` logical (zero-fill) right,
// `asr` arithmetic (sign-preserving) right, with a compile-time-constant
// amount. Word and Multiword values (B19). ---

#[test]
fn scalar_word_shifts() {
    assert_eq!(run_to_int("fn main() -> Word { 5 lsl 2 }"), 20);
    assert_eq!(run_to_int("fn main() -> Word { 20 lsr 2 }"), 5);
    // The arithmetic right shift `asr` preserves the sign.
    assert_eq!(
        run_to_int("fn main() -> Word { let x = 0 - 8; x asr 1 }"),
        -4
    );
    // The logical right shift `lsr` zero-fills, so -8 becomes a large
    // positive.
    assert_eq!(
        run_to_int("fn main() -> Word { let x = 0 - 8; x lsr 1 }"),
        9223372036854775804
    );
    // 1 lsl 63 is 2^63, which wraps to the most negative Word.
    assert_eq!(run_to_int("fn main() -> Word { 1 lsl 63 }"), i64::MIN);
    // Shift by zero is the identity.
    assert_eq!(run_to_int("fn main() -> Word { 5 lsl 0 }"), 5);
}

#[test]
fn scalar_arithmetic_left_shift_bare_wraps_like_logical() {
    // The bare arithmetic left shift `asl` produces the same value as the
    // logical left shift `lsl`; the difference appears only under the
    // checked-arithmetic construct (overflow capture), tested separately.
    assert_eq!(run_to_int("fn main() -> Word { 5 asl 2 }"), 20);
    // 1 asl 63 wraps to the most negative Word, exactly as 1 lsl 63.
    assert_eq!(run_to_int("fn main() -> Word { 1 asl 63 }"), i64::MIN);
}

#[test]
fn scalar_arithmetic_left_shift_captures_overflow() {
    // `asl` is `x * 2^k`, so it admits the checked-arithmetic arms. A
    // shift that fits takes the ok arm.
    assert_eq!(
        run_to_int("fn main() -> Word { 3 asl 2 { ok(v) => v, overflow(h, l) => 0 } }"),
        12
    );
    // 2^62 asl 1 is 2^63, one past Word::MAX, so it overflows.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let x = 4611686018427387904; x asl 1 { ok(v) => v, overflow(h, l) => 999 } }"
        ),
        999
    );
    // The overflow arm binds the two halves of the product; the low half
    // is the wrapped result, 2^63 mod 2^64 = i64::MIN.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let x = 4611686018427387904; x asl 1 { ok(v) => v, overflow(h, l) => l } }"
        ),
        i64::MIN
    );
    // saturate_max resolves in the overflow arm.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let x = 4611686018427387904; x asl 1 { ok(v) => v, overflow(h, l) => saturate_max } }"
        ),
        i64::MAX
    );
}

#[test]
fn scalar_arithmetic_left_shift_captures_underflow() {
    // -2^62 asl 2 is -2^64, past Word::MIN, so it underflows.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let x = 0 - 4611686018427387904; x asl 2 { ok(v) => v, underflow(h, l) => 0 - 1 } }"
        ),
        -1
    );
}

#[test]
fn scalar_shift_precedence_below_additive() {
    // `0 - 8 asr 1` parses as `(0 - 8) asr 1`, not `0 - (8 asr 1)`.
    assert_eq!(run_to_int("fn main() -> Word { 0 - 8 asr 1 }"), -4);
}

#[test]
fn scalar_shift_rejects_out_of_range_literal_amount() {
    // A literal amount at or beyond the value width is rejected at
    // compile time. A variable amount is now admissible (see the
    // scalar_variable_shift tests) and is masked to the word width at
    // runtime, so only a constant out-of-range literal is a compile
    // error.
    assert!(compile_fails("fn main() -> Word { 5 lsl 64 }"));
    assert!(compile_fails(
        "fn main() -> Word { let x = 0 - 1; x asr 64 }"
    ));
}

#[test]
fn scalar_variable_shift_word() {
    // A non-literal amount compiles and shifts at runtime.
    assert_eq!(run_to_int("fn main() -> Word { let k = 2; 5 lsl k }"), 20);
    assert_eq!(run_to_int("fn main() -> Word { let k = 3; 40 lsr k }"), 5);
    // Arithmetic right shift preserves the sign.
    assert_eq!(
        run_to_int("fn main() -> Word { let k = 1; let x = 0 - 8; x asr k }"),
        -4
    );
    // Logical right shift zero-fills, so a negative value becomes a large
    // positive one: -8 as unsigned, shifted right by one.
    assert_eq!(
        run_to_int("fn main() -> Word { let k = 1; let x = 0 - 8; x lsr k }"),
        9223372036854775804
    );
    // The c == 0 identity branch of the variable logical right shift.
    assert_eq!(run_to_int("fn main() -> Word { let k = 0; 5 lsr k }"), 5);
    // The runtime count is masked to the word width, so 64 wraps to 0.
    assert_eq!(run_to_int("fn main() -> Word { let k = 64; 5 lsl k }"), 5);
}

#[test]
fn byte_shift_constant_and_variable() {
    // A Byte shifts at the byte width; the left shift truncates to eight
    // bits, so 200 lsl 1 wraps modulo 256 to 144.
    assert_eq!(run_to_byte("fn main() -> Byte { 5Byte lsl 1 }"), 10);
    assert_eq!(run_to_byte("fn main() -> Byte { 200Byte lsl 1 }"), 144);
    // A Byte is unsigned, so its arithmetic and logical right shifts
    // coincide and never sign-extend.
    assert_eq!(run_to_byte("fn main() -> Byte { 255Byte lsr 1 }"), 127);
    assert_eq!(run_to_byte("fn main() -> Byte { 255Byte asr 1 }"), 127);
    // A variable amount is admissible for a Byte as well.
    assert_eq!(
        run_to_byte("fn main() -> Byte { let k = 2; 5Byte lsl k }"),
        20
    );
    assert_eq!(
        run_to_byte("fn main() -> Byte { let k = 1; 254Byte lsr k }"),
        127
    );
}

#[test]
fn byte_bitwise_and_complement() {
    assert_eq!(run_to_byte("fn main() -> Byte { 12Byte band 10Byte }"), 8);
    assert_eq!(run_to_byte("fn main() -> Byte { 12Byte bor 10Byte }"), 14);
    assert_eq!(run_to_byte("fn main() -> Byte { 12Byte bxor 10Byte }"), 6);
    // Complement is within the byte width, so bnot 0 is 255 and bnot 5
    // is 250 (0xFA), not the word-width -1/-6.
    assert_eq!(run_to_byte("fn main() -> Byte { bnot 0Byte }"), 255);
    assert_eq!(run_to_byte("fn main() -> Byte { bnot 5Byte }"), 250);
}

#[test]
fn multiword_shift_left_within_and_across_words() {
    // Within a word: (1, 0) lsl 1 = (2, 0).
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (1, 0) as Multiword<2>; let s = m lsl 1; s[0] }"),
        2
    );
    // Across a word: (5, 0) lsl 64 moves the low word into the high word.
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (5, 0) as Multiword<2>; let s = m lsl 64; s[1] }"),
        5
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (5, 0) as Multiword<2>; let s = m lsl 64; s[0] }"),
        0
    );
    // The arithmetic left shift `asl` on Multiword produces the same
    // value (Multiword wraps; it has no overflow capture).
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (1, 0) as Multiword<2>; let s = m asl 1; s[0] }"),
        2
    );
}

#[test]
fn multiword_shift_right_arithmetic_vs_logical() {
    // (0, -1) is the value -2^64. The arithmetic right shift `asr` by 1
    // gives -2^63 = (i64::MIN, -1): the sign fills the vacated top bit.
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (0, -1) as Multiword<2>; let s = m asr 1; s[1] }"),
        -1
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (0, -1) as Multiword<2>; let s = m asr 1; s[0] }"),
        i64::MIN
    );
    // The logical right shift `lsr` by 1 zero-fills the top bit, giving
    // the high word i64::MAX and the low word i64::MIN.
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (0, -1) as Multiword<2>; let s = m lsr 1; s[1] }"),
        i64::MAX
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (0, -1) as Multiword<2>; let s = m lsr 1; s[0] }"),
        i64::MIN
    );
}

#[test]
fn multiword_shift_right_whole_word() {
    // (0, 1) is 2^64, positive; lsr 64 gives 1 = (1, 0). The value is
    // positive, so the logical and arithmetic shifts agree.
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (0, 1) as Multiword<2>; let s = m lsr 64; s[0] }"),
        1
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let m = (0, 1) as Multiword<2>; let s = m lsr 64; s[1] }"),
        0
    );
}

#[test]
fn multiword_shift_rejects_out_of_range_literal_amount() {
    // A literal amount must be within the value's total bit width (128
    // here); a variable amount is admissible and shifts everything out
    // when it meets or exceeds the width (see multiword_variable_shift).
    assert!(compile_fails(
        "fn main() -> Word { let m = (1, 0) as Multiword<2>; let s = m lsl 128; s[0] }"
    ));
}

#[test]
fn multiword_variable_shift() {
    // Each case mirrors a constant-amount case above with the amount
    // bound to a variable, so the runtime lowering is checked against the
    // unrolled constant lowering as an oracle.
    // Left within a word: (1, 0) lsl 1 = (2, 0).
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 1; let m = (1, 0) as Multiword<2>; let s = m lsl k; s[0] }"
        ),
        2
    );
    // Left across a word: (5, 0) lsl 64 = (0, 5).
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 64; let m = (5, 0) as Multiword<2>; let s = m lsl k; s[1] }"
        ),
        5
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 64; let m = (5, 0) as Multiword<2>; let s = m lsl k; s[0] }"
        ),
        0
    );
    // Left across a word with a bit offset: (1, 0) lsl 65 sets bit 65,
    // which is bit 1 of the high word, so the high word is 2.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 65; let m = (1, 0) as Multiword<2>; let s = m lsl k; s[1] }"
        ),
        2
    );
    // Arithmetic left equals logical left on a Multiword.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 1; let m = (1, 0) as Multiword<2>; let s = m asl k; s[0] }"
        ),
        2
    );
    // Arithmetic right fills the vacated top with the sign word.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 1; let m = (0, 0 - 1) as Multiword<2>; let s = m asr k; s[1] }"
        ),
        -1
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 1; let m = (0, 0 - 1) as Multiword<2>; let s = m asr k; s[0] }"
        ),
        i64::MIN
    );
    // Logical right zero-fills the vacated top.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 1; let m = (0, 0 - 1) as Multiword<2>; let s = m lsr k; s[1] }"
        ),
        i64::MAX
    );
    // Logical right by a whole word.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 64; let m = (0, 1) as Multiword<2>; let s = m lsr k; s[0] }"
        ),
        1
    );
    // Shift by zero is the identity (the r == 0, q == 0 path).
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 0; let m = (5, 7) as Multiword<2>; let s = m lsl k; s[1] }"
        ),
        7
    );
    // A count at or beyond the total width shifts everything out.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 200; let m = (1, 0) as Multiword<2>; let s = m lsl k; s[0] }"
        ),
        0
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 200; let m = (1, 0) as Multiword<2>; let s = m lsl k; s[1] }"
        ),
        0
    );
}

#[test]
fn checked_asl_still_requires_constant_amount() {
    // The overflow-checked arithmetic left shift lowers to a multiply by
    // the constant 2^k, which cannot be formed for a runtime amount, so
    // the checked form still rejects a variable amount even though the
    // bare shift now admits one.
    assert!(compile_fails(
        "fn main() -> Word { let k = 2; 3 asl k { ok(v) => v, overflow(h, l) => 0 } }"
    ));
    // The bare variable arithmetic left shift is admissible.
    assert_eq!(run_to_int("fn main() -> Word { let k = 2; 3 asl k }"), 12);
}

#[test]
fn multiword_variable_shift_three_word() {
    // A three-word value exercises the unrolled-over-N path at N = 3.
    // (1, 0, 0) lsl 64 = (0, 1, 0): the low word moves up one limb.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 64; let m = (1, 0, 0) as Multiword<3>; let s = m lsl k; s[1] }"
        ),
        1
    );
    // (0, 0, 8) lsr 64 = (0, 8, 0).
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 64; let m = (0, 0, 8) as Multiword<3>; let s = m lsr k; s[1] }"
        ),
        8
    );
}

#[test]
fn multiword_variable_shift_four_word() {
    // N = 4 (256-bit) exercises the unrolled path at the width the
    // constant tests cover. (1,0,0,0) lsl 128 = (0,0,1,0): two limbs up.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 128; let m = (1, 0, 0, 0) as Multiword<4>; let s = m lsl k; s[2] }"
        ),
        1
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 128; let m = (1, 0, 0, 0) as Multiword<4>; let s = m lsl k; s[0] }"
        ),
        0
    );
    // (0,0,0,8) lsr 192 = (8,0,0,0): three limbs down.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 192; let m = (0, 0, 0, 8) as Multiword<4>; let s = m lsr k; s[0] }"
        ),
        8
    );
}

#[test]
fn multiword_variable_shift_fixed_point() {
    // A fixed-point Multiword<N, F> shifts its raw words identically to
    // the integer form, since a shift is a bit operation independent of
    // the implied binary point. (2, 0) as Q with a variable left shift by
    // one doubles the raw low word to 4.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 1; let m = (2, 0) as Multiword<2, 16>; let s = m lsl k; s[0] }"
        ),
        4
    );
    // A whole-word variable shift moves the raw limb up regardless of F.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 64; let m = (7, 0) as Multiword<2, 16>; let s = m lsl k; s[1] }"
        ),
        7
    );
}

#[test]
fn variable_shift_is_total_for_negative_and_over_large_counts() {
    // Totality is the load-bearing guarantee: a runtime shift count that
    // is negative or at/beyond the value width must produce a value
    // rather than trap. `run_to_int` panics on a VM trap, so a returning
    // call proves totality; the pinned values additionally fix the
    // mask-defined semantics.
    //
    // Scalar: a negative count masks to the word width, so 5 lsl (-1)
    // masks the count to 63 and 5 << 63 keeps only bit 0, giving the most
    // negative Word.
    assert_eq!(
        run_to_int("fn main() -> Word { let k = 0 - 1; 5 lsl k }"),
        i64::MIN
    );
    // Scalar over-large count masks to the word width (1000 mod 64 = 40).
    assert_eq!(
        run_to_int("fn main() -> Word { let k = 1000; 1 lsl k }"),
        1i64 << 40
    );
    // Multiword: a negative or over-large count completes without a trap.
    // An over-large left shift moves every bit out, giving zero.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 1000000; let m = (1, 7) as Multiword<2>; let s = m lsl k; s[0] }"
        ),
        0
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 1000000; let m = (1, 7) as Multiword<2>; let s = m lsl k; s[1] }"
        ),
        0
    );
    // A negative Multiword count is total: this must return rather than
    // trap. The value is the mask-defined result; the test's purpose is
    // to prove the call completes.
    let _neg = run_to_int(
        "fn main() -> Word { let k = 0 - 1; let m = (1, 0) as Multiword<2>; let s = m lsr k; s[0] }",
    );
    // A negative logical right shift is deterministic across runs.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let k = 0 - 5; let m = (9, 3) as Multiword<2>; let s = m lsr k; s[1] }"
        ),
        run_to_int(
            "fn main() -> Word { let k = 0 - 5; let m = (9, 3) as Multiword<2>; let s = m lsr k; s[1] }"
        )
    );
}

/// Whole-chunk WCET of the compiled module's `main`, or `Err` when the
/// bound is not statically provable.
fn main_wcet(src: &str) -> Result<u32, ()> {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let main = module
        .chunks
        .iter()
        .find(|c| c.name == "main")
        .expect("main chunk");
    wcet_whole_chunk(main).map_err(|_| ())
}

/// Worst-case arena capacity of the compiled module, or `Err` when the
/// bound is not statically provable.
fn main_wcmu(src: &str) -> Result<usize, ()> {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    auto_arena_capacity_for(&module, &[]).map_err(|_| ())
}

#[test]
fn variable_shift_bounds_are_finite_and_account_the_unrolled_ops() {
    // The definitive-bound guarantee: a variable shift must have a proven,
    // finite worst-case execution time and memory usage, and because the
    // variable lowering is unrolled over N with no runtime loop, its WCET
    // must be finite and at least the constant lowering's (it emits the
    // extra runtime index and guard opcodes). This audits that the cost
    // model actually counts the extra work rather than reporting the same
    // figure as the constant path.
    const CONST_SHIFT: &str =
        "fn main() -> Word { let m = (1, 3) as Multiword<2>; let s = m lsl 5; s[0] }";
    const VAR_SHIFT: &str =
        "fn main() -> Word { let k = 5; let m = (1, 3) as Multiword<2>; let s = m lsl k; s[0] }";
    let const_wcet = main_wcet(CONST_SHIFT).expect("constant shift WCET is provable");
    let var_wcet = main_wcet(VAR_SHIFT).expect("variable shift WCET is provable");
    assert!(const_wcet > 0 && var_wcet > 0);
    assert!(
        var_wcet > const_wcet,
        "variable shift ({var_wcet}) emits strictly more ops than constant ({const_wcet}); the cost model must reflect them"
    );
    // Both memory bounds are finite; each allocates one Multiword result.
    assert!(main_wcmu(CONST_SHIFT).expect("constant shift WCMU is provable") > 0);
    assert!(main_wcmu(VAR_SHIFT).expect("variable shift WCMU is provable") > 0);
    // The Multiword variable shift's WCET is finite at N = 4 as well.
    assert!(
        main_wcet(
            "fn main() -> Word { let k = 130; let m = (1, 0, 0, 0) as Multiword<4>; let s = m lsr k; s[0] }"
        )
        .is_ok()
    );
}

#[test]
fn byte_variable_shift_masks_to_word_width() {
    // A Byte shift promotes to Word, so the runtime count is masked to
    // the word width, not the byte width. A left shift by eight moves the
    // byte's bits above the low eight, so the truncation to Byte yields
    // zero.
    assert_eq!(
        run_to_byte("fn main() -> Byte { let k = 8; 5Byte lsl k }"),
        0
    );
    // A count of 64 masks to zero, so the shift is the identity.
    assert_eq!(
        run_to_byte("fn main() -> Byte { let k = 64; 5Byte lsl k }"),
        5
    );
    // A right shift by eight also clears the byte.
    assert_eq!(
        run_to_byte("fn main() -> Byte { let k = 8; 200Byte lsr k }"),
        0
    );
}

#[test]
fn nested_generics_parse() {
    // The keyword shift operators removed the earlier symbolic `>>`, so a
    // stacked generic close such as Option<Option<T>> lexes to two plain
    // `>` tokens again with no token-splitting. This confirms it parses.
    assert_eq!(
        run_to_int("fn id(x: Option<Option<Word>>) -> Word { 0 }\nfn main() -> Word { 7 }"),
        7
    );
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

// --- Bitwise operators: `band`, `bor`, `bxor` (binary), `bnot` (unary).
// Scalar operands operate on a single Word; Multiword operands combine
// limb by limb with no cross-limb interaction. Disambiguation is by
// operator name, never by operand type. ---

#[test]
fn scalar_bitwise_and_or_xor() {
    assert_eq!(run_to_int("fn main() -> Word { 12 band 10 }"), 8);
    assert_eq!(run_to_int("fn main() -> Word { 12 bor 10 }"), 14);
    assert_eq!(run_to_int("fn main() -> Word { 12 bxor 10 }"), 6);
}

#[test]
fn scalar_bitwise_not_is_all_ones_complement() {
    // ~0 = -1 and ~(-1) = 0 under two's complement.
    assert_eq!(run_to_int("fn main() -> Word { bnot 0 }"), -1);
    assert_eq!(run_to_int("fn main() -> Word { bnot (0 - 1) }"), 0);
    // ~5 = -6.
    assert_eq!(run_to_int("fn main() -> Word { bnot 5 }"), -6);
}

#[test]
fn scalar_bitwise_precedence_band_below_bxor_below_bor() {
    // `bor` binds loosest, then `bxor`, then `band`: 1 bor 2 band 2
    // parses as 1 bor (2 band 2) = 1 bor 2 = 3.
    assert_eq!(run_to_int("fn main() -> Word { 1 bor 2 band 2 }"), 3);
    // 5 bxor 1 band 1 parses as 5 bxor (1 band 1) = 5 bxor 1 = 4.
    assert_eq!(run_to_int("fn main() -> Word { 5 bxor 1 band 1 }"), 4);
}

#[test]
fn scalar_bitwise_below_comparison() {
    // Comparison binds looser than bitwise, so `1 band 1 == 1` parses
    // as `(1 band 1) == 1`, which is true.
    assert!(run_to_bool("fn main() -> bool { 1 band 1 == 1 }"));
}

#[test]
fn multiword_bitwise_per_limb() {
    // Each limb combines independently; there is no carry or borrow.
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (12, 6) as Multiword<2>; let b = (10, 3) as Multiword<2>; let c = a band b; c[0] }"
        ),
        8
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (12, 6) as Multiword<2>; let b = (10, 3) as Multiword<2>; let c = a band b; c[1] }"
        ),
        2
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (12, 6) as Multiword<2>; let b = (10, 3) as Multiword<2>; let c = a bor b; c[0] }"
        ),
        14
    );
    assert_eq!(
        run_to_int(
            "fn main() -> Word { let a = (12, 6) as Multiword<2>; let b = (10, 3) as Multiword<2>; let c = a bxor b; c[1] }"
        ),
        5
    );
}

#[test]
fn multiword_bitwise_not_per_limb() {
    // ~(0, 0) = (-1, -1) limb by limb.
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (0, 0) as Multiword<2>; let c = bnot a; c[0] }"),
        -1
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (5, 0) as Multiword<2>; let c = bnot a; c[1] }"),
        -1
    );
    assert_eq!(
        run_to_int("fn main() -> Word { let a = (5, 0) as Multiword<2>; let c = bnot a; c[0] }"),
        -6
    );
}

#[test]
fn bitwise_rejects_non_integer_operands() {
    // Bitwise operators require Word or Multiword operands; a Bool is
    // rejected rather than silently reinterpreted.
    assert!(compile_fails("fn main() -> Word { true band 1 }"));
    assert!(compile_fails("fn main() -> Word { bnot true }"));
}

// --- Boolean operators: eager `and`, `or`, `xor` and unary `not`, plus
// the short-circuit control forms `andalso`, `orelse`. Eager forms
// always evaluate both operands; the short-circuit forms may skip the
// right operand. In a pure total context the value is identical; the
// tests below pin the value truth tables and operator precedence. ---

#[test]
fn boolean_eager_and_truth_table() {
    assert!(run_to_bool("fn main() -> bool { true and true }"));
    assert!(!run_to_bool("fn main() -> bool { true and false }"));
    assert!(!run_to_bool("fn main() -> bool { false and true }"));
    assert!(!run_to_bool("fn main() -> bool { false and false }"));
}

#[test]
fn boolean_eager_or_truth_table() {
    assert!(run_to_bool("fn main() -> bool { true or true }"));
    assert!(run_to_bool("fn main() -> bool { true or false }"));
    assert!(run_to_bool("fn main() -> bool { false or true }"));
    assert!(!run_to_bool("fn main() -> bool { false or false }"));
}

#[test]
fn boolean_xor_truth_table() {
    assert!(!run_to_bool("fn main() -> bool { true xor true }"));
    assert!(run_to_bool("fn main() -> bool { true xor false }"));
    assert!(run_to_bool("fn main() -> bool { false xor true }"));
    assert!(!run_to_bool("fn main() -> bool { false xor false }"));
}

#[test]
fn boolean_not_and_double_not() {
    assert!(!run_to_bool("fn main() -> bool { not true }"));
    assert!(run_to_bool("fn main() -> bool { not false }"));
    assert!(run_to_bool("fn main() -> bool { not not true }"));
}

#[test]
fn boolean_short_circuit_truth_table_matches_eager() {
    // andalso / orelse produce the same value as and / or.
    assert!(run_to_bool("fn main() -> bool { true andalso true }"));
    assert!(!run_to_bool("fn main() -> bool { true andalso false }"));
    assert!(!run_to_bool("fn main() -> bool { false andalso true }"));
    assert!(run_to_bool("fn main() -> bool { false orelse true }"));
    assert!(!run_to_bool("fn main() -> bool { false orelse false }"));
    assert!(run_to_bool("fn main() -> bool { true orelse false }"));
}

#[test]
fn boolean_precedence_and_binds_tighter_than_or() {
    // `false or true and false` parses as `false or (true and false)`
    // = `false or false` = false. If `and` did not bind tighter than
    // `or`, it would parse as `(false or true) and false` = false too,
    // so use an asymmetric case to distinguish.
    // `true or false and false` = `true or (false and false)` = true.
    // The mis-grouping `(true or false) and false` would be false.
    assert!(run_to_bool("fn main() -> bool { true or false and false }"));
}

#[test]
fn boolean_precedence_comparison_binds_tighter_than_and() {
    // `1 == 1 and 2 == 2` parses as `(1 == 1) and (2 == 2)` = true.
    assert!(run_to_bool("fn main() -> bool { 1 == 1 and 2 == 2 }"));
}

#[test]
fn boolean_rejects_non_bool_operands() {
    assert!(compile_fails("fn main() -> bool { 1 and true }"));
    assert!(compile_fails("fn main() -> bool { true xor 3 }"));
    assert!(compile_fails("fn main() -> bool { 1 andalso true }"));
}
