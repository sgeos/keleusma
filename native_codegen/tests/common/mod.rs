//! Shared test support: run the shipping middle end on demand.
//!
//! # Why this exists
//!
//! Every differential on this line creates its JIT at
//! `OptimizationLevel::None`. That is a CODEGEN setting; `mem2reg` and the rest
//! of the middle end are a pass pipeline and do not run from it. Undefined
//! behaviour in emitted IR is invisible at `-O0` and appears at `-O2`.
//!
//! `corpus_differential` gained a `KEL_OPTIMIZE` hook first, and covering only
//! that one left the HAND-WRITTEN differentials unoptimised — including
//! `composite_return_aliasing`, which pins the composite-return aliasing defect,
//! the only genuine codegen defect this line has found. Region aliasing is
//! exactly the sort of thing an optimiser reasons about, so leaving that case at
//! `-O0` was the wrong one to leave.
//!
//! # Deliberately opt-in
//!
//! The default stays `None` so the everyday suite keeps its current meaning and
//! runtime, and the optimised run is a separate, explicit pass over the same
//! tests. Setting `KEL_OPTIMIZE` turns it on everywhere at once.

/// Run `default<O2>` over `lm` when `KEL_OPTIMIZE` is set, otherwise do nothing.
///
/// Call it AFTER `lower_module` and `verify`, and BEFORE creating the execution
/// engine. Verifying first keeps a pre-existing IR defect distinguishable from
/// one the optimiser introduces.
#[allow(dead_code)]
pub fn maybe_optimize(lm: &inkwell::module::Module<'_>) {
    if std::env::var("KEL_OPTIMIZE").is_err() {
        return;
    }
    use inkwell::OptimizationLevel;
    use inkwell::passes::PassBuilderOptions;
    use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};

    Target::initialize_native(&InitializationConfig::default()).expect("init native target");
    let triple = TargetMachine::get_default_triple();
    let machine = Target::from_triple(&triple)
        .expect("target")
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine");
    lm.run_passes("default<O2>", &machine, PassBuilderOptions::create())
        .expect("O2 pipeline");
    // A module that verified before the pipeline and not after it is the
    // finding this whole exercise is looking for, so it fails loudly here
    // rather than surfacing later as a wrong value.
    lm.verify().expect("IR still valid AFTER the O2 pipeline");
}
