//! What `lexer.kel` actually produces, and what that eliminates.
//!
//! # The third instrument
//!
//! `parse_cursor_trace` shows where the stage is reading and `parse_record_trace`
//! shows what it emits. Neither shows what it is *reading*, so a record carrying a
//! wrong value could be a parser fault or a token fault, and the two were not
//! separable from outside.
//!
//! That gap was not hypothetical. The declaration-mis-naming defect was narrowed
//! to "the header record carries the field's name", then to "the cursor never goes
//! backwards", and **stalled there** — because the remaining question was what
//! token sits at that position and nothing could answer it.

#![cfg(all(feature = "self-host", feature = "compile"))]

/// The four-line reproduction of the mis-naming defect.
///
/// **It uses the BARE `for` form, which `parse.kel` now refuses by name.** That
/// is the defect this reproduction was built to chase, and the refusal is its
/// resolution: the stage says which construct it cannot lower instead of
/// letting the failure surface five layers downstream as a missing chunk name.
///
/// So this source is still the right subject for the LEXER instrument below —
/// the token stream is what it always was — and it can no longer be fed to the
/// parse instruments, which is why they use `PARSEABLE`.
const REPRO: &str = "private data d { a: Word, b: Word }\n\
                     fn y() -> Word { for j in 0..8 { d.a = 3; } d.a }\n\
                     fn z() -> Word { 9 }\n\
                     fn main() -> Word { y() + z() }\n";

/// `REPRO` with the loop written in the counted form, which `parse.kel` accepts.
///
/// The sampling-rate property below is a property of the two INSTRUMENTS, not of
/// the bare loop, so it is measured on a source the parser will run. Everything
/// else about the two sources is identical, which is what keeps this a change of
/// subject rather than of question.
const PARSEABLE: &str = "private data d { a: Word, b: Word }\n\
                         fn y() -> Word { for j in 0..8 limit 8 { d.a = 3; } d.a }\n\
                         fn z() -> Word { 9 }\n\
                         fn main() -> Word { y() + z() }\n";

/// **THE TOKEN STREAM IS CORRECT, SO THE LEXER IS NOT AT FAULT.**
///
/// A fourth hypothesis, eliminated. Every declaration keyword in the reproduction
/// is followed by the right name token, `z` included — the one the parser then
/// mis-reports.
///
/// # Why this had to be checked rather than assumed
///
/// By elimination the defect had to be in the parser, the cursor, or the tokens,
/// and the first two were already ruled out. "It must be the tokens" would have
/// been a conclusion from exhaustion rather than measurement, and this line has
/// recorded enough of those to check the last box too.
#[test]
fn every_declaration_keyword_is_followed_by_its_own_name_token() {
    let (names, tokens) = keleusma::selfhost::lex_token_trace(REPRO);

    // `fn` is kind 0, `yield` 5, `loop` 6; an identifier is kind 1 and its value
    // is the interned id. Restated rather than imported, on the same ground as the
    // wire-format constants elsewhere in this tree: a test of a contract that
    // imports the contract cannot catch the contract changing.
    let mut heads: Vec<String> = Vec::new();
    for w in tokens.windows(2) {
        let (kw, _) = w[0];
        let (kind, val) = w[1];
        if matches!(kw, 0 | 5 | 6) && kind == 1 {
            heads.push(names.get(val as usize).cloned().unwrap_or_default());
        }
    }

    assert!(
        tokens.len() > 40,
        "only {} tokens were produced, so the instrument is broken and the assertion \
         below is about an empty stream",
        tokens.len()
    );
    assert_eq!(
        heads,
        vec!["y".to_string(), "z".to_string(), "main".to_string()],
        "the token stream does not carry the three declaration names in source order. If \
         `z` is missing here, the defect is in the lexer after all and every diagnosis \
         downstream of this needs revisiting"
    );
}

/// **THE CURSOR TRACE AND THE RECORD TRACE CANNOT BE ZIPPED, AND I TRIED.**
///
/// `parse_cursor_trace` samples once per virtual-machine step and
/// `parse_record_trace` once per emitted record — a difference of roughly an
/// order of magnitude. Pairing them by index correlates a record with an
/// unrelated cursor position, and the result *looks* like data — it produced a
/// neat table attributing `y`'s header to the token `{`.
///
/// **The assertion is the RATIO, and the counts are deliberately not stated.**
/// An earlier version quoted 1,232 against 78 for the bare-loop reproduction;
/// those figures were both unenforced and specific to a source this test no
/// longer uses. A number in a file of tests reads as a record, and a record
/// reads as already checked.
///
/// **A wrong answer in the shape of a right one is worse than no answer**, so the
/// mismatch is pinned rather than left as a trap for whoever reaches for the same
/// pairing. Correlating them properly needs the cursor sampled *per record*, which
/// the record sink does not yet carry.
///
/// Pinned in the firing direction: if the two ever come to the same length, that
/// is either a real change worth knowing about or a coincidence worth checking,
/// and both deserve a reader.
#[test]
fn the_two_traces_sample_at_different_rates_and_must_not_be_paired() {
    let (_, records) = keleusma::selfhost::parse_record_trace(PARSEABLE);
    let cursor = keleusma::selfhost::parse_cursor_trace(PARSEABLE);

    assert!(
        !records.is_empty() && !cursor.is_empty(),
        "one of the traces is empty, so this says nothing about their rates"
    );
    assert!(
        cursor.len() > records.len() * 4,
        "the cursor trace ({}) is no longer far longer than the record trace ({}). If the \
         sampling rates have converged, say so and revisit whether pairing them by index \
         is now meaningful -- it was not when this was written",
        cursor.len(),
        records.len()
    );
}
