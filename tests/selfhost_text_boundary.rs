//! Text signatures and boundary flags must retain established source admission.
#![cfg(feature = "self-host")]

#[test]
fn text_boundary_flags_and_signatures_match_reference() {
    let mut gaps = Vec::new();
    for source in [
        "fn main() -> Text { \"x\" }",
        "fn main(x: Text) -> Text { x }",
        "fn main(x: Text) -> Word { 7 }",
        "fn main(x: Text) -> Word { if x == \"x\" { 7 } else { 8 } }",
        "fn main(x: Text) -> Word { let x = 7; x }",
        "fn f() -> Text { \"x\" } fn main() -> Text { f() }",
        "fn f(x: Text) -> Text { x } fn main() -> Text { f(\"x\") }",
        "fn f() -> Text { \"x\" } fn main() -> Word { 7 }",
        "fn main(x: Text) -> Word when x == \"x\" { 7 } fn main(x: Text) -> Word { 8 }",
        "fn main(xs: [Text; 2]) -> Word { 7 }",
        "yield main() -> Text { yield \"x\"; \"x\" }",
        "fn main(x: Fixed<16>) -> Fixed<16> { x }",
    ] {
        let reference = keleusma::selfhost::compile_src(source);
        let actual = match keleusma::selfhost::self_hosted_compile(
            source,
            &keleusma::target::Target::host(),
        ) {
            Ok(module) => module,
            Err(error) => {
                eprintln!("{source}: {error:?}");
                gaps.push(source);
                continue;
            }
        };
        assert_eq!(actual.flags, reference.flags, "{source}");
        assert_eq!(
            keleusma::selfhost::wire_module_artifact_via_kel(&actual).unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
    }
    assert!(gaps.is_empty(), "text boundary gaps {gaps:?}");
}

#[test]
fn wildcard_parameters_remain_explicitly_unsupported() {
    use keleusma::selfhost::{SelfHostError, TypeCheckError};
    use keleusma::selfhost_host::{PARSE_DIAG_TAG_BASE, ParseRecordError};
    let source = "fn main(_: Text) -> Word { 7 }";
    keleusma::selfhost::compile_src(source);
    assert!(
        matches!(keleusma::selfhost::type_check_source_via_kel(source), Err(TypeCheckError::Parse(ParseRecordError::Diagnostic { code, .. })) if code == PARSE_DIAG_TAG_BASE - 20)
    );
    assert!(matches!(
        keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()),
        Err(SelfHostError::Unsupported { .. })
    ));
    keleusma::selfhost::self_hosted_compile(
        "fn main(x: Text) -> Word { 7 }",
        &keleusma::target::Target::host(),
    )
    .unwrap();
}

#[test]
fn unresolved_private_initializers_cannot_escape_the_shipping_comparison() {
    let source = "private data d { n: Word = 5 } fn main() -> Word { d.n = d.n + 1; d.n }";
    let reference = keleusma::selfhost::compile_src(source);
    let pipeline = keleusma::selfhost::self_host_compile_scratch(source);
    assert_eq!(pipeline.chunks[0].ops, reference.chunks[0].ops);
    assert_ne!(
        pipeline.data_layout.as_ref().unwrap().private_init,
        reference.data_layout.as_ref().unwrap().private_init
    );
    assert!(
        matches!(keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()), Err(keleusma::selfhost::SelfHostError::Unsupported { detail }) if detail.contains("private initializers"))
    );
    let control = "private data d { n: Word } fn main() -> Word { d.n = d.n + 1; d.n }";
    keleusma::selfhost::self_hosted_compile(control, &keleusma::target::Target::host()).unwrap();
}
