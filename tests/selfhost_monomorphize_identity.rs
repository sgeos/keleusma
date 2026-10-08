//! Monomorphization is an identity transform on the self-hosted subset.
//!
//! This began as a one-off spike answering a roadmap question — whether the
//! Order-1 monomorphizer obligation is a real port of `src/monomorphize.rs` or
//! very nearly nothing. It is kept as a standing test because the answer is a
//! PROPERTY OF THE STAGE SOURCES, not of the monomorphizer, and it stops being
//! true the moment any stage declares a generic.
//!
//! When it fails, nothing is broken. It means the subset has grown a generic
//! and the self-hosted compiler now genuinely owes a monomorphizer. Read
//! `docs/decisions/TYPECHECK_SELFHOST_PLAN.md` before changing anything here.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::ast::Program;
use keleusma::lexer::tokenize;
use keleusma::monomorphize::monomorphize;
use keleusma::parser::parse;
use keleusma::typecheck::check;

const STAGES: &[(&str, &str)] = &[
    ("lexer", include_str!("../src/selfhost/kel/lexer.kel")),
    ("parse", include_str!("../src/selfhost/kel/parse.kel")),
    (
        "reconstruct",
        include_str!("../src/selfhost/kel/reconstruct.kel"),
    ),
    ("codegen", include_str!("../src/selfhost/kel/codegen.kel")),
    ("analyze", include_str!("../src/selfhost/kel/analyze.kel")),
    (
        "verify_structural",
        include_str!("../src/selfhost/kel/verify_structural.kel"),
    ),
    (
        "verify_yield",
        include_str!("../src/selfhost/kel/verify_yield.kel"),
    ),
    (
        "verify_depth",
        include_str!("../src/selfhost/kel/verify_depth.kel"),
    ),
    (
        "verify_typed",
        include_str!("../src/selfhost/kel/verify_typed.kel"),
    ),
    (
        "verify_datalayout",
        include_str!("../src/selfhost/kel/verify_datalayout.kel"),
    ),
    (
        "verify_types",
        include_str!("../src/selfhost/kel/verify_types.kel"),
    ),
    ("wire", include_str!("../src/selfhost/kel/wire.kel")),
    (
        "wire_debug",
        include_str!("../src/selfhost/kel/wire_debug.kel"),
    ),
];

/// The identity claim must cover every stage, including newly added sources.
#[test]
fn the_identity_corpus_contains_every_stage_source() {
    let actual: std::collections::BTreeSet<_> = std::fs::read_dir("src/selfhost/kel")
        .expect("stage directory")
        .map(|entry| entry.expect("stage entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "kel"))
        .map(|path| {
            path.file_stem()
                .expect("stage name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let listed: std::collections::BTreeSet<_> =
        STAGES.iter().map(|(name, _)| (*name).to_owned()).collect();
    assert!(!actual.is_empty(), "the stage inventory must not be empty");
    assert_eq!(listed, actual);
}

/// Parse and type-check, mirroring what `compile_with_options` does before it
/// calls the monomorphizer.
fn checked(src: &str) -> Program {
    let mut p = parse(&tokenize(src).expect("lex")).expect("parse");
    check(&mut p).expect("typecheck");
    p
}

#[test]
fn monomorphize_is_an_identity_on_every_stage_source() {
    let mut _identical = Vec::new();
    let mut changed = Vec::new();
    for (name, src) in STAGES {
        let before = checked(src);
        let after = monomorphize(before.clone());
        if after == before {
            _identical.push(*name);
        } else {
            changed.push(*name);
        }
    }
    assert_eq!(
        changed,
        Vec::<&str>::new(),
        "monomorphization changed these stages, so it is NOT an identity on the subset"
    );
}

#[test]
fn the_comparison_can_detect_a_change_that_does_occur() {
    // MUST-FIRE. Without this, "identity everywhere" is indistinguishable from
    // a comparison that always reports equal — for instance if `monomorphize`
    // were accidentally a no-op for every input, or `PartialEq` ignored the
    // fields that change.
    let src = "fn id<T>(x: T) -> T { x } fn main(a: Word) -> Word { id(a) }";
    let before = checked(src);
    let after = monomorphize(before.clone());
    assert_ne!(
        after, before,
        "the comparison did not detect monomorphization of a generic program, \
         so the identity result above proves nothing"
    );
}
