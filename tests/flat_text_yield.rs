#![cfg(all(feature = "compile", feature = "verify"))]
//! The cross-yield prohibition extended to nested strings (B28 P3 item 2).
//!
//! A `Text` field of a struct or enum is flat: at construction it is copied
//! into the arena and becomes a dynamic string that the iteration `RESET`
//! reclaims. Such a value cannot cross the yield boundary, and the runtime
//! `contains_dynstr` walk cannot see it inside flat bytes, so the compiler
//! rejects the yield. A bare static string, and text inside a boxed
//! container (tuple, array, `Option`), keep their static/dynamic
//! distinction and are governed by the runtime check, so they still
//! compile.

extern crate alloc;

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;

fn compiles(src: &str) -> bool {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    compile(&program).is_ok()
}

#[test]
fn yielding_struct_with_text_field_is_rejected() {
    let src = "struct Greeting { msg: Text, n: Word }\n\
               loop main(seed: Word) -> Greeting { \
                   yield Greeting { msg: \"hi\", n: seed } \
               }";
    assert!(
        !compiles(src),
        "yielding a struct with a flat Text field must be rejected"
    );
}

#[test]
fn yielding_enum_with_text_payload_is_rejected() {
    let src = "enum Msg { Quiet, Loud(Text) }\n\
               loop main(seed: Word) -> Msg { yield Msg::Loud(\"hi\") }";
    assert!(
        !compiles(src),
        "yielding an enum with a flat Text payload must be rejected"
    );
}

#[test]
fn yielding_struct_holding_text_struct_is_rejected() {
    // A struct nesting another struct that carries Text: the inner struct's
    // Text is flat-nested into the outer body, so the outer yield is
    // rejected too (transitive).
    let src = "struct Inner { t: Text }\n\
               struct Outer { inner: Inner, n: Word }\n\
               loop main(seed: Word) -> Outer { \
                   yield Outer { inner: Inner { t: \"hi\" }, n: seed } \
               }";
    assert!(
        !compiles(src),
        "yielding a struct transitively containing a flat Text field must be rejected"
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
