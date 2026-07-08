#![cfg(all(feature = "compile", feature = "verify"))]
//! General const generics (B40), phase 1: function const parameters used
//! as `Word` values, supplied by an explicit turbofish `f::<8>(...)`, and
//! erased to concrete literals at monomorphization so the verifier sees
//! no symbolic const.

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

fn compile_fails(src: &str) -> bool {
    match parse(&tokenize(src).expect("lex")) {
        Ok(program) => compile(&program).is_err(),
        Err(_) => true,
    }
}

#[test]
fn const_param_used_as_value() {
    // A const parameter is a `Word` value in the body; the turbofish
    // supplies it explicitly and monomorphization substitutes the literal.
    assert_eq!(
        run_to_int("fn val<const n: Word>() -> Word { n }\nfn main() -> Word { val::<5>() }"),
        5
    );
    // Const arithmetic in the body via ordinary `+`.
    assert_eq!(
        run_to_int(
            "fn plus<const n: Word>() -> Word { n + 10 }\nfn main() -> Word { plus::<7>() }"
        ),
        17
    );
}

#[test]
fn distinct_const_args_specialize_independently() {
    // Two instantiations with different const values are distinct
    // specializations, so both concrete values are observed.
    assert_eq!(
        run_to_int(
            "fn val<const n: Word>() -> Word { n }\n\
             fn main() -> Word { val::<3>() + val::<8>() }"
        ),
        11
    );
}

#[test]
fn local_binding_shadows_const_param() {
    // A `let` that reuses the const parameter's name shadows it: the tail
    // `n` is the local `3`, not the const parameter `5`. This is the
    // load-bearing shadowing property of value substitution.
    assert_eq!(
        run_to_int(
            "fn shadow<const n: Word>() -> Word { let n = 3; n }\n\
             fn main() -> Word { shadow::<5>() }"
        ),
        3
    );
    // The const parameter is visible again after the shadowing binding's
    // introduction only within its scope; here a fresh `let` rebinds and
    // the const value is captured before the shadow.
    assert_eq!(
        run_to_int(
            "fn shadow<const n: Word>() -> Word { let a = n; let n = 100; a }\n\
             fn main() -> Word { shadow::<5>() }"
        ),
        5
    );
}

#[test]
fn const_param_bounds_a_verified_loop() {
    // A `for` loop bounded by a const parameter must verify: after
    // substitution the bound is the concrete literal, so the verifier
    // extracts a static iteration count. `Vm::new` runs the verifier, so
    // its success proves the bound became concrete (a symbolic bound
    // would be rejected as not statically boundable).
    let src = "fn loops<const n: Word>() -> Word { for i in 0..n { i } n }\n\
               fn main() -> Word { loops::<5>() }";
    assert_eq!(run_to_int(src), 5);
}

#[test]
fn turbofish_arity_is_checked() {
    // Too many const arguments.
    assert!(compile_fails(
        "fn val<const n: Word>() -> Word { n }\nfn main() -> Word { val::<5, 6>() }"
    ));
    // A const turbofish on a non-const-generic function.
    assert!(compile_fails(
        "fn plain() -> Word { 0 }\nfn main() -> Word { plain::<5>() }"
    ));
    // A const-generic function called without the required turbofish
    // cannot supply the const argument, so the arity check rejects it.
    assert!(compile_fails(
        "fn val<const n: Word>() -> Word { n }\nfn main() -> Word { val() }"
    ));
}

#[test]
fn const_param_type_must_be_word() {
    assert!(compile_fails(
        "fn val<const n: Byte>() -> Word { 0 }\nfn main() -> Word { val::<5>() }"
    ));
}

#[test]
fn const_dim_in_array_param() {
    // A parameter typed `[Word; n]` specializes to a concrete array size;
    // the argument's size unifies with the const dimension (B40 phase 2).
    assert_eq!(
        run_to_int(
            "fn first<const n: Word>(a: [Word; n]) -> Word { a[0] }\n\
             fn main() -> Word { first::<3>([10, 20, 30]) }"
        ),
        10
    );
}

#[test]
fn const_dim_in_multiword_param() {
    // A parameter typed `Multiword<n>` specializes to a concrete word
    // count; digit indexing works after substitution.
    assert_eq!(
        run_to_int(
            "fn low<const n: Word>(m: Multiword<n>) -> Word { m[0] }\n\
             fn main() -> Word { low::<2>((7, 0) as Multiword<2>) }"
        ),
        7
    );
}

#[test]
fn const_dim_mismatch_fails_at_retypecheck() {
    // A body generically valid but instantiated to a mismatching const
    // dimension is rejected: the argument `[Word; 2]` does not match the
    // specialized parameter `[Word; 3]`. The post-monomorphization
    // re-typecheck is the soundness gate (a symbolic dimension is
    // accepted in the first pass, deferred to here).
    assert!(compile_fails(
        "fn first<const n: Word>(a: [Word; n]) -> Word { a[0] }\n\
         fn main() -> Word { first::<3>([10, 20]) }"
    ));
}

#[test]
fn e2_multiword_cast_fraction_bits_range_checked() {
    // Audit E2: a cast target Multiword<N, f> whose fraction-bit parameter
    // monomorphizes out of [0, 65535] is rejected in the cast typecheck. The
    // layout-pass backstop does not rescue the cast, because the compiler's
    // layout_for consumers swallow its error to a default, so the check must be
    // at typecheck. The word-count position is not reachable via a cast, since
    // the cast arity must match a concrete tuple.
    assert!(compile_fails(
        "fn c<const f: Word>() -> Word { let m = (1, 2) as Multiword<2, f>; m[0] }\n\
         fn main() -> Word { c::<65537>() }"
    ));
    // A valid fraction-bit count compiles and runs; m[0] is the low digit.
    assert_eq!(
        run_to_int(
            "fn c<const f: Word>() -> Word { let m = (1, 2) as Multiword<2, f>; m[0] }\n\
             fn main() -> Word { c::<8>() }"
        ),
        1
    );
}

#[test]
fn f2_multiword_cast_overflowed_symbolic_fraction_bits_rejected() {
    // Audit F2: a fraction-bit const expression that overflows i64 during
    // monomorphization folding stays symbolic past the re-typecheck (checked
    // folding leaves it unresolved rather than wrapping). The cast lowering and
    // the layout fraction-bit backstop reject it rather than silently skip the
    // Multiword construction and emit the untyped tuple.
    assert!(compile_fails(
        "fn c<const f: Word>() -> Word { let m = (1, 2) as Multiword<2, f * f>; m[0] }\n\
         fn main() -> Word { c::<3037000500>() }"
    ));
    // A fraction-bit expression that stays in range still compiles and runs.
    assert_eq!(
        run_to_int(
            "fn c<const f: Word>() -> Word { let m = (1, 2) as Multiword<2, f * f>; m[0] }\n\
             fn main() -> Word { c::<3>() }"
        ),
        1
    );
}

#[test]
fn const_generic_struct_basic() {
    // A struct parameterized by a const value; the field type [Word; n]
    // specializes to a concrete array (B40 phase 3).
    assert_eq!(
        run_to_int(
            "struct Buf<const n: Word> { cap: Word, items: [Word; n] }\n\
             fn get(b: Buf<8>) -> Word { b.cap }\n\
             fn main() -> Word { get(Buf::<8> { cap: 42, items: [0, 0, 0, 0, 0, 0, 0, 0] }) }"
        ),
        42
    );
}

#[test]
fn mixed_type_and_const_parameters() {
    // A struct may carry both a type parameter and a const parameter.
    // The type argument is inferred from the field value and the const
    // argument is supplied through the const-only construction turbofish
    // `Pair::<3>`; both specialize together. `p.first` is the `T = Word`
    // field and `p.items[0]` the const-dimensioned array field.
    assert_eq!(
        run_to_int(
            "struct Pair<T, const n: Word> { first: T, items: [Word; n] }\n\
             fn get(p: Pair<Word, 3>) -> Word { p.first + p.items[0] }\n\
             fn main() -> Word { get(Pair::<3> { first: 5, items: [10, 20, 30] }) }"
        ),
        15
    );
}

#[test]
fn const_generic_struct_array_field_index() {
    // Indexing a const-generic struct's array-typed field, `b.items[i]`,
    // resolves after the field-index misrouting fix: the field type
    // `[Word; n]` specializes to `[Word; 3]` and the access lowers to the
    // general array-index path rather than a data-segment access.
    assert_eq!(
        run_to_int(
            "struct Buf<const n: Word> { items: [Word; n] }\n\
             fn get(b: Buf<3>) -> Word { b.items[2] }\n\
             fn main() -> Word { get(Buf::<3> { items: [10, 20, 30] }) }"
        ),
        30
    );
}

#[test]
fn const_generic_enum_basic() {
    // An enum parameterized by a const value; the variant payload type
    // [Word; n] specializes to a concrete array so the specialized enum
    // `Buf__c3` lays out (a symbolic dim reaching the layout pass is an
    // internal-compiler-error tripwire, so a successful compile proves the
    // const parameter was erased) (B40 phase 3, enums).
    assert_eq!(
        run_to_int(
            "enum Buf<const n: Word> { Full([Word; n]), Tag(Word) }\n\
             fn get(b: Buf<3>) -> Word { match b { Buf::Full(_) => 0, Buf::Tag(x) => x } }\n\
             fn main() -> Word { get(Buf::<3>::Tag(42)) }"
        ),
        42
    );
}

#[test]
fn const_arith_in_array_dim() {
    // A const dimension may be an arithmetic expression over a const
    // parameter. `[Word; n + 1]` with `n = 2` specializes to `[Word; 3]`,
    // so the three-element argument matches (B40 phase 4).
    assert_eq!(
        run_to_int(
            "fn first<const n: Word>(a: [Word; n + 1]) -> Word { a[0] }\n\
             fn main() -> Word { first::<2>([10, 20, 30]) }"
        ),
        10
    );
}

#[test]
fn const_arith_precedence() {
    // `*` binds tighter than `+`: `n * 2 + 1` with `n = 2` is `5`, used
    // here as a value (the body arithmetic path), confirming the const
    // parameter participates in ordinary precedence.
    assert_eq!(
        run_to_int(
            "fn v<const n: Word>() -> Word { n * 2 + 1 }\n\
             fn main() -> Word { v::<2>() }"
        ),
        5
    );
}

#[test]
fn commutative_const_dims_unify() {
    // A body whose parameter dimension is `n + 1` and whose return
    // dimension is `1 + n` must type check in the first pass: the two
    // symbolic dimensions are commutatively equal, so canonical
    // normalization makes them compatible. Without normalization the
    // first pass would reject this valid program (B40 phase 4). The
    // instantiation `n = 2` makes both `3`.
    assert_eq!(
        run_to_int(
            "fn ident<const n: Word>(a: [Word; n + 1]) -> [Word; 1 + n] { a }\n\
             fn main() -> Word { ident::<2>([7, 8, 9])[0] }"
        ),
        7
    );
}

#[test]
fn const_arith_multiword_word_count() {
    // A Multiword word count may be an arithmetic expression: `2 * n`
    // with `n = 1` specializes to a two-word value (B40 phase 4).
    assert_eq!(
        run_to_int(
            "fn low<const n: Word>(m: Multiword<2 * n>) -> Word { m[0] }\n\
             fn main() -> Word { low::<1>((7, 0) as Multiword<2>) }"
        ),
        7
    );
}

#[test]
fn distinct_enum_const_args_specialize_independently() {
    // Two instantiations of a const-generic enum with different const
    // values are distinct specializations (`Buf__c2` and `Buf__c4`), each
    // with its own concrete payload layout.
    assert_eq!(
        run_to_int(
            "enum Buf<const n: Word> { Full([Word; n]), Tag(Word) }\n\
             fn get2(b: Buf<2>) -> Word { match b { Buf::Full(_) => 0, Buf::Tag(x) => x } }\n\
             fn get4(b: Buf<4>) -> Word { match b { Buf::Full(_) => 0, Buf::Tag(x) => x } }\n\
             fn main() -> Word { get2(Buf::<2>::Tag(5)) + get4(Buf::<4>::Tag(8)) }"
        ),
        13
    );
}

#[test]
fn const_arithmetic_overflow_is_rejected_not_wrapped() {
    // audit C6: `3037000500 * 3037000500` overflows i64. Checked const
    // arithmetic yields no static value, so the const argument fails to
    // resolve rather than folding a silently wrapped, wrong dimension.
    assert!(compile_fails(
        "fn val<const n: Word>() -> Word { n }\n\
         fn main() -> Word { val::<3037000500 * 3037000500>() }"
    ));
    // A non-overflowing const expression still compiles and runs.
    assert_eq!(
        run_to_int(
            "fn val<const n: Word>() -> Word { n }\n\
                    fn main() -> Word { val::<3000 * 3>() }"
        ),
        9000
    );
}
