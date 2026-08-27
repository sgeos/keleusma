//! **`wire.kel` SELF-COMPILES BYTE-IDENTICALLY.** The largest stage in the corpus, at 486
//! chunks, and the last one that was outside the byte-identity oracle.
//!
//! # This file used to say the opposite, and that is the point of keeping it
//!
//! It was created to pin "compiles, but is NOT byte-identical" in the failing direction, so
//! that closing the gap would break it rather than pass silently. **The gap closed and it
//! broke.** What remains here is the part the oracle in `tests/selfhost_codegen.rs` does not
//! cover: the specific causes that stood in the way, pinned so a regression names the one
//! that returned instead of presenting as a raw array index again.
//!
//! # Four causes, two of them first diagnosed wrongly
//!
//! | recorded cause | verdict |
//! |---|---|
//! | a capacity bound, read off the `1024` in an index message | **wrong** |
//! | the lexer having no hexadecimal or binary literal support | correct |
//! | a cap of 256 on the declaration count | **wrong** |
//! | a `Call` record whose chunk field overflowed at index 256 | correct |
//! | `forin_count` not reset between functions | correct |
//!
//! Both wrong readings took a number in a message for a cause. The nearest miss was the
//! third: 256 was the right number attached to the wrong quantity.

#![cfg(all(feature = "self-host", feature = "compile"))]

const WIRE: &str = include_str!("../src/selfhost/kel/wire.kel");

/// The bare `for` counter is reset per function, so a program may hold several.
///
/// **This is the defect that kept `wire.kel` out of the oracle last**, and it is pinned here
/// in a five-line program rather than only through the 4,730-line stage. `forin_count`
/// indexes a record as `7 * forin_count`; unreset, the SECOND function containing a bare
/// `for` emitted a record pointing past its own parts.
///
/// Three loops, in three functions, because two was the minimum that reproduced and three
/// confirms it is every one after the first rather than the second alone.
#[test]
fn several_functions_may_each_contain_a_bare_for() {
    const SRC: &str = "data w { b: [Byte; 64] }\n\
        fn g() -> Word { for j in 0..8 { w.b[j] = w.b[j]; } 0 }\n\
        fn h() -> Word { for k in 0..4 { w.b[k] = w.b[k]; } 0 }\n\
        fn f() -> Word { for i in 0..16 { w.b[16 + i] = w.b[i]; } 0 }\n\
        fn main() -> Word { g() + h() + f() }\n";

    let reference = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(SRC).expect("lex")).expect("parse"),
    )
    .expect("reference compile");
    let mine = keleusma::selfhost::self_host_compile(SRC);

    // Per chunk, so a failure names WHICH loop lost its parts rather than only that the
    // bytes differ.
    for (m, r) in mine.chunks.iter().zip(reference.chunks.iter()) {
        assert_eq!(
            m.ops.len(),
            r.ops.len(),
            "chunk `{}` emits {} operations against the reference's {}. Fewer operations \
             means a construct was DROPPED: the bare `for` counter is running across \
             functions again",
            r.name,
            m.ops.len(),
            r.ops.len()
        );
    }
    assert_eq!(
        keleusma::wire_format::module_to_wire_bytes(&mine).expect("mine"),
        keleusma::wire_format::module_to_wire_bytes(&reference).expect("reference"),
    );
}

/// The counter's reset sits beside the analogue it was omitted from.
///
/// **A behavioural guard alone would keep passing if the reset were restored some other way**,
/// leaving the explanation above quietly false. This ties the pin to the repair it describes.
#[test]
fn the_bare_for_counter_is_reset_beside_its_analogue() {
    const STAGE: &str = include_str!("../src/selfhost/kel/parse.kel");
    assert!(
        STAGE.contains("forst.forlimit_count = 0;"),
        "the `limit` form's counter is no longer reset per function"
    );
    assert!(
        STAGE.contains("forst.forin_count = 0;"),
        "the bare form's counter is no longer reset per function. It is the analogue of \
         `forlimit_count` and was missing from this reset for exactly that reason once"
    );
}

/// `wire.kel` compiles, and the chunk count is what three sessions of prose have quoted.
///
/// Byte-identity itself is owned by `self_host_compiles_wire_kel_byte_identically` in
/// `tests/selfhost_codegen.rs`, alongside the other ten stages. This asserts only the figure.
#[test]
fn wire_kel_has_the_chunk_count_the_documents_quote() {
    let module = keleusma::selfhost::self_host_compile(WIRE);
    assert_eq!(
        module.chunks.len(),
        486,
        "`wire.kel`'s chunk count moved; the figure quoted across the channels is stale"
    );
}
