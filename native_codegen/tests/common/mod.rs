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

/// The corpus roots every sweep on this line is derived from, relative to
/// `native_codegen/`.
///
/// Kept beside [`corpus_sources`] so the roots and the walk cannot drift apart.
/// `corpus_fingerprint.rs` pins the CONTENT of these directories; this pins the
/// POPULATION read out of them.
pub const CORPUS_ROOTS: [&str; 4] = [
    "../examples/scripts",
    "../src/selfhost/kel",
    "../examples/rtos/scripts",
    "../compiler/kel",
];

/// **THE canonical corpus enumeration. One copy, so sweeps cannot disagree.**
///
/// # Why this is shared rather than repeated
///
/// Five defects on this line took the same shape: a measurement enumerated a
/// **narrower population than the thing it described**, then reported the
/// difference as a property of the subjects. A non-recursive walk saw 35 modules
/// where its consumers saw 74; a fingerprint covered three roots where consumers
/// read four; a directory listed explicitly *and* reached by recursion was
/// counted twice.
///
/// `corpus_fingerprint.rs` closed the neighbouring hole — the corpus content —
/// and its own header states the argument for this one: *"A habit is not a
/// check."* Keeping the walk in one place makes divergence impossible for callers
/// that use it, the same move that made two mutation censuses agree by
/// construction rather than by comparison.
///
/// **This eliminates the class for CALLERS OF THIS FUNCTION only.** A test still
/// carrying its own walk remains exposed.
///
/// # What it does and does not do
///
/// Enumerates `.kel` files recursively under [`CORPUS_ROOTS`], sorted and
/// deduplicated — the dedup matters because listing a root and also reaching it
/// by recursion is one of the five defects above. **It does not LOAD them.**
/// Loading is separate and some sources need a prelude prepended; unifying the
/// walk must not disturb that.
#[allow(dead_code)]
pub fn corpus_sources() -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    for dir in CORPUS_ROOTS {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut stack: Vec<std::path::PathBuf> = rd.flatten().map(|e| e.path()).collect();
        while let Some(p) = stack.pop() {
            if p.is_dir() {
                if let Ok(rd2) = std::fs::read_dir(&p) {
                    stack.extend(rd2.flatten().map(|e| e.path()));
                }
            } else if p.extension().is_some_and(|x| x == "kel") {
                out.push(p);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
