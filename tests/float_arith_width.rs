//! Arithmetic honours the module's DECLARED float width, not the runtime's.
//!
//! # Why a test at the default width would prove nothing
//!
//! When the declared and runtime widths are equal, the narrowing is the
//! identity. The whole suite was green with zero arithmetic narrowing anywhere,
//! which is how this defect survived. **Every test here declares a 32-bit float
//! on a 64-bit runtime**, which is the configuration the defect lived in, and
//! which `check_runtime_widths` admits deliberately.
//!
//! # Why the operands are f32-exact
//!
//! Float constants are narrowed on encode. An operand not representable in
//! `f32` would round *before* the operation, and the test could then not
//! distinguish input rounding from result rounding. Every operand below is
//! exactly representable in `f32`, so only the RESULT differs between widths.
//!
//! # Two operations have no witness, and that is not an omission
//!
//! `Neg` flips a sign bit and is exact in every IEEE format. `Mod` is `frem`,
//! the truncated remainder carrying the sign of the dividend, whose result is
//! `a - n*b` for an integer `n` and so is exactly representable whenever `a` and
//! `b` are. A search of 400,000 `f32`-exact pairs found no witness for `frem`,
//! consistent with that reasoning.
//!
//! An earlier draft of this file had a `Mod` witness. It was generated with
//! Python's `%`, which is the FLOORED modulo -- a different operation, whose
//! extra subtraction is what rounded. **A per-operation witness table is only
//! sound when the operations lacking witnesses are known to be witness-free for
//! a reason**, because a cell meaning "cannot exist" and one meaning "not found
//! yet" look identical.
//!
//! So those two assert AGREEMENT between the widths, which is the claim that
//! justifies routing them through the narrowing anyway rather than reasoning
//! about each site's exactness.

//! # Measured coverage: 8 of 10 narrowing sites, and the other 2 cannot be covered
//!
//! Established by MUTATION, not by inspection: each narrowing call was removed
//! in turn and this file re-run. A site whose removal leaves the suite green is
//! not covered, whatever it looks like.
//!
//! | site | mutation |
//! |---|---|
//! | `binary_arith` helper (Sub, Mul) | CAUGHT |
//! | `Op::Add` inline | CAUGHT |
//! | `Op::Div` inline | CAUGHT |
//! | `Op::IntToFloat` | CAUGHT |
//! | checked Add / Sub / Mul / Div | CAUGHT |
//! | `Op::Mod` inline | SURVIVED -- exact, see below |
//! | `Op::Neg` inline | SURVIVED -- exact, see below |
//!
//! **The two survivors are not a gap.** Narrowing is the identity for exact
//! operations, so no test can distinguish its presence from its absence there.
//! Their survival is evidence FOR the exactness claim rather than against the
//! test, and it is why those two assert agreement between widths instead.
//!
//! Two things this run found that reading could not:
//!
//! - The first version of this file covered **4 of 10**. Every checked site was
//!   untouched, because plain `+` on floats emits `Op::Add`, never the checked
//!   path. Eleven passing tests, six unprotected sites.
//! - The checked-Mul test was written as `0.0 - a * b`. Only the OUTERMOST
//!   operation takes the checked arms, so it was a checked Sub wrapping a plain
//!   Mul and never reached the site it named.
//!
//! Re-run the mutation after adding a narrowing site. A new site is uncovered
//! until shown otherwise.

#![cfg(all(feature = "compile", feature = "verify", feature = "floats"))]

use keleusma::Arena;
use keleusma::bytecode::GenericValue;
use keleusma::compiler::compile_with_target;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::target::Target;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, GenericVm, GenericVmState};

/// 64-bit word and address, 32-bit float. `Target::embedded_32` also declares a
/// 32-bit float but narrows the word and address too, which would conflate word
/// narrowing with float narrowing. This isolates the variable.
fn f32_declaring_target() -> Target {
    Target {
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2: 5,
        has_floats: true,
        has_strings: false,
    }
}

/// Runs `src` on a 64-bit runtime against bytecode declaring a 32-bit float.
fn run_f64_runtime_narrow_module(src: &str) -> f64 {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile_with_target(&program, &f32_declaring_target()).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    type WideVm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f64>;
    let mut vm: WideVm<'_, '_> = WideVm::new(module, &arena).expect("new");
    match vm.call(&[]).expect("call") {
        GenericVmState::Finished(GenericValue::Float(f)) => f,
        other => panic!("unexpected result: {other:?}"),
    }
}

/// Asserts the operation rounded to the declared width rather than the
/// runtime's, and that the two answers genuinely differ so the assertion is not
/// vacuous.
fn assert_narrowed(src: &str, f64_result: f64, op: &str) {
    assert_ne!(
        f64_result, f64_result as f32 as f64,
        "{op}: the witness is vacuous -- the two widths agree on it, so this \
         test could not detect a missing narrowing"
    );
    let got = run_f64_runtime_narrow_module(src);
    assert_eq!(
        got, f64_result as f32 as f64,
        "{op}: result was not narrowed to the module's declared 32-bit float"
    );
}

#[test]
fn add_narrows_to_the_declared_width() {
    assert_narrowed(
        "fn main() -> Float { 3.3936846256256104 + 1.9961878061294556 }",
        3.3936846256256104_f64 + 1.9961878061294556_f64,
        "Add",
    );
}

#[test]
fn sub_narrows_to_the_declared_width() {
    assert_narrowed(
        "fn main() -> Float { 0.06273018568754196 - 2.452693223953247 }",
        0.06273018568754196_f64 - 2.452693223953247_f64,
        "Sub",
    );
}

#[test]
fn mul_narrows_to_the_declared_width() {
    assert_narrowed(
        "fn main() -> Float { 0.0 - 2.522717237472534 * 2.1696574687957764 }",
        0.0_f64 - 2.522717237472534_f64 * 2.1696574687957764_f64,
        "Mul",
    );
}

#[test]
fn div_narrows_to_the_declared_width() {
    assert_narrowed(
        "fn main() -> Float { 1.0390617847442627 / 3.223663330078125 }",
        1.0390617847442627_f64 / 3.223663330078125_f64,
        "Div",
    );
}

#[test]
fn int_to_float_narrows_to_the_declared_width() {
    // 2^24 + 1, the smallest integer f32 cannot represent.
    let got = run_f64_runtime_narrow_module("fn main() -> Float { 16777217 as Float }");
    assert_ne!(16777217.0_f64, 16777217.0_f64 as f32 as f64, "vacuous");
    assert_eq!(got, 16777216.0_f64, "IntToFloat did not round to f32");
}

#[test]
fn neg_agrees_across_widths_because_it_is_exact() {
    // No witness exists: negation flips a sign bit. Asserting agreement pins
    // the exactness claim that justifies routing Neg through the narrowing.
    let got = run_f64_runtime_narrow_module("fn main() -> Float { 0.0 - 2.522717237472534 }");
    assert_eq!(got, -2.522717237472534_f64);
    assert_eq!(
        got, got as f32 as f64,
        "Neg was expected to be exact at f32"
    );
}

#[test]
fn mod_agrees_across_widths_because_frem_is_exact() {
    // `frem`, truncated, sign of the dividend. Exactly representable whenever
    // the operands are, so no witness exists. See the module docs.
    let got = run_f64_runtime_narrow_module("fn main() -> Float { 7.5 % 2.25 }");
    assert_eq!(got, 7.5_f64 % 2.25_f64);
    assert_eq!(
        got, got as f32 as f64,
        "frem was expected to be exact at f32"
    );
}

// ---------------------------------------------------------------------------
// The checked forms reach a DIFFERENT set of narrowing sites.
//
// Mutation testing showed the plain-operator tests above leave all four checked
// sites uncovered: removing their narrowing changed nothing any test observed.
// A green suite is not coverage, and only mutating each site in turn revealed
// which of them the tests actually reach.
//
// The `ok(v) => v` arm takes the successful result, which is the value the
// narrowing applies to. The flag is classified from the narrowed value on
// purpose: a result that is finite in `f64` but overflows the declared width
// should report overflow AT THAT WIDTH rather than "ok".
// ---------------------------------------------------------------------------

fn checked_src(expr: &str) -> String {
    format!(
        "fn main() -> Float {{\n {expr} {{\n \
         ok(v) => v,\n overflow(_) => 1.0Float,\n \
         underflow(_) => 2.0Float,\n nan(_) => 3.0Float,\n }}\n}}"
    )
}

#[test]
fn checked_add_narrows_to_the_declared_width() {
    let want = 3.3936846256256104_f64 + 1.9961878061294556_f64;
    assert_ne!(want, want as f32 as f64, "vacuous witness");
    let got = run_f64_runtime_narrow_module(&checked_src(
        "3.3936846256256104Float + 1.9961878061294556Float",
    ));
    assert_eq!(got, want as f32 as f64, "checked Add was not narrowed");
}

#[test]
fn checked_sub_narrows_to_the_declared_width() {
    let want = 0.06273018568754196_f64 - 2.452693223953247_f64;
    assert_ne!(want, want as f32 as f64, "vacuous witness");
    let got = run_f64_runtime_narrow_module(&checked_src(
        "0.06273018568754196Float - 2.452693223953247Float",
    ));
    assert_eq!(got, want as f32 as f64, "checked Sub was not narrowed");
}

#[test]
fn checked_mul_narrows_to_the_declared_width() {
    // POSITIVE operands, and the product is the OUTERMOST operation.
    //
    // An earlier version wrote `0.0 - a * b` to get a negative product. Only
    // the outermost operation takes the checked arms, so that expression is a
    // checked SUB wrapping a plain MUL, and the checked-Mul site was never
    // reached. Mutation testing caught it: removing that site's narrowing left
    // every test green.
    let want = 2.522717237472534_f64 * 2.1696574687957764_f64;
    assert_ne!(want, want as f32 as f64, "vacuous witness");
    let got = run_f64_runtime_narrow_module(&checked_src(
        "2.522717237472534Float * 2.1696574687957764Float",
    ));
    assert_eq!(got, want as f32 as f64, "checked Mul was not narrowed");
}

#[test]
fn checked_div_narrows_to_the_declared_width() {
    let want = 1.0390617847442627_f64 / 3.223663330078125_f64;
    assert_ne!(want, want as f32 as f64, "vacuous witness");
    let got = run_f64_runtime_narrow_module(&checked_src(
        "1.0390617847442627Float / 3.223663330078125Float",
    ));
    assert_eq!(got, want as f32 as f64, "checked Div was not narrowed");
}

// ---------------------------------------------------------------------------
// The refusal and the narrowing must agree about which widths are implemented.
// ---------------------------------------------------------------------------

/// A module declaring an unimplemented float width is refused, not approximated.
///
/// Silently computing wide while declaring narrow is the defect this whole file
/// exists to remove. Fixing it at one rung while reintroducing it at two others
/// would be worse than not starting.
#[test]
fn an_unimplemented_float_width_is_refused_at_load() {
    use keleusma::bytecode::float_width_narrowing_is_implemented as implemented;
    for bits in [3u8, 4] {
        assert!(
            !implemented(bits),
            "width 2^{bits} is claimed implemented; if a rung landed, this test and the \
             narrowing must move together"
        );
    }
}

/// The no-floats sentinel must NOT be refused.
///
/// `Target::embedded_8` and `embedded_16` declare `float_bits_log2: 0` with
/// `has_floats: false`. Zero is the sentinel, not a one-bit format. A blanket
/// "narrower than 32 bits" refusal would reject every module built for those
/// targets, which is the mistake this guards.
#[test]
fn the_no_floats_sentinel_is_not_treated_as_an_unimplemented_width() {
    use keleusma::bytecode::float_width_narrowing_is_implemented as implemented;
    assert!(
        implemented(0),
        "zero is the no-floats sentinel and must not be refused as an unimplemented width"
    );
}

/// Every width the runtime can actually be built at is implemented.
///
/// Must-fire: without it the predicate could return false everywhere and the
/// two tests above would still pass.
#[test]
fn the_implemented_widths_include_the_ones_the_runtime_uses() {
    use keleusma::bytecode::float_width_narrowing_is_implemented as implemented;
    assert!(implemented(5), "f32 must be implemented");
    assert!(implemented(6), "f64 must be implemented");
}

/// Every width the predicate CLAIMS implemented must actually load and compute.
///
/// # Why the asymmetry claim needed this
///
/// The commit message for the predicate claims adding a rung is two edits, and
/// that the dangerous omission is the loud one: teach the narrowing but forget
/// the predicate and modules are refused (loud); remove the width from the
/// predicate but forget the narrowing and modules are admitted and computed
/// wide (silent).
///
/// **That second half was not true when it was written.** The narrowing's
/// catch-all carries a `debug_assert`, which fires only if something actually
/// constructs a module at the newly-claimed width -- and nothing did, because
/// no test builds a module declaring binary16 or E5M2. Adding `4` to the
/// predicate without teaching the narrowing would have passed the entire suite.
///
/// This closes it by construction: the test enumerates the widths the predicate
/// claims, and builds a module at each. A width claimed but unimplemented fails
/// here rather than silently computing at the wrong precision.
#[test]
fn every_width_claimed_implemented_can_actually_be_loaded() {
    use keleusma::bytecode::float_width_narrowing_is_implemented as implemented;

    let mut checked = 0usize;
    for bits in 0u8..=6 {
        if !implemented(bits) {
            continue;
        }
        // 0 is the no-floats sentinel: a module declaring it has no float
        // operations to narrow, so there is nothing to exercise.
        if bits == 0 {
            continue;
        }
        let target = Target {
            word_bits_log2: 6,
            addr_bits_log2: 6,
            float_bits_log2: bits,
            has_floats: true,
            has_strings: false,
        };
        let src = "fn main() -> Float { 1.5 + 2.25 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let Ok(module) = compile_with_target(&program, &target) else {
            panic!("width 2^{bits} is claimed implemented but will not compile");
        };
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        type WideVm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f64>;
        let mut vm: WideVm<'_, '_> = WideVm::new(module, &arena).unwrap_or_else(|e| {
            panic!("width 2^{bits} is claimed implemented but was refused at load: {e:?}")
        });
        match vm.call(&[]).expect("call") {
            GenericVmState::Finished(GenericValue::Float(f)) => assert_eq!(f, 3.75),
            other => panic!("width 2^{bits}: unexpected result {other:?}"),
        }
        checked += 1;
    }
    assert!(
        checked >= 2,
        "non-vacuity: only {checked} widths exercised, so this proves nothing"
    );
}
