// The `require` machine-property directive is checked against target widths, so
// it needs a runtime at least as wide as the targets it exercises. On a
// narrow-word runtime you cannot compile for a wider target at all, so this
// suite is gated to a 64-bit runtime.
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

use keleusma::compiler::compile_with_target;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::target::Target;

fn compile_for(src: &str, target: Target) -> Result<(), String> {
    let prog = parse(&tokenize(src).map_err(|e| format!("lex: {e:?}"))?).map_err(|e| e.message)?;
    compile_with_target(&prog, &target)
        .map(|_| ())
        .map_err(|e| e.message)
}

#[test]
fn require_word_at_least_accepts_wide_and_rejects_narrow() {
    let src = "require word >= 32;\nfn main() -> Word { 1 }";
    assert!(
        compile_for(src, Target::host()).is_ok(),
        "64-bit satisfies >= 32"
    );
    assert!(
        compile_for(src, Target::wasm32()).is_ok(),
        "32-bit satisfies >= 32"
    );
    let err = compile_for(src, Target::embedded_16()).unwrap_err();
    assert!(
        err.contains("word width") && err.contains("16"),
        "16-bit must be rejected: {err}"
    );
}

#[test]
fn require_word_exactly_pins_the_width() {
    let src = "require word == 32;\nfn main() -> Word { 1 }";
    assert!(
        compile_for(src, Target::wasm32()).is_ok(),
        "== 32 on a 32-bit target"
    );
    assert!(
        compile_for(src, Target::host()).is_err(),
        "== 32 rejects a 64-bit target"
    );
    assert!(
        compile_for(src, Target::embedded_16()).is_err(),
        "== 32 rejects 16-bit"
    );
}

#[test]
fn require_is_optional_and_composes_with_normal_items() {
    assert!(compile_for("fn main() -> Word { 1 }", Target::host()).is_ok());
    assert!(
        compile_for(
            "require word >= 16;\nstruct P { x: Word }\nfn main() -> Word { 1 }",
            Target::host()
        )
        .is_ok()
    );
}

#[test]
fn a_bad_require_is_a_parse_error() {
    assert!(compile_for("require word 32;\nfn main() -> Word { 1 }", Target::host()).is_err());
    assert!(compile_for("require gpu >= 4;\nfn main() -> Word { 1 }", Target::host()).is_err());
}
