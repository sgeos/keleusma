//! **GENERATED FLOAT EXPRESSION TREES, COMPARED AGAINST THE REFERENCE.**
//!
//! # Why this exists
//!
//! The three generators in this package — expressions, composites, nesting —
//! emit **zero float programs between them**. `generated_expressions.rs` states
//! that exclusion in its own header rather than hiding it, so this is a
//! capability extension and not a correction of a false claim.
//!
//! What makes it worth doing: **floats are the least-covered scalar by EXECUTION
//! breadth**, and the float yield arm landed on 2026-09-18 resting on four
//! hand-written subjects. Every float subject in this package was chosen by
//! someone who already had a hypothesis. Generated composition is the instrument
//! for the defect nobody hypothesised.
//!
//! # ⚠ THE CONSTRAINT THAT DOMINATES THE DESIGN
//!
//! `keleusma::vm::Vm` is `GenericVm<.., f64>`. **The reference runs at EIGHT
//! bytes in BOTH float configurations**, while this backend lowers at the
//! configured width — `f32` under `narrow-float-32`. A randomly generated float
//! tree produces values that are not four-byte exact, and the two implementations
//! then differ **legitimately**. Reported as a divergence, that is a false
//! finding about the emitter.
//!
//! **This was already paid for twice on the day this file was written.** In
//! `scalar_operator_matrix.rs` a result-only exactness check let a phantom
//! `float %: Disagree` through, because the violated thing was an OPERAND.
//!
//! So exactness is arranged by CONSTRUCTION and then checked anyway:
//!
//! * only `+`, `-` and `*`, so every intermediate stays an integer;
//! * integral leaves bounded by [`LEAF_MAX`] and arguments of the same size;
//! * a depth bound making the all-multiply worst case far smaller than `2^24`,
//!   the largest integer an `f32` represents exactly;
//! * and [`assert_exact`] on the arguments and on every observed result, because
//!   a construction argument rots the moment someone edits a bound.
//!
//! # ⚠ THE WORD GENERATOR'S BOUNDS ARE WRONG HERE, AND OBVIOUSLY REUSABLE
//!
//! It uses leaves `0..=9` at depth 3, whose all-multiply worst case is `9^8`,
//! about **43 million**. That is five orders of magnitude inside `i64` — its own
//! stated argument — and **two and a half times OUTSIDE** the `2^24 = 16777216`
//! exact-integer range of a four-byte float. Copying those bounds is the obvious
//! move and it silently breaks the invariant this file rests on.
//!
//! Leaves are bounded by 3 instead, at the same depth 3, giving `3^8 = 6561`.
//! **Depth is preserved rather than traded away**, because a shallower tree would
//! make this a slower spelling of the operator matrix.
//!
//! # What this excludes, stated where a reader meets it
//!
//! **No `/` and no `%`.** Division leaves the integers immediately — `3.0 / 7.0`
//! is inexact at both widths and differently inexact between them — so a
//! generated division could not be compared exactly. Those cells are covered
//! against a chosen pair of operands in `scalar_operator_matrix.rs`.
//!
//! No comparisons, no composites, no streams, no trap paths.
//!
//! # What a green run may claim
//!
//! No divergence over the generated trees, at this commit, within the bounds
//! above. **Not** that float lowering is correct.

mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use std::collections::BTreeSet;

/// The float width this backend lowers to.
#[cfg(feature = "narrow-float-32")]
type Flt = f32;
#[cfg(not(feature = "narrow-float-32"))]
type Flt = f64;

/// Widen this configuration's float to `f64`.
///
/// **Split by configuration rather than written once**, because `f64::from` is
/// required at four bytes and is a `useless_conversion` lint error at eight.
#[cfg(feature = "narrow-float-32")]
fn wide(x: Flt) -> f64 {
    f64::from(x)
}
#[cfg(not(feature = "narrow-float-32"))]
fn wide(x: Flt) -> f64 {
    x
}

/// Narrow a host `f64` to the width this backend lowers to.
#[cfg(feature = "narrow-float-32")]
fn narrow(x: f64) -> Flt {
    x as f32
}
#[cfg(not(feature = "narrow-float-32"))]
fn narrow(x: f64) -> Flt {
    x
}

/// The largest integer a four-byte float represents exactly.
const F32_EXACT_INT_LIMIT: f64 = 16_777_216.0;

/// Leaves and arguments are integers in `0..=LEAF_MAX`.
const LEAF_MAX: u64 = 3;
/// Tree depth. `LEAF_MAX ^ (2 ^ DEPTH)` must stay under [`F32_EXACT_INT_LIMIT`].
const DEPTH: u32 = 3;

/// The all-multiply worst case this file's exactness argument rests on.
fn worst_case_magnitude() -> f64 {
    (LEAF_MAX as f64).powi(1 << DEPTH)
}

/// Fail loudly on a value the differential must not be comparing.
fn assert_exact(what: &str, src: &str, v: f64) {
    assert!(
        f64::from(v as f32) == v,
        "{what} {v} for `{src}` is not exact in four bytes. The reference runs at \
         f64 in both configurations and this backend lowers at the configured \
         width, so comparing this value would report rounding as divergence. Fix \
         the generator's bounds; do not add a tolerance."
    );
    assert!(
        v.abs() <= F32_EXACT_INT_LIMIT,
        "{what} {v} for `{src}` exceeds the four-byte exact-integer limit of \
         {F32_EXACT_INT_LIMIT}. The magnitude bound this file rests on has been \
         broken by a change to LEAF_MAX or DEPTH."
    );
}

/// Xorshift64*, so a failure is reproducible rather than an anecdote.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// **Only the operators that keep an integral value integral.** See the header.
const FLOAT_OPS: &[&str] = &["+", "-", "*"];

fn leaf(rng: &mut Rng) -> String {
    match rng.below(4) {
        0 => "a".into(),
        1 => "b".into(),
        // A float literal, integral so the exactness invariant holds.
        _ => format!("{}.0", rng.below(LEAF_MAX + 1)),
    }
}

fn expr(rng: &mut Rng, depth: u32) -> String {
    if depth == 0 {
        return leaf(rng);
    }
    let op = FLOAT_OPS[rng.below(FLOAT_OPS.len() as u64) as usize];
    format!("({} {op} {})", expr(rng, depth - 1), expr(rng, depth - 1))
}

fn program(rng: &mut Rng, depth: u32) -> String {
    format!(
        "fn main(a: Float, b: Float) -> Float {{ {} }}",
        expr(rng, depth)
    )
}

/// Drive both implementations on one generated program.
///
/// **The native signature is derived from the lowered function, not guessed.** A
/// float-in, float-out chunk lowers with floating-point parameters and return;
/// calling it through an `i64` shape is undefined behaviour that surfaces as a
/// SIGBUS inside JIT code, which this package has already paid for once.
fn drive(src: &str, a: f64, b: f64) -> (f64, f64) {
    let m = common::build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    let vm_out = match vm.call(&[Value::Float(a), Value::Float(b)]) {
        Ok(VmState::Finished(Value::Float(v))) => v,
        other => panic!("the reference did not finish with a float on `{src}`: {other:?}"),
    };

    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("k");
    keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default())
        .expect("lower");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let sym = format!("kel_chunk_{entry}");
    let f = lm.get_function(&sym).expect("entry function");
    assert_eq!(
        f.count_params(),
        2,
        "`{src}` lowers with an unexpected arity"
    );
    for i in 0..2 {
        assert!(
            f.get_nth_param(i).expect("a parameter").is_float_value(),
            "`{src}` parameter {i} is not a float value, so the floating-point \
             signature below would be wrong and calling through it is undefined \
             behaviour"
        );
    }
    assert!(
        f.get_type()
            .get_return_type()
            .is_some_and(|t| t.is_float_type()),
        "`{src}` does not return a float, so the signature below is wrong"
    );
    let g = unsafe { ee.get_function::<unsafe extern "C" fn(Flt, Flt) -> Flt>(&sym) }
        .expect("entry symbol");
    let native = wide(unsafe { g.call(narrow(a), narrow(b)) });
    (vm_out, native)
}

const PROGRAMS: usize = 200;
const SEEDS: &[u64] = &[
    0xF10A_7000_0000_0001,
    0xF10A_7000_0000_0002,
    0xF10A_7000_0000_0003,
];
/// A generator that collapsed toward a few shapes would pass on count alone.
const DISTINCT_FLOOR: usize = 150;

/// **THE COMPARISON.**
#[test]
fn generated_float_trees_agree_with_the_reference() {
    let a = 3.0_f64;
    let b = 2.0_f64;
    assert_exact("argument a", "<arguments>", a);
    assert_exact("argument b", "<arguments>", b);

    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut ops: BTreeSet<&str> = BTreeSet::new();
    let mut compared = 0usize;

    for seed in SEEDS {
        let mut rng = Rng(*seed);
        for i in 0..PROGRAMS {
            let src = program(&mut rng, DEPTH);
            seen.insert(src.clone());
            for op in FLOAT_OPS {
                if src.contains(&format!(" {op} ")) {
                    ops.insert(op);
                }
            }

            let (vm, native) = drive(&src, a, b);
            assert_exact("the reference result", &src, vm);
            assert_exact("the backend result", &src, native);
            compared += 1;
            assert_eq!(
                vm, native,
                "DIVERGENCE on generated float program {i}:\n  {src}\n  \
                 reference = {vm}\n  native    = {native}\n\nBoth values are \
                 four-byte exact, so this is NOT the f64-versus-f32 harness \
                 artefact. Reproduce by re-running: the generator is seeded and \
                 deterministic. This establishes that the two implementations \
                 disagree, NOT which of them is right."
            );
        }
    }

    assert_eq!(
        compared,
        PROGRAMS * SEEDS.len(),
        "only {compared} of {} programs were compared; a run that generated \
         almost nothing must not pass as coverage",
        PROGRAMS * SEEDS.len()
    );
    assert!(
        seen.len() >= DISTINCT_FLOOR * SEEDS.len(),
        "the generator produced only {} distinct programs, so this is a slower \
         spelling of the fixed operator matrix rather than a test of composition",
        seen.len()
    );
    assert_eq!(
        ops.len(),
        FLOAT_OPS.len(),
        "only {} of {} operators appeared: {ops:?}",
        ops.len(),
        FLOAT_OPS.len()
    );
}

/// **THE PROGRAMS REALLY CONTAIN FLOAT ARITHMETIC**, and really nest.
///
/// A generator emitting `fn main(a: Float, b: Float) -> Float {{ a }}` three
/// hundred times would satisfy every count above while exercising no operator at
/// all, and a flat tree would make the composition claim false.
#[test]
fn the_generated_float_trees_are_float_and_nested() {
    let mut rng = Rng(0xF10A_7000_0000_00FF);
    let mut nested = 0usize;
    let mut with_literal = 0usize;
    for _ in 0..PROGRAMS {
        let src = program(&mut rng, DEPTH);
        assert!(
            src.contains("a: Float") && src.contains("-> Float"),
            "a generated program is not float-typed: {src}"
        );
        // A nested tree has a parenthesis inside a parenthesis.
        if src.contains("((") || src.contains(") (") || src.matches('(').count() > 2 {
            nested += 1;
        }
        if src.contains(".0") {
            with_literal += 1;
        }
    }
    assert!(
        nested * 10 >= PROGRAMS * 9,
        "only {nested} of {PROGRAMS} generated trees nest; the composition claim \
         is false"
    );
    assert!(
        with_literal > 0,
        "no generated program contains a float literal, so the leaves are only \
         parameters and the literal path is untested"
    );
}

/// **THE EXACTNESS INVARIANT IS ARITHMETIC, AND THE ARITHMETIC IS CHECKED.**
///
/// The header argues that `LEAF_MAX ^ (2 ^ DEPTH)` stays under the four-byte
/// exact-integer limit. **An argument in a comment is not a guard**: someone
/// raising `DEPTH` to 4 would break the invariant and every test above would keep
/// passing until a value happened to land outside the range. This fails instead.
#[test]
fn the_magnitude_bound_keeps_every_value_four_byte_exact() {
    assert!(
        worst_case_magnitude() <= F32_EXACT_INT_LIMIT,
        "the all-multiply worst case is {}, above the four-byte exact-integer \
         limit of {F32_EXACT_INT_LIMIT}. LEAF_MAX or DEPTH has been changed \
         without redoing the arithmetic, and the differential would report \
         rounding as divergence.",
        worst_case_magnitude()
    );

    // **NON-VACUITY.** The word generator's own bounds must FAIL this check, or
    // the bound is not discriminating anything.
    let word_generator_worst_case = 9.0_f64.powi(1 << 3);
    assert!(
        word_generator_worst_case > F32_EXACT_INT_LIMIT,
        "the word generator's bounds (leaves 0..=9 at depth 3) no longer exceed \
         the four-byte limit, so this file's reason for choosing different ones \
         has evaporated and the header is wrong"
    );
}
