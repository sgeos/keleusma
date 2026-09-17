//! **Does the emitted IR survive the optimiser?**
//!
//! Every execution differential on this line runs at `OptimizationLevel::None`.
//! That level is a CODEGEN setting: `mem2reg` and the rest of the middle end are
//! a pass pipeline and do not run from it. Undefined behaviour in emitted IR is
//! invisible at `-O0` and appears at `-O2`, so the differentials have never
//! tested for it.
//!
//! # A correction to how this gap was first described
//!
//! It was stated as "no differential and no object file has ever been
//! optimised". **The second half is wrong.** `aot_linkage.rs` runs
//! `default<O2>` and links the result into a running C program, and its header
//! says that is exactly why it exists. The real gap is narrower: **one**
//! hand-written module has been through the middle end, against **thirty-seven**
//! in the corpus.
//!
//! `corpus_differential` now runs the whole corpus through `default<O2>` when
//! `KEL_OPTIMIZE` is set. This file is the guard that makes that run mean
//! something.
mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use keleusma::bytecode::Module;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};

fn module_of(path: &str) -> Module {
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile")
}

fn machine() -> TargetMachine {
    Target::initialize_native(&InitializationConfig::default()).expect("init target");
    let triple = TargetMachine::get_default_triple();
    Target::from_triple(&triple)
        .expect("target")
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine")
}

/// Instructions across every function, as a proxy for "did the pipeline do
/// anything at all".
fn instruction_count(m: &inkwell::module::Module<'_>) -> usize {
    let mut n = 0;
    for f in m.get_functions() {
        for bb in f.get_basic_blocks() {
            let mut i = bb.get_first_instruction();
            while let Some(ins) = i {
                n += 1;
                i = ins.get_next_instruction();
            }
        }
    }
    n
}

/// **THE VACUITY GUARD FOR PART B.**
///
/// A green optimised differential proves nothing if the pipeline never ran. This
/// asserts that `default<O2>` measurably transforms a real corpus module, so the
/// corpus-wide green result is evidence about optimised code rather than about a
/// no-op.
///
/// The whole increment this belongs to began with nine modules agreeing while
/// doing nothing. An unguarded "it passes under O2" would be the same mistake in
/// a new place.
#[test]
fn the_o2_pipeline_measurably_transforms_a_real_module() {
    let m = module_of("../examples/scripts/09_big_numbers.kel");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    lm.verify().expect("valid IR before optimisation");

    // ⚠ **THROUGH THE SHARED HELPER, NOT AN INLINE PIPELINE.**
    //
    // This test ran `run_passes` itself until 2026-09-17, which meant it guarded
    // a pipeline **nothing else used**. Measured: stubbing `common::force_optimize`
    // to return immediately left this test and both of its neighbours PASSING, so
    // the entire optimised-execution coverage would have survived a dead
    // optimiser. Routing it through the helper every optimised path shares is what
    // gives the others their reach.
    let before = instruction_count(&lm);
    common::force_optimize(&lm);
    let after = instruction_count(&lm);
    lm.verify().expect("valid IR AFTER optimisation");

    println!("  instructions before O2: {before}, after: {after}");
    assert!(
        before > 0,
        "the module lowered to no instructions; the comparison is vacuous"
    );
    assert_ne!(
        before, after,
        "`default<O2>` left the instruction count unchanged at {before}. Either the \
         pipeline did not run or it found nothing to do, and in both cases the \
         corpus-wide `KEL_OPTIMIZE` run is not evidence about optimised code."
    );
}

/// The IR the emitter produces must still VERIFY after the middle end.
///
/// A module can be valid before optimisation and invalid after it when the
/// emitter has relied on something it was not granted. Checked across several
/// corpus modules of different shapes rather than the one above.
#[test]
fn corpus_modules_still_verify_after_the_middle_end() {
    let paths = [
        "../examples/scripts/01_arithmetic.kel",
        "../examples/scripts/02_struct_field.kel",
        "../examples/scripts/03_enum_match.kel",
        "../examples/scripts/09_big_numbers.kel",
        "../examples/scripts/10_multbyte.kel",
        "../src/selfhost/kel/lexer.kel",
    ];
    let mach = machine();
    let mut checked = 0;
    for p in paths {
        let m = module_of(p);
        if !keleusma_native::module_refusals(&m, LowerOptions::default()).is_empty() {
            continue;
        }
        let ctx = Context::create();
        let lm = ctx.create_module("kel");
        lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
        lm.verify()
            .unwrap_or_else(|e| panic!("{p} invalid BEFORE O2: {e}"));
        lm.run_passes("default<O2>", &mach, PassBuilderOptions::create())
            .unwrap_or_else(|e| panic!("{p} O2 pipeline failed: {e}"));
        lm.verify()
            .unwrap_or_else(|e| panic!("{p} invalid AFTER O2: {e}"));
        checked += 1;
    }
    assert!(
        checked >= 5,
        "only {checked} modules were checked; the assertion is thin"
    );
}

// ---------------------------------------------------------------------------
// OPTIMISED **EXECUTION**, COVERED BY EVERY GATE RATHER THAN BY A SWEEP
// ---------------------------------------------------------------------------
//
// # The distinction this closes
//
// `corpus_modules_still_verify_after_the_middle_end` above asks whether the IR
// is still VALID after `default<O2>`. **That is not whether it still computes the
// same values**, and undefined behaviour characteristically manifests as a wrong
// result rather than as invalid IR — an optimiser is entitled to assume UB does
// not happen and to fold accordingly, producing IR that verifies perfectly and
// answers wrongly.
//
// # And the corpus-wide sweep was never actually run until 2026-09-17
//
// `corpus_differential` honours `KEL_OPTIMIZE`, but **no script sets it**:
// `tools/backend-gate.sh` does not, and continuous integration does not build
// this package at all. The capability existed, was documented, and had never
// produced a result.
//
// **It was run on 2026-09-17: 10 tests, 0 failed, frozen tree, 382s** — inside
// the 379-433s range of six unoptimised runs of the same phase that day, so the
// middle end costs approximately nothing here. **The variable's reach was proven
// before the result was believed**: with it set, a probe inside the hook panics;
// without it, the same test passes.
//
// A sweep is still a sweep. The subjects below are driven through the middle end
// on EVERY run, so the coverage does not depend on anyone remembering.

/// Subjects chosen for what an optimiser is most likely to disturb.
const OPTIMISED_SUBJECTS: &[(&str, &str)] = &[
    // Region aliasing: the backend hands every call site a disjoint block of the
    // caller's buffer, and alias analysis is exactly what `-O2` sharpens.
    (
        "a composite returned through the caller's region",
        "struct P { x: Word, y: Word }\nfn mk(a: Word, b: Word) -> P { P { x: a, y: b } }\nfn main(a: Word, b: Word) -> Word { let p: P = mk(a, b); p.x + p.y }",
    ),
    // Checked arithmetic lowers to intrinsics with branches an optimiser folds.
    (
        "checked arithmetic near its boundary",
        "fn main(a: Word, b: Word) -> Word { (a + b) * (a - b) }",
    ),
    // Nested conditionals, which `-O2` folds and re-associates aggressively.
    //
    // ⚠ **THIS ROW REPLACES A LOOP SUBJECT THAT DID NOT PARSE.** It was written
    // with `let mut`, and **Keleusma has no mutable local** — the accumulator
    // shape it assumed does not exist in the language. A probe implicating itself
    // rather than the backend, which this line has now done five times; the
    // failure was a `ParseError`, not a divergence.
    (
        "nested conditionals",
        "fn main(a: Word, b: Word) -> Word { if a > b { if a > 0 { a - b } else { b - a } } else { if b > 0 { b + a } else { 0 } } }",
    ),
    // Nested field reads through two levels of flat offsets.
    (
        "a nested composite read",
        "struct I { v: Word }\nstruct O { i: I, w: Word }\nfn main(a: Word, b: Word) -> Word { let o: O = O { i: I { v: a }, w: b }; o.i.v + o.w }",
    ),
];

/// **The optimised results must equal the reference's, not merely verify.**
#[test]
fn the_subjects_execute_identically_after_the_middle_end() {
    for (label, src) in OPTIMISED_SUBJECTS {
        let (vm, plain) = common::vm_and_native_two_arg_at(src, 9, 4, false);
        let (_, optimised) = common::vm_and_native_two_arg_at(src, 9, 4, true);
        assert_eq!(
            vm, plain,
            "`{label}` already disagrees at -O0, so the optimised comparison below \
             would be measuring the wrong thing"
        );
        assert_eq!(
            vm, optimised,
            "`{label}` DIVERGES after `default<O2>`: the reference gives {vm}, the \
             optimised lowering gives {optimised}, and the unoptimised lowering \
             gives {plain}. IR that verifies after the middle end can still answer \
             wrongly -- that is the whole reason this test exists beside the \
             verification one. This establishes that the two implementations \
             disagree, NOT which of them is right."
        );
    }
}
