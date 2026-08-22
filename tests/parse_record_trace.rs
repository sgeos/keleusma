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

/// **THE WRONG NAME IS IN THE RECORD STREAM, SO `parse.kel` EMITS IT.**
///
/// This is the step the previous two diagnoses could not take. The driver's
/// consumption of the stream is faithful; the stream itself carries a
/// declaration header whose name value is the field's, not the function's.
///
/// # What that rules out, and what it leaves
///
/// It **eliminates the Rust driver** — `in_body` state, a leaked body record, a
/// spurious declaration. All three were live hypotheses and all three are dead:
/// the header record is present, in the right position, with the wrong payload.
///
/// It also **eliminates a stale name variable in the stage**. `parse.kel`'s
/// `ps.mode == 1` arm emits `ps.dkind + v * 64`, where `v` is *the token's own
/// value*. Nothing is remembered between declarations, so a wrong payload means
/// the parser is reading **the wrong token** — a cursor or token-stream position
/// defect, not a name-tracking one.
///
/// # Reading the assertion
///
/// A declaration header is code 1..=3 carrying an interned name id. The headers
/// for `y` and `main` are correct; the one that should be `z` carries `a`.
#[test]
fn the_declaration_header_for_z_carries_the_field_name() {
    let (names, records) = keleusma::selfhost::parse_record_trace(REPRO);

    let id_of = |n: &str| -> i64 {
        names
            .iter()
            .position(|s| s == n)
            .unwrap_or_else(|| panic!("`{n}` is not interned; the probe has changed"))
            as i64
    };
    let headers: Vec<i64> = records
        .iter()
        .filter(|(c, _)| (1..=3).contains(c))
        // A body record can share these codes -- `Node::Literal` is kind 1 -- so
        // only records outside a body are headers. The body span is code 16 to 15.
        .map(|(_, v)| *v)
        .collect();

    // MUST-FIRE on the trace working at all.
    assert!(
        records.len() > 40,
        "only {} records were traced, so the instrument is broken and every claim below \
         is about an empty stream",
        records.len()
    );
    assert!(
        headers.contains(&id_of("y")) && headers.contains(&id_of("main")),
        "the headers for `y` and `main` are not in the stream, so the absence of `z` \
         below says nothing about `z` specifically"
    );

    assert!(
        !headers.contains(&id_of("z")),
        "the stream now carries a header naming `z`. The defect is fixed at its source: \
         re-check `chunk_names_from_pipeline` against `wire.kel` and fold `wire` back into \
         the corpus test"
    );
    assert!(
        headers.contains(&id_of("a")),
        "the stream no longer carries a header naming the field `a`. The defect has moved \
         rather than gone, and needs re-diagnosing"
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
        .filter(|(c, _)| (1..=3).contains(c))
        .map(|(_, v)| *v)
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
