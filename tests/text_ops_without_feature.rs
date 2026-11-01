//! The bundled text operations are refused in a build without the `text`
//! cargo feature.
//!
//! The operations are methods resolved by receiver type, so the refusal is a
//! compile error that names the feature whenever one of the five methods is
//! called on a `Text<N>`. It holds even when the program would otherwise be a
//! valid use of the operations, and it reaches no other receiver. The
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

fn compile_ok(src: &str) -> keleusma::bytecode::Module {
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    compile(&program).unwrap_or_else(|e| panic!("compile failed for {src}: {}", e.message))
}

#[test]
fn every_operation_is_refused_naming_the_feature() {
    let mut reached = 0usize;
    for src in [
        "fn main() -> Word { (\"ab\" as Text<4>).len() }",
        "fn main() -> Word { (\"ab\" as Text<4>).capacity() }",
        "fn main() -> Text<8> { (\"ab\" as Text<4>).concat(\"c\" as Text<4>) }",
        "fn main() -> bool { (\"ab\" as Text<4>).eq(\"c\" as Text<4>) }",
        "fn main() -> Text<4> { (\"ab\" as Text<4>).slice(0, 1) }",
        "fn f(t: Text<4>) -> Word { t.len() }\nfn main() -> Word { 1 }",
    ] {
        let msg = compile_err(src);
        assert!(msg.contains("`text` cargo feature"), "{src}: {msg}");
        reached += 1;
    }
    assert_eq!(reached, 6);
}

#[test]
fn a_method_that_is_not_an_operation_is_not_refused_for_the_feature() {
    // Only the five operation names are gated. Any other method on a
    // `Text<N>` reaches the ordinary resolution, which finds no impl.
    let msg = compile_err("fn main() -> Word { (\"ab\" as Text<4>).length() }");
    assert!(msg.contains("no method `length`"), "{msg}");
    assert!(!msg.contains("cargo feature"), "{msg}");
}

#[test]
fn a_user_type_keeps_its_own_method_of_the_same_name() {
    // The gate is keyed on the receiver type, so a trait method named `len`
    // on a struct compiles without the feature.
    compile_ok(
        "struct S { a: Word }\n\
         trait Size { fn len(s: S) -> Word; }\n\
         impl Size for S { fn len(s: S) -> Word { s.a } }\n\
         fn main() -> Word { let s = S { a: 2 }; s.len() }",
    );
}

#[test]
fn the_text_path_is_an_ordinary_path() {
    // The free-function spelling was replaced by the methods, so nothing is
    // reserved under `text::`. Without a `use` a call there is an undefined
    // function, as any other undeclared path is, and a host native may be
    // imported under the path, in this build as in the other.
    let msg = compile_err("fn main() -> Word { text::len(\"ab\" as Text<4>) }");
    assert!(msg.contains("undefined function"), "{msg}");
    assert!(!msg.contains("cargo feature"), "{msg}");
    let module = compile_ok(
        "use text::upper\nfn greet(name: Text) -> Text { text::upper(name) }\n\
         fn main() -> Word { 1 }",
    );
    assert!(
        module.native_names.iter().any(|n| n == "text::upper"),
        "{:?}",
        module.native_names
    );
}

/// The self-hosted entry consults the reference before refusing a `Text<N>`
/// program, so in this build it reports the reference's refusal of the
/// operation rather than a subset limit.
#[cfg(feature = "self-host")]
#[test]
fn the_self_hosted_entry_reports_the_reference_refusal() {
    let src = "fn main() -> Word { (\"ab\" as Text<2>).len() }";
    match keleusma::selfhost::self_hosted_compile(src, &keleusma::target::Target::host()) {
        Err(keleusma::selfhost::SelfHostError::ReferenceRejected { .. }) => {}
        other => panic!("expected the reference refusal, got {other:?}"),
    }
}

#[test]
fn a_literal_cast_without_an_operation_still_compiles() {
    // The feature gates the operations, not the type.
    compile_ok("fn main() -> Text<4> { \"ab\" as Text<4> }");
}
