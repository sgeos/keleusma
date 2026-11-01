//! Measure emitter independence after the authoritative expression recording pass.
//! The intervention does not disable admission or type checking.

extern crate std;

use super::*;
use crate::{lexer::tokenize, parser::parse};
use alloc::borrow::ToOwned;
use alloc::collections::BTreeSet;

const STAGES: &[(&str, &str)] = &[
    ("lexer", include_str!("../selfhost/kel/lexer.kel")),
    ("parse", include_str!("../selfhost/kel/parse.kel")),
    (
        "template_scan",
        include_str!("../selfhost/kel/template_scan.kel"),
    ),
    (
        "reconstruct",
        include_str!("../selfhost/kel/reconstruct.kel"),
    ),
    ("codegen", include_str!("../selfhost/kel/codegen.kel")),
    ("analyze", include_str!("../selfhost/kel/analyze.kel")),
    (
        "verify_structural",
        include_str!("../selfhost/kel/verify_structural.kel"),
    ),
    (
        "verify_yield",
        include_str!("../selfhost/kel/verify_yield.kel"),
    ),
    (
        "verify_depth",
        include_str!("../selfhost/kel/verify_depth.kel"),
    ),
    (
        "verify_typed",
        include_str!("../selfhost/kel/verify_typed.kel"),
    ),
    (
        "verify_datalayout",
        include_str!("../selfhost/kel/verify_datalayout.kel"),
    ),
    (
        "verify_types",
        include_str!("../selfhost/kel/verify_types.kel"),
    ),
    ("wire", include_str!("../selfhost/kel/wire.kel")),
    ("wire_debug", include_str!("../selfhost/kel/wire_debug.kel")),
    (
        "wire_schema",
        include_str!("../selfhost/kel/wire_schema.kel"),
    ),
];

#[test]
fn inference_corpus_covers_every_stage_source() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/selfhost/kel");
    let actual: BTreeSet<_> = std::fs::read_dir(directory)
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
    let listed: BTreeSet<_> = STAGES.iter().map(|(name, _)| (*name).to_owned()).collect();
    assert!(!actual.is_empty());
    assert_eq!(listed.len(), STAGES.len(), "duplicate stage entries");
    assert_eq!(listed, actual);
}

#[test]
fn all_stage_artifacts_are_independent_of_recorded_expression_types() {
    let target = crate::target::Target::host();
    let options = CompileOptions::default();
    let mut distinct = BTreeSet::new();
    let mut total_entries = 0;
    for &(name, source) in STAGES {
        let program = parse(&tokenize(source).expect("lex stage")).expect("parse stage");
        assert!(
            program.fn_expr_types.is_empty(),
            "parsed input must start unrecorded"
        );
        let reference = compile_with_options(&program, &target, &options)
            .expect("reference compile")
            .0;
        let mut calls = 0;
        let mut erased = 0;
        let actual = compile_with_recorded_type_hook(&program, &target, &options, |table| {
            calls += 1;
            erased = table.values().map(BTreeMap::len).sum::<usize>();
            assert!(erased > 0, "{name} has no recorded types to erase");
            table.clear();
            assert!(table.is_empty());
        })
        .unwrap_or_else(|error| panic!("{name} without recorded types: {error:?}"))
        .0;
        assert_eq!(calls, 1, "{name} intervention must run exactly once");
        let expected = reference.to_bytes().expect("reference serialization");
        let bytes = actual.to_bytes().expect("fallback serialization");
        assert_eq!(
            bytes, expected,
            "{name} depends on recorded expression types"
        );
        assert!(!bytes.is_empty());
        std::eprintln!(
            "{name}: erased {erased} recorded types, {} artifact bytes",
            bytes.len()
        );
        total_entries += erased;
        distinct.insert(bytes);
    }
    assert_eq!(
        distinct.len(),
        STAGES.len(),
        "artifact comparison must discriminate stages"
    );
    assert!(total_entries > 0);
    std::eprintln!(
        "{} stages, {total_entries} erased expression types",
        STAGES.len()
    );
}

#[test]
fn altering_a_recorded_array_type_changes_emission() {
    let source = "fn main(xs: [Word; 2]) -> Word { xs[0] }";
    let program = parse(&tokenize(source).unwrap()).unwrap();
    let target = crate::target::Target::host();
    let options = CompileOptions::default();
    let reference = compile_with_options(&program, &target, &options).unwrap().0;
    let mut changed = 0;
    let poisoned = compile_with_recorded_type_hook(&program, &target, &options, |table| {
        for entries in table.values_mut() {
            for ty in entries.values_mut() {
                if let TypeExpr::Array(element, _, _) = ty {
                    **element = TypeExpr::Prim(PrimType::Byte, element.span());
                    changed += 1;
                }
            }
        }
    })
    .expect("controlled poisoned emission")
    .0;
    assert!(changed > 0, "control must alter a populated record");
    assert_ne!(
        reference.to_bytes().unwrap(),
        poisoned.to_bytes().unwrap(),
        "the intervention must reach a record the emitter actually consults"
    );
}

#[test]
fn erasing_recorded_types_does_not_bypass_admission() {
    let program = parse(&tokenize("fn main() -> Word { true }").unwrap()).unwrap();
    let mut reached = false;
    let result = compile_with_recorded_type_hook(
        &program,
        &crate::target::Target::host(),
        &CompileOptions::default(),
        |table| {
            reached = true;
            table.clear();
        },
    );
    assert!(result.is_err(), "ill-typed source must still fail");
    assert!(!reached, "admission must reject before the intervention");
}
