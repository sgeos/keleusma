//! Reference-poisoning controls for the Order 1 independence claims.
//!
//! The completion audit's no-borrowing and pipeline-owned type-input rows rested on code
//! inspection plus output equality, and equality cannot distinguish an independent
//! construction from one that copies the reference. These controls intervene instead. The
//! user program carries a marker that makes the reference lexer panic and count, while the
//! unmarked stage programs the pipeline bootstraps through the reference stay available.
//! A production path that consulted the reference about the user program would therefore
//! change its result or record a hit.
//!
//! Subjects are `self_host_compile_scratch`, which assembles every module field, and
//! `type_check_source_via_kel`, which transports type inputs through
//! `try_call_type_inputs_from_pipeline`. The shipping `self_hosted_compile` is not a
//! subject, because it compiles the user program with the reference by design to
//! cross-check the result. Limits are stated in the helper below and in the Order 1 audit.

extern crate std;

use super::*;
use crate::lexer::reference_poison;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

/// Every stage source, checked against the directory by the inventory test below.
const STAGES: &[(&str, &str)] = &[
    ("lexer", include_str!("kel/lexer.kel")),
    ("parse", include_str!("kel/parse.kel")),
    ("reconstruct", include_str!("kel/reconstruct.kel")),
    ("codegen", include_str!("kel/codegen.kel")),
    ("analyze", include_str!("kel/analyze.kel")),
    (
        "verify_structural",
        include_str!("kel/verify_structural.kel"),
    ),
    ("verify_yield", include_str!("kel/verify_yield.kel")),
    ("template_scan", include_str!("kel/template_scan.kel")),
    ("verify_depth", include_str!("kel/verify_depth.kel")),
    ("verify_typed", include_str!("kel/verify_typed.kel")),
    (
        "verify_datalayout",
        include_str!("kel/verify_datalayout.kel"),
    ),
    ("verify_types", include_str!("kel/verify_types.kel")),
    ("wire", include_str!("kel/wire.kel")),
    ("wire_debug", include_str!("kel/wire_debug.kel")),
    ("wire_schema", include_str!("kel/wire_schema.kel")),
];

/// Embed `marker` as a trailing comment on every line that opens a function at column
/// zero, and once more on a final line, returning the text and the number of copies.
///
/// Spreading copies across function heads means a consultation of one function's text
/// also carries a marker. A fragment that begins inside a function and ends before the
/// next head would still evade the substring match. No existing line is reordered and no
/// line is inserted before existing text, so line numbers are unchanged.
fn mark(source: &str, marker: &str) -> (String, usize) {
    let mut out = String::with_capacity(source.len() + 64 * 1024);
    let mut copies = 0usize;
    for line in source.split_inclusive('\n') {
        let (text, newline) = match line.strip_suffix('\n') {
            Some(text) => (text, "\n"),
            None => (line, ""),
        };
        out.push_str(text);
        if text.starts_with("fn ") {
            out.push_str(" // ");
            out.push_str(marker);
            copies += 1;
        }
        out.push_str(newline);
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("// ");
    out.push_str(marker);
    out.push('\n');
    (out, copies + 1)
}

fn reference_module(source: &str) -> Module {
    compile_src(source)
}

#[test]
fn poison_corpus_covers_every_stage_source() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/selfhost/kel");
    let actual: BTreeSet<String> = std::fs::read_dir(directory)
        .expect("stage directory")
        .map(|entry| entry.expect("stage entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "kel"))
        .map(|path| {
            path.file_stem()
                .expect("stage name")
                .to_string_lossy()
                .into()
        })
        .collect();
    let listed: BTreeSet<String> = STAGES.iter().map(|(name, _)| (*name).into()).collect();
    assert_eq!(listed.len(), STAGES.len(), "duplicate stage entries");
    assert_eq!(listed, actual);
}

/// The poison must actually stop the reference, or every pass below is empty.
#[test]
fn poison_stops_the_reference_and_spares_unmarked_text() {
    const MARKER: &str = "reference-poison-control-a3f1";
    let (marked, copies) = mark("fn main() -> Word { 7 }", MARKER);
    assert_eq!(copies, 2);
    let armed = reference_poison::arm(MARKER);
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let poisoned = std::panic::catch_unwind(|| reference_module(&marked));
    std::panic::set_hook(prev);
    assert!(poisoned.is_err(), "the reference compiled poisoned text");
    assert_eq!(armed.hits(), 1);
    reference_module("fn main() -> Word { 7 }");
    assert_eq!(armed.hits(), 1, "unmarked text must not count");
    drop(armed);
    reference_module(&marked);
}

/// Marking must not change what either compiler produces, so the comparison below
/// isolates the reference intervention rather than a change of input.
#[test]
fn marking_preserves_the_reference_artifact_of_every_stage() {
    for &(name, source) in STAGES {
        let (marked, copies) = mark(source, "reference-poison-identity-5c20");
        assert!(copies > 1, "{name}: no function head was marked");
        assert!(
            marked.len() <= crate::selfhost_host::LEX_SOURCE_CAP,
            "{name}: marked source exceeds the lexer source capacity"
        );
        assert_eq!(
            reference_module(&marked)
                .to_bytes()
                .expect("marked artifact"),
            reference_module(source).to_bytes().expect("artifact"),
            "{name}"
        );
    }
}

/// Every module field of every stage is assembled while the reference cannot read the
/// user program, and the result still equals the unpoisoned reference artifact.
#[test]
fn scratch_assembly_matches_every_stage_with_the_reference_poisoned() {
    const MARKER: &str = "reference-poison-scratch-9e47";
    let mut reached = Vec::new();
    for &(name, source) in STAGES {
        let expected = reference_module(source);
        let (marked, _) = mark(source, MARKER);
        let armed = reference_poison::arm(MARKER);
        let actual = self_host_compile_scratch(&marked);
        assert_eq!(
            armed.hits(),
            0,
            "{name}: scratch assembly read the reference"
        );
        drop(armed);
        assert_eq!(
            artifact_compare::difference(&actual, &expected),
            None,
            "{name}"
        );
        assert_eq!(
            actual.to_bytes().expect("pipeline artifact"),
            expected.to_bytes().expect("reference artifact"),
            "{name}: complete artifact"
        );
        reached.push(name);
    }
    assert_eq!(reached.len(), STAGES.len());
}

/// Production type checking reaches the same verdicts while the reference cannot read
/// the user program, over every stage, the ill-typed corpus and its valid controls.
///
/// A poisoned consultation panics inside the transport, which `type_check_source_via_kel`
/// reports as `ExtractionPanic`. That is an error rather than either verdict, so a
/// rejection cannot be manufactured by the poison itself. The rule-form census in
/// tests/selfhost_typecheck.rs is not repeated here.
#[test]
fn production_type_verdicts_hold_with_the_reference_poisoned() {
    use super::type_corpus::{ILL_TYPED, WELL_TYPED};
    const MARKER: &str = "reference-poison-types-61bd";
    assert_eq!((ILL_TYPED.len(), WELL_TYPED.len()), (20, 7));
    let cases = STAGES
        .iter()
        .chain(WELL_TYPED)
        .map(|&(name, source)| (name, source, true))
        .chain(
            ILL_TYPED
                .iter()
                .map(|&(name, source)| (name, source, false)),
        );
    let mut reached = (0usize, 0usize);
    for (name, source, expected) in cases {
        let (marked, _) = mark(source, MARKER);
        let armed = reference_poison::arm(MARKER);
        let verdict = type_check_source_via_kel(&marked);
        assert_eq!(armed.hits(), 0, "{name}: type inputs read the reference");
        drop(armed);
        match verdict {
            Ok(actual) => assert_eq!(actual, expected, "{name}"),
            Err(error) => panic!("{name}: no verdict under poison: {error}"),
        }
        if expected {
            reached.0 += 1;
        } else {
            reached.1 += 1;
        }
    }
    assert_eq!(reached, (STAGES.len() + WELL_TYPED.len(), ILL_TYPED.len()));
}
