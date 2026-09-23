//! **GENERATED FIXED EXPRESSION TREES, COMPARED AGAINST THE REFERENCE.**
//!
//! # Why this surface
//!
//! `Fixed` was the last scalar with no generated coverage, and it is the one with
//! the most machinery behind it: a Q-format scale, dedicated `FixedMul` and
//! `FixedDiv` opcodes, and a `%` the reference virtual machine TRAPS on.
//!
//! **`Fixed % Fixed` is also the only real divergence this line has ever found** —
//! the reference compiler accepted it, the virtual machine trapped, and this
//! backend returned an arithmetically correct answer. That history is the argument
//! for generated breadth here rather than against it.
//!
//! # ⚠ THE HAZARD PROFILE INVERTS FROM THE FLOAT GENERATORS
//!
//! **The f32-versus-f64 constraint does not apply at all.** `Fixed` is exact
//! integer arithmetic on `i64` bits in both implementations; there is no configured
//! width to disagree about. Copying `generated_floats.rs`'s exactness machinery
//! would be guarding against a hazard this surface does not have, and noise makes
//! the real guard harder to see.
//!
//! **What replaces it is overflow, and it is worse.** `Op::Add`, `Op::Sub` and
//! `Op::Mul` are the UNCHECKED opcodes — the compiler reserves them for `Byte`,
//! `Fixed` and `Float` — so a `Fixed` overflow is a **wrong number rather than a
//! trap**. The word generator reasons from `SIGTRAP` killing the process; that
//! reasoning does not transfer.
//!
//! And the differential cannot see it: **both implementations wrapping the same
//! way would AGREE and prove nothing.** So [`the_magnitude_bound_is_arithmetic`]
//! is the only thing standing between a green run and a vacuous one.
//!
//! # The scale, measured rather than assumed
//!
//! Bare `Fixed` is **`Fixed<32>`**, so a value `v` is stored as `v * 2^32`.
//! Measured: `3 / 3` yields `4294967296`, `3 * 2 + 2` yields `34359738368`, and
//! `-3` yields `-12884901888`. A previous increment's prose read raw operands as
//! Q16.16 and printed wrong decimals; the differential was unaffected but the
//! figures were not.
//!
//! # What this excludes, stated where a reader meets it
//!
//! **No `%`.** The reference traps on it and the backend refuses it — sound, and
//! generating it would turn that refusal into a stream of failures.
//!
//! **Every divisor is a non-zero literal**, so division by zero cannot arise
//! however the left side evaluates.
//!
//! **A bare `2.0` is a FLOAT literal**, not a `Fixed` one, and `Fixed + Float` is
//! a mixed pair the reference refuses. Literals are written `(N as Fixed)`.
//!
//! No comparisons, no composites, no streams.
//!
//! # What a green run may claim
//!
//! No divergence over the generated trees, at this commit, within these bounds.
//! **Not** that Fixed lowering is correct.

mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use std::collections::BTreeSet;

/// One, in Q32.32: a value `v` is stored as `v * ONE`.
const ONE: i64 = 1 << 32;

/// Integer leaves and arguments are bounded by this.
const LEAF_MAX: u64 = 3;
/// Tree depth. `LEAF_MAX ^ (2 ^ DEPTH)` must stay far inside the representable
/// range, which for Q32.32 means far inside `2^31`.
const DEPTH: u32 = 3;

/// The largest whole value Q32.32 represents, `2^63 / 2^32`.
const FIXED_WHOLE_LIMIT: i128 = 1i128 << 31;

/// The all-multiply worst case this file's overflow argument rests on.
fn worst_case_magnitude() -> i128 {
    (LEAF_MAX as i128).pow(1 << DEPTH)
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

/// Operators over a same-typed pair. **No `%`** — see the header.
const FIXED_OPS: &[&str] = &["+", "-", "*"];

fn leaf(rng: &mut Rng) -> String {
    match rng.below(4) {
        0 => "a".into(),
        1 => "b".into(),
        // **`(N as Fixed)`, not `N.0`.** A bare decimal literal is a FLOAT, and
        // `Fixed + Float` is a mixed pair the reference refuses.
        _ => format!("({} as Fixed)", rng.below(LEAF_MAX + 1)),
    }
}

fn expr(rng: &mut Rng, depth: u32) -> String {
    if depth == 0 {
        return leaf(rng);
    }
    match rng.below(6) {
        // A division whose divisor is a literal in `1..=3`, so it can never be
        // zero however the left side evaluates.
        0 => format!(
            "({} / ({} as Fixed))",
            expr(rng, depth - 1),
            rng.below(LEAF_MAX) + 1
        ),
        n => {
            let op = FIXED_OPS[(n as usize - 1) % FIXED_OPS.len()];
            format!("({} {op} {})", expr(rng, depth - 1), expr(rng, depth - 1))
        }
    }
}

fn program(rng: &mut Rng, depth: u32) -> String {
    format!(
        "fn main(a: Fixed, b: Fixed) -> Fixed {{ {} }}",
        expr(rng, depth)
    )
}

/// Drive both implementations on one generated program.
///
/// **A `Fixed` lowers with `i64` parameters**, because a Q-format value IS an
/// `i64` of fixed-point bits. The signature is asserted against the emitted
/// function rather than assumed, since a mismatched hand-named signature is
/// undefined behaviour this package has already paid for once.
fn drive(src: &str, a: i64, b: i64) -> (i64, i64) {
    let m = common::build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    let vm_out = match vm.call(&[Value::Fixed(a), Value::Fixed(b)]) {
        Ok(VmState::Finished(Value::Fixed(v))) => v,
        other => panic!("the reference did not finish with a Fixed on `{src}`: {other:?}"),
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
            f.get_nth_param(i).expect("a parameter").is_int_value(),
            "`{src}` parameter {i} is not an integer value, so the hand-named \
             signature below would be wrong and calling through it is undefined \
             behaviour"
        );
    }
    let g = unsafe { ee.get_function::<unsafe extern "C" fn(i64, i64) -> i64>(&sym) }
        .expect("entry symbol");
    let native = unsafe { g.call(a, b) };
    (vm_out, native)
}

const PROGRAMS: usize = 200;
const SEEDS: &[u64] = &[
    0xF10E_D000_0000_0001,
    0xF10E_D000_0000_0002,
    0xF10E_D000_0000_0003,
];
const DISTINCT_FLOOR: usize = 150;

/// **THE COMPARISON.**
#[test]
fn generated_fixed_trees_agree_with_the_reference() {
    let a = 3 * ONE;
    let b = 2 * ONE;

    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut ops: BTreeSet<&str> = BTreeSet::new();
    let mut compared = 0usize;

    for seed in SEEDS {
        let mut rng = Rng(*seed);
        for i in 0..PROGRAMS {
            let src = program(&mut rng, DEPTH);
            seen.insert(src.clone());
            for op in FIXED_OPS.iter().chain(["/"].iter()) {
                if src.contains(&format!(" {op} ")) {
                    ops.insert(op);
                }
            }

            let (vm, native) = drive(&src, a, b);

            // **THE OVERFLOW GUARD, APPLIED TO WHAT WAS ACTUALLY PRODUCED.** Both
            // implementations wrapping identically would agree and prove nothing,
            // so a result outside the representable whole range is a GENERATOR
            // defect and must fail rather than pass quietly.
            assert!(
                (vm as i128).abs() < (FIXED_WHOLE_LIMIT << 32),
                "the reference produced {vm} raw for `{src}`, outside the Q32.32 \
                 range. Fixed arithmetic is UNCHECKED, so this is a wrong number \
                 rather than a trap, and a matching wrong number on both sides \
                 would agree. Tighten LEAF_MAX or DEPTH."
            );

            compared += 1;
            assert_eq!(
                vm,
                native,
                "DIVERGENCE on generated Fixed program {i}:\n  {src}\n  \
                 reference = {vm} raw ({} whole)\n  native    = {native} raw\n\n\
                 The generator is seeded and deterministic. This establishes that \
                 the two implementations disagree, NOT which of them is right.",
                vm / ONE
            );
        }
    }

    assert_eq!(
        compared,
        PROGRAMS * SEEDS.len(),
        "only {compared} of {} programs were compared",
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
        FIXED_OPS.len() + 1,
        "only {} of {} operators appeared: {ops:?}",
        ops.len(),
        FIXED_OPS.len() + 1
    );
}

/// **THE OVERFLOW BOUND IS ARITHMETIC, AND THE ARITHMETIC IS CHECKED.**
///
/// The header argues the all-multiply worst case stays far inside the Q32.32
/// whole range. **An argument in a comment is not a guard**: raising `DEPTH` to 4
/// would give `3^16`, about 43 million, still inside — but raising `LEAF_MAX` to 9
/// at depth 3 gives `9^8`, about 43 million too, and at depth 4 it is `9^16`, far
/// outside. This fails instead of discovering it as a silent agreement.
#[test]
fn the_magnitude_bound_is_arithmetic() {
    assert!(
        worst_case_magnitude() < FIXED_WHOLE_LIMIT,
        "the all-multiply worst case is {}, at or above the Q32.32 whole limit of \
         {FIXED_WHOLE_LIMIT}. LEAF_MAX or DEPTH has been changed without redoing \
         the arithmetic, and Fixed arithmetic is UNCHECKED so the differential \
         would see two matching wrong numbers.",
        worst_case_magnitude()
    );

    // **NON-VACUITY.** A bound that nothing could exceed guards nothing.
    let reckless = 9i128.pow(1 << 4);
    assert!(
        reckless >= FIXED_WHOLE_LIMIT,
        "leaves of 9 at depth 4 no longer exceed the Q32.32 whole limit, so this \
         bound is not discriminating anything"
    );
}

/// **THE GENERATED PROGRAMS REALLY ARE FIXED, AND REALLY NEST.**
#[test]
fn the_generated_fixed_trees_are_fixed_and_nested() {
    let mut rng = Rng(0xF10E_D000_0000_00FF);
    let mut nested = 0usize;
    let mut with_literal = 0usize;
    for _ in 0..PROGRAMS {
        let src = program(&mut rng, DEPTH);
        assert!(
            src.contains("a: Fixed") && src.contains("-> Fixed"),
            "a generated program is not Fixed-typed: {src}"
        );
        // **NO BARE DECIMAL LITERAL.** One would be a Float and the reference
        // would refuse the mixed pair, which would look like a backend refusal.
        assert!(
            !src.contains(".0"),
            "a generated program contains a decimal literal, which is a FLOAT and \
             makes the pair mixed: {src}"
        );
        if src.matches('(').count() > 3 {
            nested += 1;
        }
        if src.contains("as Fixed") {
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
        "no generated program contains a Fixed literal, so the leaves are only \
         parameters and the literal path is untested"
    );
}
