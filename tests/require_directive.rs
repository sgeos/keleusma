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

/// A 32-bit-word target whose float follows THIS BUILD rather than being pinned to f64.
///
/// `word32_target()` declares a 64-bit float. This suite is about the `require word` directive, so
/// the float width is incidental -- but under `narrow-float-32` the compiler refuses any target
/// wider than the runtime, and these tests failed on that with assertion messages about the WORD
/// width, which described what was being asserted rather than why it failed.
///
/// Deriving the float here is the right repair precisely BECAUSE the float is not the subject.
/// Where a float width is the subject, deriving it would make the test vacuous instead; see
/// `wider_float_bytecode_never_reaches_execution` in `tests/narrow_vm.rs`.
fn word32_target() -> Target {
    Target {
        word_bits_log2: 5,
        addr_bits_log2: 5,
        float_bits_log2: keleusma::bytecode::RUNTIME_FLOAT_BITS_LOG2,
        has_floats: true,
        has_strings: true,
    }
}

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
        compile_for(src, word32_target()).is_ok(),
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
        compile_for(src, word32_target()).is_ok(),
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
