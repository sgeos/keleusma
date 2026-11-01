//! **WHAT WOULD A REAL FLOAT ABI ACTUALLY UNBLOCK?**
//!
//! The operator ruled Option A for the float ABI on 2026-08-29: give float-typed
//! entries a real floating-point ABI rather than the current `i64`. This file
//! measures the scope of that ruling **before** any of it is built, because this
//! line has three times in recent increments acted on a plausible guess about
//! where work sat and been wrong.
//!
//! # OUTCOME, 2026-08-30: the constant route is open and the prediction held
//!
//! Slice two opened the CONSTANT route only. `float_witness.kel` lowers and
//! agrees with the reference in the corpus differential. The signature, native
//! return and data-slot routes still refuse, because nothing is built behind
//! them.
//!
//! # The result sharpens the ruling
//!
//! The ruling names the **entry ABI**. The corpus's only float-carrying module is
//! blocked by a float **CONSTANT**, and **no corpus module has a float in a
//! signature at all** — so the entry-ABI change has **zero corpus witnesses** and
//! could not be verified against the corpus if it were built alone. What blocks
//! lowering today is float *representation*: `f64_type` and constant emission.
//!
//! Option A as recorded covers both (`f64_type`, FP registers, float opcodes),
//! so the ruling is not wrong — but "the entry ABI" understates the work, and a
//! reader planning from the phrase alone would build the wrong piece first.
//!
//! # A width assumption, which is MINE and not the operator's
//!
//! `Float` is `f32` or `f64` depending on the `narrow-float-32` feature, so
//! "double" is incoherent in a build with no `f64`. **The reading proceeded on is
//! that the floating-point type matches the runtime's float width.** Recorded as
//! an assumption; see `docs/decisions/ABI_RULINGS.md`.

mod common;

use keleusma::bytecode::{ConstValue, WireShape};

const FLOAT_TAG: u8 = 5;

fn is_float(w: &WireShape) -> bool {
    matches!(w, WireShape::Scalar { kind } if *kind == FLOAT_TAG)
}

fn compiled(src: &str) -> Option<keleusma::bytecode::Module> {
    keleusma::lexer::tokenize(src)
        .ok()
        .and_then(|t| keleusma::parser::parse(&t).ok())
        .and_then(|a| keleusma::compiler::compile(&a).ok())
}

/// The scope of the float ruling, pinned so it cannot drift unnoticed.
#[test]
fn what_a_float_abi_would_unblock() {
    let mut modules = 0usize;
    let mut by_signature: Vec<String> = Vec::new();
    let mut by_constant: Vec<String> = Vec::new();
    let mut by_native_return: Vec<String> = Vec::new();
    let mut refused_for_float: Vec<String> = Vec::new();

    for p in common::corpus_sources() {
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Some(m) = compiled(&src) else { continue };
        modules += 1;
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if m.signatures
            .iter()
            .any(|sg| is_float(&sg.ret) || sg.params.iter().any(is_float))
        {
            by_signature.push(name.clone());
        }
        if m.chunks.iter().any(|c| {
            c.constants
                .iter()
                .any(|k| matches!(k, ConstValue::Float(_)))
        }) {
            by_constant.push(name.clone());
        }
        if m.native_return_shapes.iter().any(is_float) {
            by_native_return.push(name.clone());
        }
        if keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default())
            .iter()
            .any(|(_, e)| format!("{e}").contains("Float"))
        {
            refused_for_float.push(name);
        }
    }

    println!("\n================ FLOAT ABI SCOPE, measured before building");
    println!("  corpus modules compiled     : {modules}");
    println!(
        "  float via SIGNATURE         : {} {by_signature:?}",
        by_signature.len()
    );
    println!(
        "  float via CONSTANT          : {} {by_constant:?}",
        by_constant.len()
    );
    println!(
        "  float via NATIVE RETURN     : {} {by_native_return:?}",
        by_native_return.len()
    );
    println!(
        "  modules REFUSED for a float : {} {refused_for_float:?}",
        refused_for_float.len()
    );
    println!(
        "\n  The ruling names the ENTRY ABI; the corpus is blocked by a CONSTANT.\n  \
         The entry-ABI change has no corpus witness, so building it alone could\n  \
         not be verified here."
    );
    println!("================\n");

    assert!(
        modules > 40,
        "only {modules} modules compiled, so this scope measurement reads a much \
         narrower population than the censuses it informs"
    );
    // **The finding, pinned.** If a corpus module gains a float in a signature,
    // the entry-ABI change acquires a witness and the plan above changes.
    assert!(
        by_signature.is_empty(),
        "a corpus module now carries a float in a SIGNATURE: {by_signature:?}. The \
         entry ABI now has a corpus witness, which it did not when the float \
         ruling was scoped, so re-read that plan before building to it"
    );
    // **THE PREMISE IS SPENT, AS THIS ASSERTION ANTICIPATED.** It used to require
    // that some module was refused for a float. Slice two opened the CONSTANT
    // route, and `float_witness.kel` now lowers and AGREES with the reference in
    // the corpus differential — so nothing is refused for a float any more.
    //
    // **The scope measurement this file made was correct**: exactly one module,
    // reached by a constant and not by a signature. That prediction is what the
    // slice was planned from, and recording that it held is the reason to keep
    // the file rather than delete it.
    assert!(
        refused_for_float.is_empty(),
        "a module is refused for a float again: {refused_for_float:?}. Slice two \
         opened the constant route and verified the witness differentially, so a \
         refusal here is a regression rather than the old guard"
    );
}

/// **THE FLOAT SLOT ROUTE'S CORPUS POPULATION, READ AS DATA.**
///
/// The prediction recorded before the float shared slot was built was that no
/// corpus module declares one, so the route has zero corpus witnesses and every
/// census stays where it is. **A crude scan of the corpus SOURCE TEXT agrees,
/// and that instrument is exactly the one this package has already been wrong
/// with**: a regular expression over source is a model of the compiler, and the
/// fourth instrument error found on the sibling line was that shape. This reads
/// the module's own layout table instead — the kind tags the backend itself
/// dispatches on.
#[test]
fn the_float_shared_slot_route_has_no_corpus_witness() {
    let mut modules = 0usize;
    let mut with_float_slot: Vec<String> = Vec::new();
    let mut shared_slots = 0usize;

    for p in common::corpus_sources() {
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Some(m) = compiled(&src) else { continue };
        modules += 1;
        let Some(dl) = m.data_layout.as_ref() else {
            continue;
        };
        shared_slots += dl.shared_layout.len();
        if dl.shared_layout.iter().any(|e| e.kind == FLOAT_TAG) {
            with_float_slot.push(p.file_name().unwrap().to_string_lossy().to_string());
        }
    }

    println!("\n================ FLOAT SHARED SLOT, corpus population");
    println!("  corpus modules compiled  : {modules}");
    println!("  shared slots declared    : {shared_slots}");
    println!(
        "  modules with a FLOAT slot: {} {with_float_slot:?}",
        with_float_slot.len()
    );
    println!("================\n");

    // **NON-VACUITY.** A sweep that found no shared slots at all would report an
    // empty float population for the wrong reason, and would keep reporting it
    // if the layout table moved or the corpus roots broke.
    assert!(
        shared_slots > 0,
        "the sweep found no shared slots anywhere in {modules} modules, so its \
         empty float population says nothing about floats"
    );
    assert!(
        with_float_slot.is_empty(),
        "a corpus module now declares a FLOAT shared slot: {with_float_slot:?}. \
         The float slot route acquired a corpus witness it did not have when it \
         was built, so the censuses may legitimately move -- re-derive them \
         rather than reading a movement as a regression"
    );
}
