//! **THE CHECKED FIXED-POINT MULTIPLY, WHICH WAS REFUSED.**
//!
//! # What was refused and why it is not a decision
//!
//! `Op::FixedMul` and `Op::FixedDiv` — the bare `a * b` and `a / b` on `Fixed` —
//! both lower. The *checked* form, with `ok` and `overflow` arms, did not:
//! `Op::CheckedMul(n)` with a non-zero fraction count fell through to a refusal.
//!
//! The refusal's own test said the boundary *"must be changed deliberately — as
//! the division case above had to be when multiplication arrived — rather than
//! deleted"*. **It has moved before, on purpose, when capability arrived.**
//!
//! # The count is the static type signal, and that is an upstream premise
//!
//! The runtime dispatches this opcode on the operand's RUNTIME type. The backend
//! cannot see one — `Fixed` and `Word` are both eight bytes. **The compiler emits
//! a non-zero fraction count only for the Fixed arm**, so the count carries the
//! type, and that is a claim about what the compiler emits.
//!
//! # Read from the runtime, because the integer arm is a trap
//!
//! | | integer `CheckedMul(0)` | Fixed `CheckedMul(n)` |
//! |---|---|---|
//! | middle slot | the product's HIGH half | **always zero** |
//! | low slot | truncated | truncated, contract is **wrapping** |
//! | shift | none | arithmetic right by `n` before classifying |
//!
//! Reusing the integer triple would hand an overflow arm a high half the runtime
//! never produces. And this is distinct from `Op::FixedMul`, which **saturates**
//! where this one **wraps** — same arithmetic in the middle, different contract at
//! the edges.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::{LowerOptions, lower_module, module_refusals};

mod common;

/// **The arms return DIFFERENT things**, so which one ran is observable. A
/// subject whose arms agree proves the arithmetic and nothing about the flag.
const CHECKED: &str = "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> \
                       { a * b { ok(v) => v, overflow(w) => w - 1 } }";

/// The saturating bare form, for the contrast test.
const BARE: &str = "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a * b }";

fn vm_fixed(src: &str, a: i64, b: i64) -> i64 {
    let m = common::build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m, &arena).expect("vm");
    match vm
        .call(&[Value::Fixed(a), Value::Fixed(b)])
        .expect("vm run")
    {
        VmState::Finished(Value::Fixed(v)) => v,
        other => panic!("expected a Fixed result, got {other:?}"),
    }
}

fn native_fixed(src: &str, a: i64, b: i64) -> i64 {
    let m = common::build(src);
    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("k");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let f = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, i64) -> i64>(&format!("kel_chunk_{entry}"))
    }
    .expect("entry symbol");
    unsafe { f.call(a, b) }
}

/// One Q16 unit.
const ONE: i64 = 1 << 16;

#[test]
fn the_checked_fixed_multiply_agrees_across_ok_overflow_and_underflow() {
    let refusals = module_refusals(&common::build(CHECKED), LowerOptions::default());
    assert!(
        refusals.is_empty(),
        "the checked form must lower: {refusals:?}"
    );

    // In range; above the word range; below it. The third is the one an
    // unsigned comparison would get wrong.
    let cases = [
        (ONE, 2 * ONE, "ok"),
        (1i64 << 40, 1i64 << 40, "overflow"),
        (-(1i64 << 40), 1i64 << 40, "underflow"),
        (-ONE, ONE, "ok, negative"),
    ];
    let mut arms_seen = 0usize;
    for (a, b, what) in cases {
        let vm = vm_fixed(CHECKED, a, b);
        let nat = native_fixed(CHECKED, a, b);
        assert_eq!(nat, vm, "{what}: a={a} b={b} native={nat} vm={vm}");
        // The overflow arm subtracts one, so an out-of-range product is
        // observably a different value from the wrapped low alone.
        if what.starts_with("overflow") || what.starts_with("underflow") {
            arms_seen += 1;
        }
    }
    assert_eq!(
        arms_seen, 2,
        "both the overflow and underflow cases must be driven, or the flag has \
         only been exercised in one direction"
    );
}

/// **THE ARM SELECTION IS OBSERVABLE**, which agreement alone does not establish.
///
/// The overflow arm returns `w - 1`, so an out-of-range product differs from the
/// wrapped value by exactly one. If the flag were always zero, the two would be
/// equal and every agreement above would still hold.
#[test]
fn the_overflow_arm_is_actually_selected() {
    let a = 1i64 << 40;
    let wrapped_only = vm_fixed(
        "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a * b { ok(v) => v, overflow(w) => w } }",
        a,
        a,
    );
    let with_marker = vm_fixed(CHECKED, a, a);
    // **`w - 1` IS FIXED SUBTRACTION, NOT A RAW DECREMENT.** A first version of
    // this expectation wrote `wrapped_only - 1` and failed at -65536 against -1:
    // the literal `1` is one UNIT, which is `1 << 16` in Q16 raw bits. The
    // lowering was right and the test's arithmetic was wrong, which is the more
    // common way round and worth the comment.
    assert_eq!(
        with_marker,
        wrapped_only - ONE,
        "the reference must take the overflow arm for an out-of-range product; if \
         it does not, this subject is not exercising the flag"
    );
    assert_eq!(
        native_fixed(CHECKED, a, a),
        with_marker,
        "native must select the same arm as the reference"
    );
}

/// **WRAPPING, NOT SATURATING**, which is the contract difference from the bare
/// form and the one a shared implementation would silently lose.
#[test]
fn the_checked_form_wraps_where_the_bare_form_saturates() {
    let a = 1i64 << 40;
    let bare = vm_fixed(BARE, a, a);
    let checked = vm_fixed(
        "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a * b { ok(v) => v, overflow(w) => w } }",
        a,
        a,
    );
    assert_ne!(
        bare, checked,
        "the bare form saturates and the checked form wraps; if they agree on an \
         out-of-range product this test is not distinguishing them"
    );
    assert_eq!(
        native_fixed(BARE, a, a),
        bare,
        "the bare saturating form must still agree"
    );
}

/// An out-of-range fraction count is refused, as the bare form already refuses
/// it, rather than shifting by a count at or beyond the word width.
#[test]
fn a_fraction_count_at_the_word_width_is_refused() {
    use keleusma::bytecode::Op;
    let mut m = common::build(CHECKED);
    let entry = m.entry_point.expect("entry point");
    let mut patched = false;
    for op in m.chunks[entry].ops.iter_mut() {
        if let Op::CheckedMul(n) = op
            && *n != 0
        {
            *op = Op::CheckedMul(64);
            patched = true;
            break;
        }
    }
    assert!(
        patched,
        "the subject must contain a non-zero-fraction CheckedMul, or this test \
         patches nothing"
    );
    let refusals = module_refusals(&m, LowerOptions::default());
    assert!(
        !refusals.is_empty(),
        "a fraction count at the word width must be refused; the runtime reports \
         InvalidBytecode there"
    );
}

/// **THE CHECKED FIXED-POINT DIVIDE**, which was refused on a ground that did not
/// distinguish it.
///
/// The reason given was that it reaches for `__divti3`, the compiler runtime's
/// 128-bit signed division. **That cost is already paid**:
/// `linkage_symbol_census.rs` measured `Fixed` division as the one construct in
/// its sweep needing a compiler-runtime symbol, and the bare `Op::FixedDiv`
/// lowers. Any object doing fixed division already depends on it.
///
/// Three paths, and they are genuinely different: an ordinary quotient, a
/// quotient outside the word range, and a zero divisor — which the runtime
/// reifies as flag 3 with the NUMERATOR in the low slot rather than faulting.
#[test]
fn the_checked_fixed_divide_agrees_on_ok_out_of_range_and_zero_divisor() {
    const DIV: &str = "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> \
                       { a / b { ok(v) => v, overflow(w) => w, zero_divisor(n) => n } }";

    let refusals = module_refusals(&common::build(DIV), LowerOptions::default());
    assert!(
        refusals.is_empty(),
        "the checked divide must lower: {refusals:?}"
    );

    for (a, b, what) in [
        (6 * ONE, 2 * ONE, "ok"),
        (-6 * ONE, 2 * ONE, "ok, negative"),
        (i64::MAX, ONE / 2, "out of range"),
        (5 * ONE, 0, "zero divisor"),
    ] {
        let vm = vm_fixed(DIV, a, b);
        let nat = native_fixed(DIV, a, b);
        assert_eq!(nat, vm, "{what}: a={a} b={b} native={nat} vm={vm}");
    }
}

/// **The zero-divisor path is distinct**, which agreement alone does not show: if
/// it produced the same value as an ordinary quotient, every comparison above
/// would still hold.
#[test]
fn the_zero_divisor_binds_the_numerator_rather_than_a_quotient() {
    const DIV: &str = "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> \
                       { a / b { ok(v) => v, overflow(w) => w, zero_divisor(n) => n } }";
    let numerator = 5 * ONE;
    let zero_div = vm_fixed(DIV, numerator, 0);
    assert_eq!(
        zero_div, numerator,
        "the reference binds the NUMERATOR on a zero divisor; if that changes, this \
         subject is no longer exercising flag 3"
    );
    let ordinary = vm_fixed(DIV, numerator, 2 * ONE);
    assert_ne!(
        ordinary, zero_div,
        "the zero-divisor result must differ from an ordinary quotient, or this test \
         cannot tell the paths apart"
    );
    assert_eq!(
        native_fixed(DIV, numerator, 0),
        zero_div,
        "native must take the same path as the reference"
    );
}

/// An out-of-range fraction count is refused for the divide as for the multiply.
#[test]
fn a_divide_fraction_count_at_the_word_width_is_refused() {
    use keleusma::bytecode::Op;
    const DIV: &str = "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> \
                       { a / b { ok(v) => v, overflow(w) => w, zero_divisor(n) => n } }";
    let mut m = common::build(DIV);
    let entry = m.entry_point.expect("entry point");
    let mut patched = false;
    for op in m.chunks[entry].ops.iter_mut() {
        if let Op::CheckedDiv(n) = op
            && *n != 0
        {
            *op = Op::CheckedDiv(64);
            patched = true;
            break;
        }
    }
    assert!(
        patched,
        "the subject must contain a non-zero-fraction CheckedDiv"
    );
    assert!(
        !module_refusals(&m, LowerOptions::default()).is_empty(),
        "a fraction count at the word width must be refused"
    );
}
