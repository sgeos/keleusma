//! Independent checks on the per-operation models `analyze.kel` consumes.
//!
//! `analyze.kel` self-hosts the control-flow algorithm and the bound
//! extraction, NOT the models. It receives `Op::cost()`,
//! `Op::stack_growth()`/`Op::stack_shrink()`, `Op::heap_alloc()` and the derived
//! class and opcode-kind tables from the host, so the self-hosted differential
//! reproduces whatever the reference says and agrees with it BY CONSTRUCTION.
//!
//! **A differential against the model under test cannot detect that the model is
//! wrong.** The stack-effect model was found unsound while every differential in
//! the tree was green; see `tests/operand_stack_model.rs`. These are the
//! remaining models, each checked against a source that is not itself.

// It uses `Vm::new` / `keleusma::verify`, both of which the `verify` feature
// provides, so it needs that feature as well as `compile`. Gated on `compile`
// alone, `--features compile` did not BUILD. Found by the feature-combination
// sweep: no continuous-integration job and no release-gate step builds `compile`
// without `verify`, so nothing reported it.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::bytecode::{Chunk, Module, Op};

fn compile_src(src: &str) -> Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
    )
    .expect("compile")
}

fn chunk_named<'a>(m: &'a Module, name: &str) -> &'a Chunk {
    m.chunks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no chunk named {name}"))
}

/// What the model says a straight-line chunk allocates from the arena.
fn modelled_heap(chunk: &Chunk) -> u32 {
    chunk.ops.iter().map(|op| op.heap_alloc(chunk)).sum()
}

/// THE HEAP MODEL, AGAINST OBSERVED ARENA CONSUMPTION.
///
/// `heap_alloc_bytes` makes a falsifiable claim: **only `NewComposite`
/// allocates**, and it allocates exactly its operand's `alloc_bytes()`. The
/// arena itself is the independent source — it reports what was actually taken.
///
/// The comparison is against a BASELINE program with no composite construction,
/// so whatever the virtual machine spends on frames and bookkeeping cancels and
/// what remains is the composite allocation the model claims to predict.
#[test]
fn the_heap_model_matches_what_the_arena_actually_gives_out() {
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    use keleusma_arena::Arena;

    fn run_and_measure(src: &str) -> (usize, usize, u32) {
        let module = compile_src(src);
        let need = required_persistent_capacity_for(&module);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("persistent");
        let mut vm = Vm::new(module.clone(), &arena).expect("verify");
        let before = arena.bottom_used() + arena.top_used();
        match vm.call(&[]).expect("run") {
            VmState::Finished(_) => {}
            other => panic!("unexpected state {other:?}"),
        }
        let after = arena.bottom_used() + arena.top_used();
        let modelled = modelled_heap(chunk_named(&module, "main"));
        (before, after, modelled)
    }

    // Baseline: no composite construction anywhere.
    let (b0, a0, m0) = run_and_measure("fn main() -> Word { 1 + 2 }");
    assert_eq!(
        m0, 0,
        "the model claims a composite-free chunk allocates {m0} bytes"
    );
    let baseline_observed = a0 - b0;

    // Every composite shape the compiler can construct in a straight line.
    const CASES: &[(&str, &str)] = &[
        ("tuple", "fn main() -> (Word, Word) { (1, 2) }"),
        ("array", "fn main() -> [Word; 4] { [1, 2, 3, 4] }"),
        (
            "struct",
            "struct S { a: Word, b: Word }\nfn main() -> S { S { a: 1, b: 2 } }",
        ),
        ("enum", "enum E { A, B }\nfn main() -> E { E::A }"),
        (
            "nested-tuple",
            "fn main() -> ((Word, Word), Word) { ((1, 2), 3) }",
        ),
        (
            "array-of-struct",
            "struct S { a: Word }\nfn main() -> [S; 2] { [S { a: 1 }, S { a: 2 }] }",
        ),
        (
            "byte-array",
            "fn main() -> [Byte; 4] { [1 as Byte, 2 as Byte, 3 as Byte, 4 as Byte] }",
        ),
    ];

    let mut checked = 0;
    for (label, src) in CASES {
        let (b, a, m) = run_and_measure(src);
        let observed = (a - b) - baseline_observed;
        assert!(
            m > 0,
            "{label}: the model predicts no allocation, so this case tests nothing"
        );
        assert_eq!(
            observed as u32, m,
            "{label}: the arena gave out {observed} bytes above baseline and the model \
             predicted {m}. A model UNDER the observation is unsound: the worst-case-memory \
             bound would be smaller than the arena the program actually consumes."
        );
        checked += 1;
    }
    assert_eq!(
        checked,
        CASES.len(),
        "not every composite shape was measured"
    );
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/keleusma-bench/measured_cost_models/aarch64_apple_darwin.rs"
));

/// THE COST MODEL, AGAINST THE MEASUREMENTS ITS OWN GENERATOR RECORDED.
///
/// `NOMINAL_COST_MODEL` documents itself as "unmeasured estimates chosen for
/// RELATIVE ORDERING, not measured pipelined cycles". So equality with a
/// measured model is the wrong test; ORDERING is the claim, and the committed
/// per-opcode measurements in `keleusma-bench/measured_cost_models/` are an
/// independent source for it.
///
/// **TWO FINDINGS, BOTH PINNED HERE RATHER THAN REPAIRED.** Repairing either is
/// a judgment call about a calibration, not a correctness fix, and belongs to
/// whoever owns the cost model.
///
/// **1. The nominal model separates `{Div, Mod}` from `{CmpEq, CmpLt}` into
/// different tiers; measurement puts them in one band.** Nominal says
/// `Div = 3` against `CmpEq = 2`. Measured on aarch64, with the SAME
/// `ops_per_pattern: 4` so the setup overhead is comparable: `Div` 138.56,
/// `Mod` 139.36, `CmpEq` 140.70, `CmpLt` 133.55. Those four are within seven
/// cycles of each other, and `Div` is the CHEAPEST of them. The nominal tier
/// boundary is not supported by the measurement.
///
/// **2. The generator discards measured values into buckets.** `CmpEq` measured
/// 140.70 and is emitted as 164; `CmpLt` measured 133.55 and is emitted as 164.
/// Overstating is conservative for a worst-case bound and is therefore safe,
/// but it destroys the ordering the model exists to provide -- and it is what
/// creates the apparent 140-against-164 gap between division and arithmetic in
/// the emitted model, which the raw measurements do not show.
///
/// **WHAT THIS DOES NOT ESTABLISH.** Only 17 opcodes were ever measured, of an
/// instruction set of 66. Every other value in the emitted model is a bucket
/// assignment, not a measurement, so no ordering claim about them is checked by
/// anything. `Op::Add` itself is among the unmeasured: the arithmetic bucket was
/// measured through `CheckedAdd`/`CheckedSub`/`CheckedMul`, whose bench pattern
/// tears down three stack slots against division's one, so even that comparison
/// is confounded and is deliberately NOT asserted below.
#[test]
fn the_nominal_cost_ordering_agrees_with_the_raw_measurements() {
    use keleusma::bytecode::nominal_op_cycles as nominal;

    // Per-opcode CPU cycles as recorded in the provenance header of
    // `keleusma-bench/measured_cost_models/aarch64_apple_darwin.rs`. These are
    // the RAW measurements, before the generator's bucketing -- which is the
    // point: the bucketed output is not a faithful independent source.
    //
    // Restricted to opcodes sharing `ops_per_pattern: 4`, so setup and teardown
    // overhead is comparable and a cross-opcode comparison means something.
    const MEASURED_PATTERN_4: &[(&str, Op, f64)] = &[
        ("Div", Op::Div, 138.5646),
        ("Mod", Op::Mod, 139.3568),
        ("CmpEq", Op::CmpEq, 140.6964),
        ("CmpLt", Op::CmpLt, 133.5504),
    ];

    // The band these four occupy, measured. If the nominal model separated them
    // into tiers correctly, the tier boundary would fall inside this band.
    let lo = MEASURED_PATTERN_4
        .iter()
        .map(|(_, _, c)| *c)
        .fold(f64::INFINITY, f64::min);
    let hi = MEASURED_PATTERN_4
        .iter()
        .map(|(_, _, c)| *c)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        hi - lo < 10.0,
        "the four comparably-measured opcodes span {:.1} cycles, so treating them as one \
         band no longer holds and this test's premise needs remeasuring",
        hi - lo
    );

    // THE PINNED DISAGREEMENT. Nominal splits this one measured band into two
    // tiers. Recorded as an assertion so that a change to either side reports
    // itself: fixing the nominal model fires this, and so does a remeasurement
    // that moves the band.
    let tiers: std::collections::BTreeSet<u32> = MEASURED_PATTERN_4
        .iter()
        .map(|(_, op, _)| nominal(op))
        .collect();
    assert_eq!(
        tiers.len(),
        2,
        "the nominal model no longer splits the measured band into exactly two tiers; \
         re-read the finding in this test's documentation before adjusting the number"
    );

    // And the cheapest of the four by measurement sits in the EXPENSIVE nominal
    // tier, which is the inversion stated plainly.
    let cheapest = MEASURED_PATTERN_4
        .iter()
        .min_by(|a, b| a.2.partial_cmp(&b.2).expect("finite"))
        .expect("non-empty");
    let dearest = MEASURED_PATTERN_4
        .iter()
        .max_by(|a, b| a.2.partial_cmp(&b.2).expect("finite"))
        .expect("non-empty");
    assert_eq!(
        cheapest.0, "CmpLt",
        "the cheapest comparably-measured opcode changed"
    );
    assert_eq!(
        dearest.0, "CmpEq",
        "the dearest comparably-measured opcode changed"
    );
    assert!(
        nominal(&Op::Div) > nominal(&Op::CmpEq),
        "the nominal model no longer ranks Div above CmpEq -- if that was a deliberate \
         repair, delete this assertion and the finding it pins"
    );
    assert!(
        MEASURED_PATTERN_4[0].2 < MEASURED_PATTERN_4[2].2,
        "Div measured dearer than CmpEq, reversing the recorded finding"
    );
}
