//! **WHERE A `Text` REACHES THIS BACKEND, AND WHERE IT STOPS.**
//!
//! # Why this file exists, and what it corrects
//!
//! The string surface was recorded in this line's own summary as *"likely refused"*
//! by the backend. **That was an assumption, and it is wrong.** Measured:
//!
//! | shape | result |
//! |---|---|
//! | a `Text` literal bound to a local | **lowers**, zero refusals |
//! | two `Text` locals | **lowers** |
//! | a `Text` RETURN | **lowers** |
//! | **a `Text` in a struct field** | **REFUSED** — an operand of unknown packed width |
//!
//! Two other guesses were wrong on the way, and both are worth naming because each
//! would have gone into a record: the type is **`Text`**, not `Str` — *"function
//! `main` returns Str but body produces Text"* — and `length` is a **host native**,
//! not a builtin, so a bare call is *"undefined function `length`"*.
//!
//! # ⚠ A `Text` RETURN LOWERS AND NOTHING COMPARES IT, WITH A STATED REASON
//!
//! This file does NOT drive a `Text` return, and the reason is not oversight. The
//! reference returns a `Value::StaticStr` carrying a `String`; the lowered function
//! returns an `i64` handle. **Comparing them is the "arena handle against a
//! pointer" problem** that `corpus_differential` records as having cost it a whole
//! module's comparison once.
//!
//! So the gap is recorded rather than closed, and it is a real one: **a lowered
//! `Text` return has no differential.** Closing it needs handle marshalling, which
//! is the same machinery the fifteen undriven streaming corpus modules need.
//!
//! # The refused case is the THIRD instance of one family
//!
//! `NewComposite` refusing an operand of unknown packed width has now appeared
//! three times on this line:
//!
//! | instance | disposition |
//! |---|---|
//! | the resumed float reply | **CLOSED** — one site, using the module's own `float_bytes` |
//! | a `Multiword` multiply | **DECLINED** — traced to the multi-write local rule, which is deliberate |
//! | **a `Text` struct field** | **pinned here** — at op 2, so it is the TAG-level case, like `Float` was |
//!
//! **SETTLED BY READING, 2026-09-23** — see `TEXT_FIELD_VERDICT.md`. The width IS
//! knowable: `value_layout.rs` sizes a `Text` at `2 * word_bytes`, a handle of
//! pointer-or-offset plus length-or-epoch with a discriminant. **And it still must
//! not be supplied**, because the emitter lowers a string constant as a BARE
//! ADDRESS — one word, no discriminant — and says so where it withholds the width:
//! *"packing an address into a composite body as though it were a scalar is exactly
//! the mistake `Width::Unknown` exists to make impossible."*
//!
//! So the refusal is correct and deliberate, and closing it is the string
//! representation workstream rather than a width lookup. **A shared error message is
//! not a shared cause**: three instances of this refusal had three different roots,
//! and only the float one was a width that was merely missing.

mod common;

/// **A `Text` LOWERS IN A LOCAL AND AS A RETURN.**
///
/// Pinned because the opposite was assumed in this line's own records.
#[test]
fn a_text_lowers_in_a_local_and_as_a_return() {
    for (what, src) in [
        (
            "a literal bound to a local",
            "fn main(a: Word) -> Word { let s = \"hi\"; let _t = s; 1 }",
        ),
        (
            "two literals in two locals",
            "fn main(a: Word) -> Word { let s = \"hi\"; let t = \"yo\"; \
             let _u = s; let _v = t; a }",
        ),
        ("a Text return", "fn main(a: Word) -> Text { \"hi\" }"),
    ] {
        let m = common::build(src);
        let refusals =
            keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
        assert!(
            refusals.is_empty(),
            "{what} is now refused: {refusals:?}. If that is deliberate it belongs \
             beside the struct-field refusal with its cause, not as a silent \
             narrowing."
        );
    }
}

/// **A `Text` IN A STRUCT FIELD IS REFUSED, WITH ITS CAUSE AND A CONTROL.**
///
/// `NewComposite` receives an operand whose packed width it cannot establish. The
/// refusal is sound for the reason the whole family is sound: it fails closed
/// rather than packing at a guessed size, so the cost is capability and never a
/// mispack.
///
/// **The control is what stops this overclaiming.** The same struct with a `Word`
/// in place of the `Text` LOWERS, so the refusal is about the string field rather
/// than about composites or about this program's shape.
#[test]
fn a_text_struct_field_is_refused_and_a_word_field_is_not() {
    let with_text = "struct P { s: Text, n: Word }\n\
                     fn main(a: Word) -> Word { let p = P { s: \"hi\", n: a }; p.n }";
    let refusals = keleusma_native::module_refusals(
        &common::build(with_text),
        keleusma_native::LowerOptions::default(),
    );
    let Some((_, e)) = refusals.first() else {
        panic!(
            "a Text struct field now LOWERS. That is the gap closing — but a \
             string's packed width must then come from the canonical layout rather \
             than a guess, and this test must be replaced by one that checks the \
             PACKING, not by deleting it."
        );
    };
    let msg = format!("{e:?}");
    assert!(
        msg.contains("NewComposite") && msg.contains("unknown packed width"),
        "the Text struct field is refused for a different reason now: {msg}. The \
         recorded cause is an operand whose packed width is unknown."
    );

    // **THE CONTROL.** Without it this test would support "the backend cannot build
    // this struct", which is false.
    let all_words = "struct Q { s: Word, n: Word }\n\
                     fn main(a: Word) -> Word { let q = Q { s: 1, n: a }; q.n }";
    assert!(
        keleusma_native::module_refusals(
            &common::build(all_words),
            keleusma_native::LowerOptions::default(),
        )
        .is_empty(),
        "the all-Word control is refused too, so the refusal above says nothing \
         specific about a Text field"
    );
}

/// **THE TWO SYNTAX GUESSES THAT WERE WRONG, PINNED SO THE RECORD CANNOT DRIFT.**
///
/// A record naming a type or a function that does not exist is worse than none,
/// because a later reader takes it for the language. Both of these went into a
/// draft before being measured.
#[test]
fn the_string_type_is_text_and_length_is_a_host_native() {
    // `Str` is not the type a literal produces.
    assert!(
        common::try_build("fn main(a: Word) -> Str { \"hi\" }").is_none(),
        "`Str` is now a type a string literal satisfies. The header of this file \
         says the type is `Text`, and that claim must be corrected."
    );
    // `length` is not in scope without a host declaration.
    assert!(
        common::try_build("fn main(a: Word) -> Word { let s = \"hi\"; length(s) }").is_none(),
        "`length` is now a builtin. This file records it as a host native, and that \
         claim must be corrected."
    );
    // **NON-VACUITY.** The shape these two differ from must still compile, or the
    // assertions above would pass for an unrelated reason.
    assert!(
        common::try_build("fn main(a: Word) -> Text { \"hi\" }").is_some(),
        "a Text return no longer compiles, so the two refusals above are not \
         isolating the type name and the function name at all"
    );
}
