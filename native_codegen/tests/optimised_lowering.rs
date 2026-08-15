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

    let before = instruction_count(&lm);
    lm.run_passes("default<O2>", &machine(), PassBuilderOptions::create())
        .expect("O2 pipeline");
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
