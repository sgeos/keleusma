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

/// **A GUARD PROVES ITS REACH BEFORE IT PROVES THE TREE.** The refusal above is
/// checked against three declaration positions chosen by hand, and three examples
/// establish that three examples are covered. This enumerates the positions a
/// type expression can actually occupy, by class rather than by example, because
/// the refusal's whole value is that NO program reaches code generation on the
/// strength of an unbuilt type.
///
/// The history here is the argument. Increment 1 shipped a refusal that four of
/// five declaration positions walked straight past, because the infallible
/// `TypeExpr`-to-`Type` conversion resolved `Text<N>` to static text before any
/// fallible pass ran. That was found by trying positions, not by reading the
/// guard, and the guard has since been rewritten as a whole-program walk. The
/// question this test asks is whether the REWRITE reaches everywhere, which is a
/// different question from whether it fixed the three cases that prompted it.
///
/// A position added to the language later and not added here is a hole this test
/// cannot see. That is a real limit and it is recorded rather than papered over:
/// the enumeration is maintained by hand because `TypeExpr` positions are spread
/// across declaration forms rather than reachable from one visitor.
#[test]
fn no_position_that_can_name_a_type_admits_an_unbuilt_one() {
    // Each entry is (position name, program). Every one must be refused.
    let positions: &[(&str, &str)] = &[
        (
            "let annotation",
            "fn main() -> Word { let s: Text<8> = \"hi\"; 1 }",
        ),
        (
            "function parameter",
            "fn f(s: Text<8>) -> Word { 1 }\nfn main() -> Word { 1 }",
        ),
        (
            "function return",
            "fn f() -> Text<8> { \"hi\" }\nfn main() -> Word { 1 }",
        ),
        // NESTED positions. The finder for `let` annotations matched only an
        // OUTERMOST `Text<N>`, so a capacity inside an array, tuple or option
        // was invisible to it while being just as unbuilt.
        (
            "let annotation, inside an array",
            "fn main() -> Word { let s: [Text<8>; 2] = [\"a\", \"b\"]; 1 }",
        ),
        (
            "let annotation, inside a tuple",
            "fn main() -> Word { let s: (Word, Text<8>) = (1, \"a\"); 1 }",
        ),
        (
            "let annotation, inside an option",
            "fn main() -> Word { let s: Option<Text<8>> = Option::None; 1 }",
        ),
        (
            "parameter, inside an array",
            "fn f(s: [Text<8>; 2]) -> Word { 1 }\nfn main() -> Word { 1 }",
        ),
        // DECLARATION forms other than a function. These are not reached by the
        // function walk at all; whether anything else refuses them is exactly
        // what is unknown without asking.
        (
            "struct field",
            "struct S { t: Text<8> }\nfn main() -> Word { 1 }",
        ),
        (
            "struct field, inside an array",
            "struct S { t: [Text<8>; 2] }\nfn main() -> Word { 1 }",
        ),
        (
            "enum variant payload",
            "enum E { V(Text<8>) }\nfn main() -> Word { 1 }",
        ),
        (
            "cast target",
            "fn main() -> Word { let s = \"hi\" as Text<8>; 1 }",
        ),
        (
            "let annotation, doubly nested",
            "fn main() -> Word { let s: [(Word, Text<8>); 2] = [(1, \"a\"), (2, \"b\")]; 1 }",
        ),
        // A trait declaration is a type position that the whole-program walk does
        // NOT visit: it iterates functions and impl blocks. Whether anything
        // else refuses it is exactly what is unknown without asking.
        (
            "trait method signature",
            "trait T { fn f(t: Text<8>) -> Word; }\nfn main() -> Word { 1 }",
        ),
        (
            "trait method signature and its impl",
            "trait T { fn f(t: Text<8>) -> Word; }\nstruct S { a: Word }\n\
             impl T for S { fn f(t: Text<8>) -> Word { 1 } }\nfn main() -> Word { 1 }",
        ),
    ];

    let mut admitted: Vec<&str> = Vec::new();
    let mut reached_compile: Vec<&str> = Vec::new();
    // Records WHY a case stopped, not merely that it did. A bare name here would
    // make a mistyped fixture indistinguishable from a genuine early refusal,
    // and a test naming a position its body never reaches is worse than no test:
    // it consumes the attention that would have written a real one.
    let mut stopped_earlier: Vec<String> = Vec::new();
    for (name, src) in positions {
        let tokens = match tokenize(src) {
            Ok(t) => t,
            // A lex or parse failure is a refusal too: the program does not
            // compile, which is the property under test.
            Err(e) => {
                stopped_earlier.push(format!("{name}: lex: {e:?}"));
                continue;
            }
        };
        let program = match parse(&tokens) {
            Ok(p) => p,
            Err(e) => {
                stopped_earlier.push(format!("{name}: parse: {e:?}"));
                continue;
            }
        };
        reached_compile.push(name);
        if compile(&program).is_ok() {
            admitted.push(name);
        }
    }

    // **NON-VACUOUS.** Counting a parse failure as a refusal is correct for the
    // property -- the program does not compile either way -- but it means this
    // test could pass while NOTHING reached the compiler, which would be a guard
    // measuring nothing while reporting success. Two derivations in this
    // repository have already passed that way. The floor is stated as a count so
    // that a syntax change silently diverting cases into the parser fails here
    // rather than quietly emptying the population.
    assert_eq!(
        reached_compile.len(),
        positions.len(),
        "every fixture must REACH the compiler, or it tests the parser instead of the \
         refusal. A lex or parse failure would still satisfy the property under test -- the \
         program does not compile either way -- which is precisely why it must not be \
         accepted silently: this test would then pass while measuring nothing. Equality \
         rather than a floor, so a syntax change that breaks a fixture fails here instead of \
         quietly shrinking the population. Stopped earlier: {stopped_earlier:?}"
    );

    assert!(
        admitted.is_empty(),
        "these positions COMPILED a type that is not built below its surface: {admitted:?}"
    );
}
