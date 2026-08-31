//! A census of reference-versus-self-hosted divergence over the LEXICAL surface.
//!
//! # Why this exists
//!
//! The byte-identity oracle compares eleven stage sources. It is a strong instrument over the
//! inputs it is given and says nothing whatever about inputs it is not given. On 2026-08-30 two
//! divergences were found in one afternoon, both in what a string literal's bytes ARE, and both
//! invisible for the same reason:
//!
//! **No `.kel` source in this repository contains a non-ASCII string literal, and none contains
//! any escape sequence at all.** The corpus was never chosen to exercise the lexer, and did not.
//!
//! One of the two had the reference compiler wrong and the self-hosted pipeline right. That
//! direction is worth holding onto: the reference is not the definition of correct, it is one of
//! two implementations, and the oracle is symmetric.
//!
//! # What this measures
//!
//! For each probe, the module the reference compiler produces and the module the SHIPPING
//! self-hosted driver produces are compared field by field. The shipping driver is the target
//! deliberately: `tests/selfhost_codegen.rs` classifies a COPY of the driver, and for as long as
//! only the copy was measured, four defects in the shipping one were invisible.
//!
//! # What a passing run does and does not claim
//!
//! It claims that no probe in the set produced DIFFERENT BYTES under the two pipelines. It does
//! not claim the lexical surface is exhausted, and it cannot: the probe set is a sample. What it
//! adds over the corpus is that the sample was chosen to contain what the corpus lacks.
//!
//! **A refusal is not an agreement.** A probe the self-hosted subset declines to compile has been
//! measured at nothing, and counting it as a pass is how a census comes to report agreement when
//! it means silence. Refusals are counted apart and reported.

#![cfg(all(feature = "compile", feature = "verify", feature = "self-host"))]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use std::panic::{AssertUnwindSafe, catch_unwind};

use keleusma::bytecode::Module;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Outcome {
    /// Both pipelines produced the same bytes. The only outcome that is evidence of agreement.
    Agrees,
    /// Both compiled and the bytes DIFFER. A silent miscompile; this is what the census hunts.
    Diverges,
    /// The self-hosted subset declined. An honest gap, and NOT evidence of anything else.
    SelfHostRefuses,
    /// The reference itself rejects the source, so there is nothing to compare.
    ReferenceRejects,
}

fn reference_compile(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

fn same_bytes(a: &Module, b: &Module) -> bool {
    a.chunks.len() == b.chunks.len()
        && a.chunks.iter().zip(b.chunks.iter()).all(|(x, y)| {
            x.name == y.name
                && x.ops == y.ops
                && x.constants == y.constants
                && x.local_count == y.local_count
        })
}

fn classify(src: &str) -> Outcome {
    if catch_unwind(AssertUnwindSafe(|| reference_compile(src))).is_err() {
        return Outcome::ReferenceRejects;
    }
    match catch_unwind(AssertUnwindSafe(|| {
        keleusma::selfhost::self_host_compile(src)
    })) {
        Err(_) => Outcome::SelfHostRefuses,
        Ok(theirs) => {
            let ours = reference_compile(src);
            if same_bytes(&theirs, &ours) {
                Outcome::Agrees
            } else {
                Outcome::Diverges
            }
        }
    }
}

/// The escapes the reference lexer ACCEPTS, computed by asking it rather than by restating a
/// list.
///
/// A restated set goes stale the first time an escape is added, silently, and the staleness looks
/// exactly like coverage. Every axis in this census that names a set derives it the same way.
fn accepted_escapes() -> Vec<u8> {
    (0u8..=127)
        .filter(|b| {
            tokenize(&format!(
                "fn f() -> Word {{ let s = \"a\\{}b\"; 1 }}",
                *b as char
            ))
            .is_ok()
        })
        .collect()
}

/// The probe set. Each entry is a label and a whole program.
///
/// The programs are minimal on purpose: the census is about the LEXER, so every probe should
/// differ from its neighbours only in lexical content. A probe carrying an interesting construct
/// as well would not say which of the two caused a divergence.
fn probes() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |label: String, src: String| out.push((label, src));

    // --- string literals: content the corpus does not contain -------------------------
    //
    // The corpus has no non-ASCII literal anywhere. This is the axis on which the reference
    // lexer was found re-encoding every byte at or above 0x80, so a six-byte literal baked as
    // eleven. These probes are the census-level regression pin for that.
    for (name, text) in [
        ("latin1_supplement", "é"),
        ("cjk", "漢字"),
        ("mixed_ascii_and_multibyte", "aé漢z"),
        ("emoji_four_byte", "🜁"),
        ("combining_mark", "e\u{0301}"),
    ] {
        push(
            format!("string/nonascii/{name}"),
            format!("fn f() -> Word {{ let s = \"{text}\"; 1 }}"),
        );
    }

    // Every escape the reference accepts, derived above. `\0` and `\r` were the two the
    // self-hosted unescape routine was missing, and no stage source uses ANY escape.
    for b in accepted_escapes() {
        push(
            format!("string/escape/{:02x}", b),
            format!("fn f() -> Word {{ let s = \"a\\{}b\"; 1 }}", b as char),
        );
    }

    // --- string literals: shape rather than content -----------------------------------
    for (name, body) in [
        ("empty", ""),
        ("only_escapes", "\\n\\t"),
        ("leading_escape", "\\nx"),
        ("trailing_escape", "x\\n"),
        ("space_only", " "),
        ("looks_like_a_comment", "// not a comment"),
        ("looks_like_code", "fn g() -> Word { 1 }"),
        ("brace_heavy", "{{}}"),
    ] {
        push(
            format!("string/shape/{name}"),
            format!("fn f() -> Word {{ let s = \"{body}\"; 1 }}"),
        );
    }

    // Interning: whether two identical literals share a pool entry is a choice each pipeline
    // makes, and the two must make it the same way or the constant pools differ.
    push(
        String::from("string/interning/duplicate_literals"),
        String::from("fn f() -> Word { let a = \"same\"; let b = \"same\"; 1 }"),
    );
    push(
        String::from("string/interning/distinct_literals"),
        String::from("fn f() -> Word { let a = \"one\"; let b = \"two\"; 1 }"),
    );
    push(
        String::from("string/interning/empty_and_nonempty"),
        String::from("fn f() -> Word { let a = \"\"; let b = \"x\"; 1 }"),
    );
    // A literal whose CONTENT collides with an identifier already in the intern table. The
    // self-hosted lexer reuses the identifier intern table for literal content, so this is the
    // shape where a shared table could confuse the two.
    push(
        String::from("string/interning/content_equals_an_identifier"),
        String::from("fn f() -> Word { let s = \"f\"; 1 }"),
    );

    // --- integer literals -------------------------------------------------------------
    //
    // Radix support was absent from the self-hosted lexer and unmeasured until 2026-08-26,
    // because the boundary table had no radix case. Same class as the string axis.
    for (name, expr) in [
        ("decimal_zero", "0"),
        ("decimal_one", "1"),
        ("decimal_large", "1234567890"),
        ("hex_lower", "0xff"),
        ("hex_upper", "0XFF"),
        ("hex_mixed_case_digits", "0xAbCd"),
        ("hex_zero", "0x0"),
        ("binary_lower", "0b1010"),
        ("binary_upper", "0B1010"),
        ("binary_zero", "0b0"),
        ("leading_zero_decimal", "007"),
    ] {
        push(
            format!("int/{name}"),
            format!("fn f() -> Word {{ {expr} }}"),
        );
    }

    // --- comments ---------------------------------------------------------------------
    //
    // A comment produces no token, so a divergence here is a divergence in what gets SKIPPED.
    // The two section signs in the tree's `.kel` files are both inside comments, which is the
    // only non-ASCII any stage source carries.
    for (name, src) in [
        ("leading", "// leading\nfn f() -> Word { 1 }"),
        (
            "trailing_with_newline",
            "fn f() -> Word { 1 }\n// trailing\n",
        ),
        (
            "trailing_without_newline",
            "fn f() -> Word { 1 }\n// trailing",
        ),
        (
            "between_items",
            "fn f() -> Word { 1 }\n// between\nfn g() -> Word { 2 }",
        ),
        ("inside_body", "fn f() -> Word {\n// inside\n1 }"),
        ("containing_a_quote", "// a \" quote\nfn f() -> Word { 1 }"),
        (
            "containing_a_backslash",
            "// a \\ backslash\nfn f() -> Word { 1 }",
        ),
        ("containing_nonascii", "// section §\nfn f() -> Word { 1 }"),
        (
            "containing_a_string_opener",
            "// \"unterminated\nfn f() -> Word { 1 }",
        ),
    ] {
        push(format!("comment/{name}"), String::from(src));
    }

    // --- whitespace and file shape ----------------------------------------------------
    for (name, src) in [
        ("no_trailing_newline", "fn f() -> Word { 1 }"),
        ("trailing_newline", "fn f() -> Word { 1 }\n"),
        ("many_trailing_newlines", "fn f() -> Word { 1 }\n\n\n"),
        ("leading_blank_lines", "\n\n fn f() -> Word { 1 }"),
        ("tab_indentation", "fn f() -> Word {\n\t1 }"),
        ("carriage_return_line_ending", "fn f() -> Word { 1 }\r\n"),
    ] {
        push(format!("whitespace/{name}"), String::from(src));
    }

    out
}

/// Sources whose outcome is known to be something OTHER than agreement, used to prove the
/// instrument can report a non-agreement at all.
///
/// # Why a clean census is not evidence until this passes
///
/// The census reports every probe agreeing. That is either a real result or a broken
/// classifier, and the two are indistinguishable from the census alone: a `classify` that
/// always returned `Agrees` would produce exactly the same output. **A passing check is
/// evidence about the checker's reach before it is evidence about the tree.**
///
/// The two controls are taken from the construct-support boundary in
/// `tests/selfhost_codegen.rs`, which records their outcomes independently, so this does not
/// invent an expectation. They are outside the lexical surface on purpose: a control drawn from
/// the axis under test would fail the moment the census found a real defect there.
fn positive_controls() -> &'static [(&'static str, Outcome, &'static str)] {
    &[
        // The self-hosted subset has no generics; the driver refuses rather than miscompiling.
        (
            "control/generic_fn",
            Outcome::SelfHostRefuses,
            "fn id<T>(x: T) -> T { x }\nfn f() -> Word { id(1) }",
        ),
        // Float arithmetic compiles on both sides and produces DIFFERENT bytes. This is the
        // control that matters most: it proves the census can see a silent miscompile, which is
        // the whole thing it exists to detect.
        (
            "control/float_arith",
            Outcome::Diverges,
            "fn f(a: Float, b: Float) -> Float { a + b }",
        ),
    ]
}

/// The census, its positive controls, and its coverage claim, in ONE test.
///
/// # Why this is not three tests
///
/// Silencing the panic hook around `catch_unwind` is process-global. Split across two tests in
/// one binary, the harness runs them concurrently and the census's no-op hook swallows the OTHER
/// test's failure message: the run reports `FAILED` with no reason given. That happened here, and
/// a test that fails without saying why is barely better than one that does not run.
#[test]
fn the_lexical_surface_shows_no_divergence_between_the_two_pipelines() {
    // --- the instrument must be able to report a non-agreement ------------------------
    //
    // Checked BEFORE the census so a broken classifier is reported as a broken classifier
    // rather than as a clean lexical surface.
    let prev_ctl = std::panic::take_hook();
    std::panic::set_hook(std::boxed::Box::new(|_| {}));
    let control_outcomes: Vec<(&str, Outcome, Outcome)> = positive_controls()
        .iter()
        .map(|(label, expected, src)| (*label, *expected, classify(src)))
        .collect();
    std::panic::set_hook(prev_ctl);
    for (label, expected, actual) in &control_outcomes {
        assert_eq!(
            actual, expected,
            "positive control `{label}` came back {actual:?} where the construct-support \
             boundary records {expected:?}. Until a control reports a non-agreement, a clean \
             census is evidence about this classifier and not about the lexer."
        );
    }
    assert!(
        control_outcomes
            .iter()
            .any(|(_, _, a)| *a == Outcome::Diverges),
        "no control produced Diverges, so nothing shows this census can detect a silent \
         miscompile at all"
    );

    let prev = std::panic::take_hook();
    std::panic::set_hook(std::boxed::Box::new(|_| {}));

    let mut agrees = 0usize;
    let mut refused: Vec<String> = Vec::new();
    let mut ref_rejects: Vec<String> = Vec::new();
    let mut diverged: Vec<String> = Vec::new();

    let all = probes();
    for (label, src) in &all {
        match classify(src) {
            Outcome::Agrees => agrees += 1,
            Outcome::Diverges => diverged.push(label.clone()),
            Outcome::SelfHostRefuses => refused.push(label.clone()),
            Outcome::ReferenceRejects => ref_rejects.push(label.clone()),
        }
    }

    std::panic::set_hook(prev);

    // THE CENSUS REPORTS ITS NUMBERS WHETHER OR NOT IT PASSES.
    //
    // A census that prints nothing on success is a census whose coverage nobody can read without
    // editing it. The breakdown is the result; "the test passed" is not. Visible under
    // `--nocapture`, and quoted in the failure messages below either way.
    println!(
        "lexical divergence census: {} probes -- {agrees} agree, {} diverge, {} refused by the \
         self-hosted subset, {} rejected by the reference",
        all.len(),
        diverged.len(),
        refused.len(),
        ref_rejects.len()
    );
    if !refused.is_empty() {
        println!("  refused (measured at nothing, NOT agreements): {refused:?}");
    }
    if !ref_rejects.is_empty() {
        println!("  rejected by the reference (nothing to compare): {ref_rejects:?}");
    }

    // NON-VACUITY, and it is the assertion that makes the rest mean anything. A probe set that
    // was entirely refused, or entirely rejected by the reference, would report zero divergences
    // while having compared NOTHING. The floor is deliberately well below the current count so
    // that adding probes does not require editing it, and well above zero so that a subset
    // regression that silently swallowed the set fails here.
    let compared = agrees + diverged.len();
    assert!(
        compared >= 30,
        "only {compared} of {} probes reached a byte comparison ({} refused by the self-hosted \
         subset, {} rejected by the reference). A census that compares nothing reports no \
         divergence for the wrong reason.",
        all.len(),
        refused.len(),
        ref_rejects.len()
    );

    // The census's actual claim.
    assert!(
        diverged.is_empty(),
        "the two pipelines produced DIFFERENT BYTES for {} probe(s): {:?}. Each is a silent \
         miscompile on the shipping path. Determine which side is wrong before changing this \
         test -- on the non-ASCII axis it was the REFERENCE that was wrong, not the self-hosted \
         pipeline.",
        diverged.len(),
        diverged
    );

    // --- coverage: the census must still contain what the corpus cannot -----------------
    //
    // The census's value is entirely in containing what the eleven stage sources do not. If a
    // future edit trimmed the probe set back to what the corpus already exercises, every
    // assertion above would still pass and the census would have become decorative.
    let labels: Vec<&str> = all.iter().map(|(l, _)| l.as_str()).collect();

    for axis in [
        "string/nonascii/",
        "string/escape/",
        "string/interning/",
        "int/",
        "comment/",
        "whitespace/",
    ] {
        assert!(
            labels.iter().any(|l| l.starts_with(axis)),
            "the probe set has no `{axis}` probes, so the census no longer covers that axis"
        );
    }

    // The blind spot, measured against the tree rather than believed.
    //
    // # THE CORPUS CONTAINS NO STRING LITERAL AT ALL
    //
    // This began as a check that no stage source uses an escape sequence. The measurement came
    // back stronger: **every double quote in all twelve stage sources is inside a line comment,
    // so the corpus has zero string literals.** Not "no escapes" -- nothing. Every property of
    // the string-literal path (escapes, non-ASCII content, interning, the empty literal, the
    // constant pool's string tag) is entirely unwitnessed by the byte-identity oracle.
    //
    // That is the justification for this whole file, and it is why the two defects found on
    // 2026-08-30 were not near-misses in otherwise-covered code. They were in a region the
    // oracle has never once exercised.
    //
    // # THE FIRST INSTRUMENT HERE WAS WRONG, AND IT IS THE FOURTH TIME ON THIS LINE
    //
    // It scanned the source text character by character, toggling an "inside a string" flag on
    // every double quote and counting backslashes while the flag was set. It reported ONE escape.
    // The line it found is a COMMENT in `lexer.kel` describing how the lexer treats backslashes,
    // and the quote characters inside that comment flipped the flag. A hand-written grep checked
    // against it reported zero, and was also wrong -- it searched for two literal backslashes.
    // **Two instruments disagreed and neither was right.**
    //
    // The check now tokenizes each stage with the real lexer, which does not emit tokens for
    // comments, and counts string-literal tokens. Exact, comment-immune, and derived from the
    // implementation rather than imitating it. **When the data is reachable through its real
    // reader, parsing its source text is choosing to have an instrument that can be wrong.**
    let stage_dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/selfhost/kel"));
    let mut stages = 0usize;
    let mut tokens_seen = 0usize;
    let mut literals: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(stage_dir).expect("the stage source directory") {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_some_and(|x| x == "kel") {
            stages += 1;
            let src = std::fs::read_to_string(&path).expect("read a stage source");
            let Ok(tokens) = tokenize(&src) else { continue };
            tokens_seen += tokens.len();
            for t in &tokens {
                if matches!(t.kind, keleusma::token::TokenKind::StringLit(_)) {
                    literals.push(format!(
                        "{}:{}",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        t.span.line
                    ));
                }
            }
        }
    }
    assert!(
        stages >= 10,
        "found {stages} stage sources, so this check has broken rather than the corpus having \
         shrunk"
    );
    // NON-VACUITY on the instrument, not on the result. The result being zero is the FINDING;
    // what must not be zero is the amount of source actually tokenized, because a lexer that
    // returned nothing would also report zero literals.
    assert!(
        tokens_seen > 1000,
        "only {tokens_seen} tokens were read from {stages} stage sources, so this check examined \
         essentially nothing and its zero-literal result means nothing"
    );
    assert!(
        literals.is_empty(),
        "a stage source now contains {} string literal(s), at {literals:?}. That is fine in \
         itself, but the corpus's TOTAL absence of string literals is what makes this census's \
         string axes measure something the byte-identity oracle cannot see. Revisit the \
         reasoning above rather than deleting this assertion.",
        literals.len()
    );
}
