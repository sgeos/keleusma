//! Complete serialized artifacts from the production compiler, across the stage inventory.
//! This checks scaffold fields as well as chunk bodies. Serialization still uses the Rust
//! encoder on both sides; production Keleusma wire emission is a separate obligation.
#![cfg(all(
    feature = "self-host",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

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
];

#[test]
fn complete_artifact_corpus_covers_every_stage_source() {
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
    assert!(!actual.is_empty());
    assert_eq!(listed, actual);
}

#[test]
fn production_compiler_matches_every_complete_stage_artifact() {
    let mut differences = Vec::new();
    for &(name, source) in STAGES {
        let tokens = keleusma::lexer::tokenize(source).expect("reference lex");
        let ast = keleusma::parser::parse(&tokens).expect("reference parse");
        let reference = keleusma::compiler::compile(&ast).expect("reference compile");
        let actual = keleusma::selfhost::self_host_compile_scratch(source);
        let expected_bytes = reference.to_bytes().expect("reference artifact");
        let actual_bytes = actual.to_bytes().expect("pipeline artifact");
        assert!(!reference.chunks.is_empty(), "{name} has no chunks");
        if actual_bytes != expected_bytes {
            let first = actual_bytes
                .iter()
                .zip(&expected_bytes)
                .position(|(a, b)| a != b);
            differences.push((name, actual_bytes.len(), expected_bytes.len(), first));
        }
        eprintln!(
            "{name}: {} chunks, pipeline {} bytes, reference {} bytes",
            actual.chunks.len(),
            actual_bytes.len(),
            expected_bytes.len()
        );
    }
    assert!(
        differences.is_empty(),
        "complete artifact differences {differences:?}"
    );
}
