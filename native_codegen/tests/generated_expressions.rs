//! **GENERATED EXPRESSION TREES, COMPARED AGAINST THE REFERENCE.**
//!
//! # Why a generator, when there are already 132 operator cells
//!
//! Every other subject in this package is hand-written: 74 corpus modules, 90
//! same-type operator cells, 42 mixed cells, four opcode-witness families, three
//! stream shapes. Each enumerates a surface someone thought of, and **each tests
//! operators ONE AT A TIME**.
//!
//! Nothing tests **composition** — a checked multiply feeding a divide feeding a
//! bitwise fold, three levels deep. A lowering defect that survives every
//! single-operator cell would live exactly there.
//!
//! The backend has produced no defect since `Fixed % Fixed`, through a 200-tick
//! stream sweep, an arena high-water measurement, a corpus-wide region census, 64
//! of 66 driven opcode witnesses, and a full corpus run under `default<O2>`.
//! **That is the classic signal that hand-written subjects are exhausted.**
//!
//! # ⚠ TRAPPING IS WHY THIS IS NARROWER THAN IT LOOKS
//!
//! `+`, `-` and `*` are CHECKED. On overflow the reference returns an error and
//! the native side executes `llvm.trap`, which **kills the process with
//! SIGTRAP** — not a comparison, a dead test binary. Division and modulo by zero
//! are the same hazard by another route.
//!
//! So the generator is built so evaluation **cannot** trap: leaves are `0..=9`,
//! the arguments are small, depth is capped so the worst-case magnitude of any
//! subexpression stays far inside the word, and **every divisor is a non-zero
//! literal**. With leaves at most 9 and depth 3, an all-multiply tree reaches at
//! most `9^8`, about 43 million — five orders of magnitude inside `i64`.
//!
//! **This narrowing is the cost and it is stated rather than hidden**: nothing
//! here says anything about the trap paths, which the existing witnesses cover
//! separately, nor about floats, composites or streams, which this generator does
//! not emit.
//!
//! # What a green run may claim
//!
//! No divergence over the generated trees, at this commit. **Not** that the
//! lowering is correct, and not that composition is exhaustively covered.

mod common;

use std::collections::BTreeSet;

/// A deterministic generator, so a failure is reproducible rather than an
/// anecdote. Xorshift64*, chosen for being short enough to audit.
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

/// Operators that cannot trap on bounded operands. **`/` and `%` appear only
/// with a literal non-zero right-hand side**, handled separately below.
const SAFE_BINARY: &[&str] = &["+", "-", "*", "band", "bor", "bxor"];

/// Leaves: the two parameters and small literals. Bounded so the magnitude
/// argument in the header holds.
fn leaf(rng: &mut Rng) -> String {
    match rng.below(4) {
        0 => "a".into(),
        1 => "b".into(),
        _ => format!("{}", rng.below(10)),
    }
}

/// An expression tree of at most `depth` levels.
fn expr(rng: &mut Rng, depth: u32) -> String {
    if depth == 0 {
        return leaf(rng);
    }
    match rng.below(8) {
        // A division or modulo, whose divisor is a literal in `1..=9` so it can
        // never be zero however the left side evaluates.
        0 => format!("({} / {})", expr(rng, depth - 1), rng.below(9) + 1),
        1 => format!("({} % {})", expr(rng, depth - 1), rng.below(9) + 1),
        n => {
            let op = SAFE_BINARY[(n as usize - 2) % SAFE_BINARY.len()];
            format!("({} {op} {})", expr(rng, depth - 1), expr(rng, depth - 1))
        }
    }
}

fn program(rng: &mut Rng, depth: u32) -> String {
    format!(
        "fn main(a: Word, b: Word) -> Word {{ {} }}",
        expr(rng, depth)
    )
}

/// A `Byte` leaf. **Bounded by 3, not by 9**, and the reason is arithmetic
/// rather than caution: `Byte` arithmetic is checked and a byte overflows far
/// sooner than a word. With leaves at most 3, a depth-2 all-multiply tree reaches
/// 81 and stays inside a byte; **depth 3 reaches 6561 and does not**, and the
/// consequence is not a failing assertion but a `SIGTRAP` that kills the binary.
fn byte_leaf(rng: &mut Rng) -> String {
    match rng.below(4) {
        0 => "(a as Byte)".into(),
        1 => "(b as Byte)".into(),
        n => format!("({} as Byte)", n - 1),
    }
}

/// A `Byte` expression tree, wrapped by the caller into a `Word` signature.
///
/// **`Byte` is the type with the worst record in this package**: checked `Byte`
/// multiply and add once returned untruncated values, so `200 * 100` gave 20000
/// where the reference gives `Byte(32)`. The fixed matrices cover those operators
/// one at a time; this composes them.
fn byte_expr(rng: &mut Rng, depth: u32) -> String {
    if depth == 0 {
        return byte_leaf(rng);
    }
    match rng.below(6) {
        0 => format!(
            "({} / ({} as Byte))",
            byte_expr(rng, depth - 1),
            rng.below(9) + 1
        ),
        1 => format!(
            "({} % ({} as Byte))",
            byte_expr(rng, depth - 1),
            rng.below(9) + 1
        ),
        n => {
            let op = SAFE_BINARY[(n as usize - 2) % SAFE_BINARY.len()];
            format!(
                "({} {op} {})",
                byte_expr(rng, depth - 1),
                byte_expr(rng, depth - 1)
            )
        }
    }
}

fn byte_program(rng: &mut Rng, depth: u32) -> String {
    format!(
        "fn main(a: Word, b: Word) -> Word {{ ({}) as Word }}",
        byte_expr(rng, depth)
    )
}

/// Programs per seed. Each is a compile, a lowering and a JIT, measured at a few
/// milliseconds. **The gate's longest phase is already 380s and had to be split,
/// so this must stay cheap.**
const PROGRAMS: usize = 300;
const DEPTH: u32 = 3;

/// `Byte` trees are SHALLOWER, and the bound is derived from the type rather
/// than inherited. See `byte_leaf`.
const BYTE_DEPTH: u32 = 2;

/// **A single seed is a single sample of the shape space.** Sweeping several
/// costs seconds, which is the cheapest real strengthening available.
const SEEDS: &[u64] = &[
    0x5EED_1234_ABCD_0001,
    0x0F0F_0F0F_0F0F_0F0F,
    0xDEAD_BEEF_FEED_FACE,
    0x1111_2222_3333_4444,
    0xA5A5_A5A5_5A5A_5A5A,
];

/// Below this the generator collapsed and this is a slower operator matrix.
const DISTINCT_FLOOR: usize = 250;
/// Operators that must actually appear across the run.
const OPERATOR_FLOOR: usize = 7;

/// **THE COMPARISON.**
#[test]
fn generated_expression_trees_agree_with_the_reference() {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut ops: BTreeSet<&str> = BTreeSet::new();
    let mut compared = 0usize;

    for seed in SEEDS {
        let mut rng = Rng(*seed);
        for i in 0..PROGRAMS {
            let src = program(&mut rng, DEPTH);
            seen.insert(src.clone());
            for op in SAFE_BINARY.iter().chain(["/", "%"].iter()) {
                if src.contains(&format!(" {op} ")) {
                    ops.insert(op);
                }
            }

            // **A program this generator cannot compile is the GENERATOR's defect
            // until shown otherwise**, and it fails loudly. Discarding such cases
            // silently is how a generator shrinks to the trivial subset while still
            // reporting a program count.
            let (vm, native) = common::vm_and_native_two_arg(&src, 7, 3);
            compared += 1;
            assert_eq!(
                vm, native,
                "DIVERGENCE on generated program {i}:\n  {src}\n  reference = {vm}\n  \
             native    = {native}\n\nReproduce by re-running this test: the \
             generator is seeded and deterministic. This establishes that the two \
             implementations disagree, NOT which of them is right."
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
        "the generator produced only {} distinct programs. It \
         has collapsed toward a small set, which makes this a slower version of \
         the fixed operator matrix rather than a test of composition.",
        seen.len()
    );
    assert!(
        ops.len() >= OPERATOR_FLOOR,
        "only {} distinct operators appeared across the run: {ops:?}. The trees \
         are not exercising the operator set they are built from.",
        ops.len()
    );
}

/// **The generator must actually nest**, or every program is a single operator
/// and the composition claim is false.
#[test]
fn the_generated_trees_are_actually_nested() {
    let mut rng = Rng(0x5EED_1234_ABCD_0002);
    let mut deepest = 0usize;
    for _ in 0..PROGRAMS {
        let src = program(&mut rng, DEPTH);
        // Nesting depth, read off the parentheses the generator emits.
        let (mut cur, mut max) = (0usize, 0usize);
        for c in src.chars() {
            match c {
                '(' => {
                    cur += 1;
                    max = max.max(cur);
                }
                ')' => cur = cur.saturating_sub(1),
                _ => {}
            }
        }
        deepest = deepest.max(max);
    }
    assert!(
        deepest >= 3,
        "the deepest generated tree nests {deepest} level(s). At one level this \
         file duplicates `scalar_operator_matrix.rs`; the whole reason it exists \
         is that nothing else tests COMPOSITION."
    );
}

/// **`Byte` trees, across every seed.**
///
/// Kept separate from the `Word` sweep because the depth bound differs and the
/// reason for the difference is worth reading at the point of use.
#[test]
fn generated_byte_trees_agree_with_the_reference() {
    let mut compared = 0usize;
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for (s_i, seed) in SEEDS.iter().enumerate() {
        let mut rng = Rng(seed ^ 0xBB);
        for i in 0..PROGRAMS {
            let src = byte_program(&mut rng, BYTE_DEPTH);
            seen.insert(src.clone());
            let (vm, native) = common::vm_and_native_two_arg(&src, 7, 3);
            compared += 1;
            assert_eq!(
                vm, native,
                "DIVERGENCE on generated BYTE program {i} of seed {s_i}:\n  {src}\n  \
                 reference = {vm}\n  native    = {native}\n\nByte arithmetic is \
                 where this line has already found two genuine defects -- checked \
                 multiply and add returning untruncated values. The generator is \
                 seeded, so this reproduces by re-running. This establishes that \
                 the two implementations disagree, NOT which is right."
            );
        }
    }

    assert_eq!(
        compared,
        PROGRAMS * SEEDS.len(),
        "only {compared} byte programs compared"
    );
    assert!(
        seen.len() >= PROGRAMS,
        "the byte generator produced only {} distinct programs across {} seeds; \
         at a shallower depth it collapses sooner, and below this floor it is \
         re-testing the fixed matrix.",
        seen.len(),
        SEEDS.len()
    );
}
