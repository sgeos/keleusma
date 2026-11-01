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

/// **THE REPRODUCTION COMPILES, AND IT NAMES EVERY DECLARATION CORRECTLY.**
///
/// This test has had three subjects, one per state of the defect. It first
/// pinned the SYMPTOMS — a header for `z` carrying the field name `a`, the
/// mis-name following a trailing field read, the body closing at the loop's
/// brace. Then, when `parse.kel` learned to refuse the bare `for` by name, it
/// pinned that the stream stopped at a named diagnostic instead of misnaming.
/// Now the form is supported, so it pins the thing all of that was in aid of:
/// **the reproduction parses, and the names are right.**
///
/// The control below is what keeps this from being satisfied by a parser that
/// names everything `y`.
#[test]
fn the_loop_reproduction_now_names_every_declaration_correctly() {
    let (names, records) = keleusma::selfhost::parse_record_trace(REPRO);
    let id_of = |n: &str| -> i64 { names.iter().position(|s| s == n).expect("interned") as i64 };
    let headers: Vec<i64> = records
        .iter()
        .filter(|(c, _, _)| (1..=3).contains(c))
        .map(|(_, v, _)| *v)
        .collect();
    // CONTAINMENT, matching the control below. The 1..=3 filter also admits data
    // and body records, so an exact sequence would be a claim about the filter
    // rather than about the names. The defect was that `z`'s header carried the
    // data field `a`; its presence here is what refutes that.
    assert!(
        records.len() > 20,
        "only {} records traced; the instrument is broken",
        records.len()
    );
    for n in ["y", "z", "main"] {
        assert!(
            headers.contains(&id_of(n)),
            "the bare-`for` reproduction's stream has no header for `{n}`. That \
             is the original mis-naming defect returning."
        );
    }
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
