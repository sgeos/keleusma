//! `Text<N>` exists as a type surface and is refused everywhere below it.
//!
//! # What this increment is, and what it deliberately is not
//!
//! `Text<N>` parses, is distinct from bare `Text`, and carries its capacity
//! through monomorphization. **Nothing below the type surface is built**: there
//! is no flat layout, no runtime representation, and no operation.
//!
//! Every stage that cannot yet handle it REFUSES with a named error rather than
//! approximating. That is deliberate and is the default-deny posture this
//! codebase states: guessing a representation is how a wrong size reaches the
//! worst-case memory analysis, which is the one number the ecosystem sells.
//!
//! **Each later increment removes one refusal.** The compiler enumerates what
//! remains, so no comment has to claim it.
//!
//! # The design these tests pin
//!
//! Static and dynamic text are DIFFERENT TYPES, not one parameterised family. A
//! literal is static and has the bare type. `Text<N>` is a flat composite
//! carrying no pointer, handle, or epoch -- which is the whole reason it is
//! admissible, since a handle implies unbounded lifetime and puts worst-case
//! memory beyond static reach.
//!
//! See `docs/decisions/TEXT_CAPACITY_TYPE.md`.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::typecheck::check;

/// The property that matters is that NO PROGRAM COMPILES, not that one stage
/// refuses. An earlier version of this asserted a type error and passed a `let`
/// annotation, which `check_composite_dimensions` does not visit -- it covers
/// signature, parameter and data-field positions. The program checked clean and
/// the test reported a missing refusal that was in fact one stage further on.
///
/// Compiling end to end asks the question the user asks.
fn compile_err(src: &str) -> String {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    match compile(&program) {
        Err(e) => format!("{e:?}"),
        Ok(_) => panic!("expected a refusal; the program compiled"),
    }
}

#[test]
fn text_with_a_capacity_parses_as_its_own_type() {
    // The parse must SUCCEED -- the refusal belongs to a later stage. If this
    // failed, the message a user sees would be a syntax error rather than a
    // statement about the feature.
    let tokens = tokenize("fn main() -> Word { let s: Text<8> = \"hi\"; 1 }").expect("lex");
    parse(&tokens).expect("Text<N> must parse; the refusal is a type-check concern");
}

/// Every declaration position refuses, not just the one that happened to be
/// tested. A `let` annotation, a parameter and a return type each reach a
/// different guard, and a feature that refuses in two of three is a feature
/// that compiles in one.
#[test]
fn text_with_a_capacity_is_refused_in_every_declaration_position() {
    let positions = [
        (
            "let annotation",
            "fn main() -> Word { let s: Text<8> = \"hi\"; 1 }",
        ),
        (
            "parameter",
            "fn f(s: Text<8>) -> Word { 1 }\nfn main() -> Word { 1 }",
        ),
        (
            "return type",
            "fn f() -> Text<8> { \"hi\" }\nfn main() -> Word { 1 }",
        ),
    ];
    for (position, src) in positions {
        let msg = compile_err(src);
        assert!(
            msg.contains("Text<N>"),
            "{position}: the refusal must name the feature, not report an unknown type: {msg}"
        );
    }
}

#[test]
fn the_refusal_says_the_feature_is_unbuilt_rather_than_the_program_is_wrong() {
    let msg = compile_err("fn f(s: Text<8>) -> Word { 1 }\nfn main() -> Word { 1 }");
    assert!(
        msg.contains("not implemented"),
        "the refusal must say the feature is unbuilt, not that the program is wrong: {msg}"
    );
}

#[test]
fn bare_text_is_unaffected_and_still_compiles() {
    // MUST-FIRE CONTROL. Without it the refusal above could be rejecting all
    // text and this file would still pass. Bare `Text` is a different type and
    // must keep working.
    let tokens = tokenize("fn main() -> Text { \"hi\" }").expect("lex");
    let mut program = parse(&tokens).expect("parse");
    check(&mut program).expect("bare Text is a distinct type and must still check");
    let program = parse(&tokens).expect("parse");
    compile(&program).expect("bare Text must still compile end to end");
}

#[test]
fn a_capacity_is_part_of_the_type_and_two_capacities_are_two_types() {
    // Both refuse today, so this pins the PARSE distinction rather than a
    // semantic one: `Text<4>` and `Text<8>` must not collapse to one type
    // expression, or they would later share a monomorphized instantiation and
    // therefore a size.
    let four = parse(&tokenize("fn f(s: Text<4>) -> Word { 1 }").expect("lex")).expect("parse");
    let eight = parse(&tokenize("fn f(s: Text<8>) -> Word { 1 }").expect("lex")).expect("parse");
    assert_ne!(
        format!("{:?}", four.functions[0].params[0].type_expr),
        format!("{:?}", eight.functions[0].params[0].type_expr),
        "Text<4> and Text<8> must be distinct type expressions"
    );
}
