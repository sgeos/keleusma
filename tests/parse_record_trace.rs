//! The record stream `parse.kel` emits, made observable.
//!
//! # Why this instrument exists
//!
//! The declaration-mis-naming defect pinned by `tests/selfhost_chunk_names.rs`
//! was diagnosed three times without it, and each attempt stopped short of a
//! cause: first "a data block's field turns up in its place" (a pairing, not a
//! mechanism), then "the mis-name follows the trailing field access" (a rule, not
//! a site). What blocked the third step was that **the record stream the driver
//! consumes was not observable from outside it**.
//!
//! `thread_local!` is unavailable under `no_std`, so a hook cannot be smuggled in
//! from a test. `keleusma::selfhost::parse_record_trace` threads a sink through
//! `parse_functions_impl` instead. Every other caller passes a sink that
//! discards.
//!
//! **It is public rather than hidden on purpose.** A hidden instrument is one the
//! next person does not know exists, which is how this defect survived three
//! diagnoses.

#![cfg(all(feature = "self-host", feature = "compile"))]

/// The four-line reproduction, shared with `tests/selfhost_chunk_names.rs`.
const REPRO: &str = "private data d { a: Word, b: Word }\n\
                     fn y() -> Word { for j in 0..8 { d.a = 3; } d.a }\n\
                     fn z() -> Word { 9 }\n\
                     fn main() -> Word { y() + z() }\n";

/// **THE INVESTIGATION IS CLOSED: THE STREAM NO LONGER GETS FAR ENOUGH TO MISNAME.**
///
/// This test replaces three that pinned SYMPTOMS of the mis-naming defect — a
/// declaration header for `z` carrying the field name `a`, the mis-name
/// following a trailing field read, and the function body closing at the loop's
/// brace. All three were true, all three were consequences of one cause, and
/// **the cause is now refused at the point where it is known**: `parse.kel`
/// phase 4 of the loop header sees `{` where the counted form's `limit` would
/// be, and reports an unsupported-construct diagnostic.
///
/// The three symptom pins are gone rather than repointed. Each asserted a
/// specific wrong record in a stream that is no longer produced, so repointing
/// them at a source the parser accepts would have kept the names and changed the
/// subjects — which is how a test comes to measure something other than what it
/// says.
///
/// # What this asserts instead, and why it is the right successor
///
/// That the stream stops at the diagnostic, with no declaration header emitted
/// carrying a wrong name. The control below is what keeps this from being
/// satisfied by a parser that refuses everything.
#[test]
fn the_loop_reproduction_stops_at_a_named_diagnostic_rather_than_misnaming() {
    let err = keleusma::selfhost::try_parse_functions(REPRO)
        .expect_err("the reproduction contains a bare `for` and must now be refused");
    let message = err.to_string();
    assert!(
        message.contains("bare `for") && message.contains("not implemented"),
        "the reproduction is refused, but not for the construct that causes it: {message}"
    );
    assert!(
        !message.contains("chunk named"),
        "the refusal is still the downstream symptom the three retired tests pinned: \
         {message}"
    );
}

/// **THE CONTROL: THE SAME PROGRAM WITHOUT THE LOOP HAS A CORRECT STREAM.**
///
/// Without this, "the record stream carries a wrong name" would be a claim about
/// the instrument as easily as about the defect.
#[test]
fn the_same_program_without_the_loop_names_every_declaration_correctly() {
    const CLEAN: &str = "private data d { a: Word, b: Word }\n\
                         fn y() -> Word { d.a = 3; d.a }\n\
                         fn z() -> Word { 9 }\n\
                         fn main() -> Word { y() + z() }\n";
    let (names, records) = keleusma::selfhost::parse_record_trace(CLEAN);
    let id_of = |n: &str| -> i64 { names.iter().position(|s| s == n).expect("interned") as i64 };
    let headers: Vec<i64> = records
        .iter()
        .filter(|(c, _, _)| (1..=3).contains(c))
        .map(|(_, v, _)| *v)
        .collect();

    assert!(
        records.len() > 20,
        "the control traced only {} records; the instrument is broken",
        records.len()
    );
    for n in ["y", "z", "main"] {
        assert!(
            headers.contains(&id_of(n)),
            "the control's stream is missing a header for `{n}`, so the `for` loop is not \
             what distinguishes the two cases"
        );
    }
}

// `header_names`, a helper that mirrored the driver's declaration state machine,
// was deleted with the three symptom tests it served. **It was a COPY of driver
// state**, and its own documentation said so and required every caller to check
// the extracted count against the source. With no callers there is nothing to
// check it, and an unchecked copy of a state machine is worse than no copy.
