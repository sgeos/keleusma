//! A native's declarations agree with each other and with its bundled library.
//!
//! # The defect
//!
//! A native's declared result type decides the layout its result is decoded
//! with. Measured on 2026-10-30, `use shell::run(Text) -> (Text, Word)`
//! compiled although the Shell library declares `(Text) -> (Word, Text)`, and
//! so did two contradictory declarations of one native. The documented remedy,
//! prepending `Shell::SIGNATURES` to the script, therefore caught nothing. A
//! pykeleusma finding, which read a Word as a text pointer by this route.
//!
//! # What these tests do NOT establish
//!
//! A host's own natives have no library declaration to compare against, so a
//! wrong declaration of one is still the host's contract to keep. A bare `use`
//! without a signature is not compared.
#![cfg(all(feature = "compile", feature = "verify"))]

fn compiled(source: &str) -> Result<(), String> {
    let tokens = keleusma::lexer::tokenize(source).map_err(|e| format!("{e:?}"))?;
    let program = keleusma::parser::parse(&tokens).map_err(|e| format!("{e:?}"))?;
    keleusma::compiler::compile(&program)
        .map(|_| ())
        .map_err(|e| e.message)
}

#[test]
fn two_declarations_of_one_native_must_agree() {
    let conflicting = "use host::f(Word) -> (Word, Word)\nuse host::f(Word) -> Word\nfn main() -> Word { host::f(1) }";
    let message = compiled(conflicting).expect_err(conflicting);
    assert!(
        message.contains("declared twice with different signatures"),
        "{message}"
    );
    let identical =
        "use host::f(Word) -> Word\nuse host::f(Word) -> Word\nfn main() -> Word { host::f(1) }";
    compiled(identical).unwrap_or_else(|e| panic!("{identical}: {e}"));
}

#[cfg(feature = "shell")]
#[test]
fn a_bundled_native_must_be_declared_as_its_library_declares_it() {
    let swapped = "use shell::run(Text) -> (Text, Word)\nfn main() -> (Text, Word) { shell::run(\"echo hi\") }";
    let message = compiled(swapped).expect_err(swapped);
    assert!(
        message.contains("differs from its bundled library"),
        "{message}"
    );
    let matching = "use shell::run(Text) -> (Word, Text)\nfn main() -> (Word, Text) { shell::run(\"echo hi\") }";
    compiled(matching).unwrap_or_else(|e| panic!("{matching}: {e}"));
    // The documented remedy now works, the library's text prepended to a script
    // that declares the native differently.
    let prepended = format!("{}{swapped}", keleusma::stddsl::Shell::SIGNATURES);
    assert!(compiled(&prepended).is_err());
}
