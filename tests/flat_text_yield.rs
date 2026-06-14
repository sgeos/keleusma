#![cfg(all(feature = "compile", feature = "verify"))]
//! Flat-text composites may cross the yield boundary (B28 P3 item 4).
//!
//! A `Text` field of a struct or enum is flat. A static-string field points at
//! the immortal bytecode image (rodata); a dynamic field is an ephemeral arena
//! string. Under the read-before-resume contract the host decodes a yielded
//! composite before the next `resume()` (the RESET point), so the earlier
//! compile-time rejection of flat-text composite yields is lifted. These tests
//! assert that the yield now compiles; the host-decode round trip and the
//! survival of static (rodata) text across a RESET are pinned in
//! `flat_text_rodata_yield.rs`. A bare static string and a struct without text
//! were always allowed and remain so.

extern crate alloc;

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;

fn compiles(src: &str) -> bool {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    compile(&program).is_ok()
}

// The next five tests cover composites that carry a flat `Text` field. A flat
// `Text` holds a host data pointer, so it is flat-eligible only when the module
// word is at least the host pointer width (`value_layout.rs`:
// `word_bytes >= size_of::<usize>()`). A narrow-word build keeps `Text` boxed,
// where a yielded dynamic string is governed by the runtime `contains_dynstr`
// check; the flat-text path these tests exercise exists only on the full
// 64-bit word, so they are guarded off the narrow-word builds.
#[cfg(not(any(
    feature = "narrow-word-8",
    feature = "narrow-word-16",
    feature = "narrow-word-32"
)))]
#[test]
fn yielding_struct_with_text_field_compiles() {
    let src = "struct Greeting { msg: Text, n: Word }\n\
               loop main(seed: Word) -> Greeting { \
                   yield Greeting { msg: \"hi\", n: seed } \
               }";
    assert!(
        compiles(src),
        "yielding a struct with a flat Text field must now compile (read-before-resume)"
    );
}

#[cfg(not(any(
    feature = "narrow-word-8",
    feature = "narrow-word-16",
    feature = "narrow-word-32"
)))]
#[test]
fn yielding_enum_with_text_payload_compiles() {
    let src = "enum Msg { Quiet, Loud(Text) }\n\
               loop main(seed: Word) -> Msg { yield Msg::Loud(\"hi\") }";
    assert!(
        compiles(src),
        "yielding an enum with a flat Text payload must now compile"
    );
}

#[cfg(not(any(
    feature = "narrow-word-8",
    feature = "narrow-word-16",
    feature = "narrow-word-32"
)))]
#[test]
fn yielding_struct_holding_text_struct_compiles() {
    // A struct nesting another struct that carries Text: the inner struct's
    // Text is flat-nested into the outer body, and the outer yield now compiles
    // (transitive).
    let src = "struct Inner { t: Text }\n\
               struct Outer { inner: Inner, n: Word }\n\
               loop main(seed: Word) -> Outer { \
                   yield Outer { inner: Inner { t: \"hi\" }, n: seed } \
               }";
    assert!(
        compiles(src),
        "yielding a struct transitively containing a flat Text field must now compile"
    );
}

#[cfg(not(any(
    feature = "narrow-word-8",
    feature = "narrow-word-16",
    feature = "narrow-word-32"
)))]
#[test]
fn yielding_text_tuple_compiles() {
    // A tuple with a Text element flattens its text into the body (B28 P3 item
    // 5 C4); the yield now compiles, the same as a struct.
    let src = "loop main(seed: Word) -> (Word, Text) { yield (seed, \"hi\") }";
    assert!(
        compiles(src),
        "yielding a tuple with a flat Text element must now compile"
    );
}

#[cfg(not(any(
    feature = "narrow-word-8",
    feature = "narrow-word-16",
    feature = "narrow-word-32"
)))]
#[test]
fn yielding_text_array_compiles() {
    // An array of Text flattens likewise (B28 P3 item 5 C4).
    let src = "loop main(seed: Word) -> [Text; 2] { yield [\"a\", \"b\"] }";
    assert!(
        compiles(src),
        "yielding an array of flat Text must now compile"
    );
}

#[test]
fn yielding_bare_static_string_still_compiles() {
    // A bare static string is rodata, not a dynamic arena string, and is
    // free to cross the yield boundary.
    let src = "loop main(seed: Word) -> Text { yield \"hello\" }";
    assert!(
        compiles(src),
        "yielding a bare static string must remain allowed"
    );
}

#[test]
fn yielding_struct_without_text_still_compiles() {
    let src = "struct Point { x: Word, y: Word }\n\
               loop main(seed: Word) -> Point { yield Point { x: seed, y: 2 } }";
    assert!(
        compiles(src),
        "yielding a struct with no Text field must remain allowed"
    );
}
