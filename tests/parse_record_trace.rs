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

/// Declaration-header names in the stream, with body spans skipped.
///
/// # Why this cannot just filter on the code
///
/// A body record shares codes 1..=3 — `Node::Literal` is kind 1 — so a filter that
/// ignored nesting would report literals as declarations. Body spans run from code
/// 16 to code 15, and a data block from 9 to 5.
///
/// **This mirrors part of the driver's state machine, which is a copy**, so it is
/// checked rather than trusted: every caller asserts the extracted count against
/// the source's own `fn` count. A copy that drifts fails there instead of quietly
/// reporting the wrong headers.
fn header_names(src: &str) -> Vec<String> {
    let (names, records) = keleusma::selfhost::parse_record_trace(src);
    let mut out = Vec::new();
    let mut in_body = false;
    let mut in_data = false;
    for &(code, val) in &records {
        if in_body {
            if code == 15 {
                in_body = false;
            }
            continue;
        }
        if in_data {
            if code == 5 {
                in_data = false;
            }
            continue;
        }
        match code {
            16 => in_body = true,
            9 => in_data = true,
            1..=3 => out.push(names.get(val as usize).cloned().unwrap_or_default()),
            _ => {}
        }
    }
    out
}

/// **THE MIS-NAME FOLLOWS THE TRAILING FIELD ACCESS — RE-PINNED AGAINST THE
/// STREAM.**
///
/// This rule was first measured through `chunk_names_from_pipeline`, which at the
/// time derived the chunk numbering by hand and so **inherited the defect**. That
/// function now delegates to `first_pass`, which computes the table correctly, and
/// the rule had to move to evidence that does not depend on a derivation of mine.
///
/// The record stream is that evidence: it is what `parse.kel` emits, independent
/// of anything the driver or a helper does with it.
///
/// | preceding function's body | the following header names |
/// |---|---|
/// | `for … { d.a = 3; }` then `d.a` | `a` |
/// | `for … { d.a = 3; }` then `d.b` | **`b`** |
/// | `for … { d.b = 3; }` then `d.a` | **`a`** |
/// | `for … { d.a = 3; }` then a literal | `z`, correct |
/// | the same body **without the loop** | `z`, correct |
///
/// **Row three rules out the alternative**: if the mis-name came from the ASSIGNED
/// field it would read `b`; it reads `a`.
#[test]
fn the_stream_misnames_after_a_loop_with_a_trailing_field_read() {
    let headers = |body: &str| -> Vec<String> {
        let src = format!(
            "private data d {{ a: Word, b: Word }}\n\
             fn y() -> Word {{ {body} }}\n\
             fn z() -> Word {{ 9 }}\n\
             fn main() -> Word {{ y() + z() }}\n"
        );
        // Every probe is a program the REFERENCE accepts, so a case that stopped
        // parsing fails loudly rather than measuring a syntax error.
        keleusma::compiler::compile(
            &keleusma::parser::parse(&keleusma::lexer::tokenize(&src).expect("lex"))
                .expect("parse"),
        )
        .expect("the reference must accept every probe");
        let h = header_names(&src);
        // The extraction mirrors the driver's nesting rule; this is what catches it
        // drifting. Three `fn` heads in, three headers out.
        assert_eq!(
            h.len(),
            3,
            "the header extraction found {} declarations in a three-function program, so \
             it has drifted from the driver's nesting rule and everything below is \
             measuring the extractor: {h:?}",
            h.len()
        );
        h
    };

    let trail_a = headers("for j in 0..8 { d.a = 3; } d.a");
    let trail_b = headers("for j in 0..8 { d.a = 3; } d.b");
    let assign_b_trail_a = headers("for j in 0..8 { d.b = 3; } d.a");
    let trail_literal = headers("for j in 0..8 { d.a = 3; } 7");
    let no_loop = headers("d.a = 3; d.a");

    assert!(
        trail_a.contains(&"a".to_string()) && !trail_a.contains(&"z".to_string()),
        "the trailing `d.a` case no longer mis-names; the defect has moved: {trail_a:?}"
    );
    assert!(
        trail_b.contains(&"b".to_string()) && !trail_b.contains(&"a".to_string()),
        "the mis-name did not follow the trailing field to `b`, so it is not the trailing \
         access that carries it and this diagnosis is wrong: {trail_b:?}"
    );
    assert!(
        assign_b_trail_a.contains(&"a".to_string()),
        "assigning `d.b` and trailing `d.a` gave {assign_b_trail_a:?}. The mis-name follows \
         the ASSIGNED field after all, which is the alternative this case rules out"
    );
    assert!(
        trail_literal.contains(&"z".to_string()),
        "a trailing LITERAL now mis-names too, so the trailing field access is not the \
         trigger: {trail_literal:?}"
    );
    assert!(
        no_loop.contains(&"z".to_string()),
        "the same body without the `for` loop now mis-names, so the loop is not required: \
         {no_loop:?}"
    );
}
