//! The bundled text operations are refused in a build without the `text`
//! cargo feature.
//!
//! The refusal is a compile error that names the feature, and it holds even
//! when the program would otherwise be a valid use of the operations. The
//! companion `tests/text_ops.rs` compiles only in a build with the feature.
//! Each build therefore runs exactly one of the two files. Continuous
//! integration covers both, because the workspace run enables the feature
//! through `keleusma-cli` and the single-package runs do not.

#![cfg(all(feature = "compile", not(feature = "text")))]

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;

fn compile_err(src: &str) -> String {
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    match compile(&program) {
        Ok(_) => panic!("expected a refusal, but this compiled: {src}"),
        Err(e) => e.message,
    }
}

#[test]
fn every_operation_is_refused_naming_the_feature() {
    let mut reached = 0usize;
    for src in [
        "fn main() -> Word { text::len(\"ab\" as Text<4>) }",
        "fn main() -> Word { text::capacity(\"ab\" as Text<4>) }",
        "fn main() -> Text<8> { text::concat(\"ab\" as Text<4>, \"c\" as Text<4>) }",
        "fn main() -> bool { text::eq(\"ab\" as Text<4>, \"c\" as Text<4>) }",
        "fn main() -> Text<4> { text::slice(\"ab\" as Text<4>, 0, 1) }",
        "fn main() -> Word { text::not_an_operation(1) }",
    ] {
        let msg = compile_err(src);
        assert!(msg.contains("`text` cargo feature"), "{src}: {msg}");
        reached += 1;
    }
    assert_eq!(reached, 6);
}

#[test]
fn the_namespace_is_reserved_without_the_feature_too() {
    // A host native cannot be imported under the path in either build, so the
    // same source never means a native in one build and an intrinsic in the
    // other.
    let msg = compile_err("use text::concat\nfn main() -> Word { 1 }");
    assert!(
        msg.contains("the `text` path names the bundled text operations"),
        "{msg}"
    );
}

/// The self-hosted entry consults the reference before refusing a `Text<N>`
/// program, so in this build it reports the reference's refusal of the
/// operation rather than a subset limit.
#[cfg(feature = "self-host")]
#[test]
fn the_self_hosted_entry_reports_the_reference_refusal() {
    let src = "fn main() -> Word { text::len(\"ab\" as Text<2>) }";
    match keleusma::selfhost::self_hosted_compile(src, &keleusma::target::Target::host()) {
        Err(keleusma::selfhost::SelfHostError::ReferenceRejected { .. }) => {}
        other => panic!("expected the reference refusal, got {other:?}"),
    }
}

#[test]
fn a_literal_cast_without_an_operation_still_compiles() {
    // The feature gates the operations, not the type.
    let program = parse(&tokenize("fn main() -> Text<4> { \"ab\" as Text<4> }").expect("lex"))
        .expect("parse");
    compile(&program).expect("the literal cast does not need the feature");
}
