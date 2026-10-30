//! `Text<N>` as a type surface, and the refusals that remain below it.
//!
//! # What this file pins
//!
//! `Text<N>` parses, is distinct from bare `Text`, and carries its capacity
//! through monomorphization. Since the literal-cast increment a positive
//! literal capacity is admitted, and `tests/textn_literal.rs` covers the values.
//! This file pins the REACH of what is still refused. A capacity below one is
//! not a legal type, and a refusal that some declaration position walked past
//! would admit it.
//!
//! # The history that shapes the enumeration
//!
//! The first refusal of `Text<N>` was walked past by four of five declaration
//! positions, because the positions reach different guards. It was found by
//! trying positions, not by reading the guard. The enumeration below is
//! therefore by class rather than by example, and it is maintained by hand,
//! because `TypeExpr` positions are spread across declaration forms rather than
//! reachable from one visitor. A position added to the language later and not
//! added here is a hole this test cannot see.
//!
//! See `docs/decisions/TEXT_CAPACITY_TYPE.md`.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::typecheck::check;

#[test]
fn text_with_a_capacity_parses_as_its_own_type() {
    // The parse must SUCCEED. A refusal belongs to a later stage, and a syntax
    // error would tell the user nothing about the feature.
    let tokens = tokenize("fn main() -> Word { let s: Text<8> = \"hi\"; 1 }").expect("lex");
    parse(&tokens).expect("Text<N> must parse; any refusal is a later concern");
}

#[test]
fn bare_text_is_unaffected_and_still_compiles() {
    // MUST-FIRE CONTROL. Bare `Text` is a different type and must keep working.
    let tokens = tokenize("fn main() -> Text { \"hi\" }").expect("lex");
    let mut program = parse(&tokens).expect("parse");
    check(&mut program).expect("bare Text is a distinct type and must still check");
    let program = parse(&tokens).expect("parse");
    compile(&program).expect("bare Text must still compile end to end");
}

#[test]
fn a_capacity_is_part_of_the_type_and_two_capacities_are_two_types() {
    // `Text<4>` and `Text<8>` must not collapse to one type expression, or they
    // would share a monomorphized instantiation and therefore a size.
    let four = parse(&tokenize("fn f(s: Text<4>) -> Word { 1 }").expect("lex")).expect("parse");
    let eight = parse(&tokenize("fn f(s: Text<8>) -> Word { 1 }").expect("lex")).expect("parse");
    assert_ne!(
        format!("{:?}", four.functions[0].params[0].type_expr),
        format!("{:?}", eight.functions[0].params[0].type_expr),
        "Text<4> and Text<8> must be distinct type expressions"
    );
}

/// Every position that can name a type, written once with a legal capacity and
/// once with `Text<0>`. The legal form must compile and the zero form must be
/// refused. Pairing them is what makes the refusal meaningful: a position whose
/// legal form fails to compile would refuse `Text<0>` for an unrelated reason,
/// and the refusal count would then measure nothing.
#[test]
fn every_position_admits_a_positive_capacity_and_refuses_zero() {
    // Each entry is (position name, program with `CAP` standing for the capacity).
    let positions: &[(&str, &str)] = &[
        (
            "let annotation",
            "fn main() -> Word { let s: Text<CAP> = \"\" as Text<CAP>; 1 }",
        ),
        (
            "function parameter",
            "fn f(s: Text<CAP>) -> Word { 1 }\nfn main() -> Word { 1 }",
        ),
        (
            "function return",
            "fn f() -> Text<CAP> { \"\" as Text<CAP> }\nfn main() -> Word { 1 }",
        ),
        (
            "let annotation, inside an array",
            "fn main() -> Word { let s: [Text<CAP>; 2] = [\"\" as Text<CAP>, \"\" as Text<CAP>]; 1 }",
        ),
        (
            "let annotation, inside a tuple",
            "fn main() -> Word { let s: (Word, Text<CAP>) = (1, \"\" as Text<CAP>); 1 }",
        ),
        (
            "let annotation, inside an option",
            "fn main() -> Word { let s: Option<Text<CAP>> = Option::None; 1 }",
        ),
        (
            "parameter, inside an array",
            "fn f(s: [Text<CAP>; 2]) -> Word { 1 }\nfn main() -> Word { 1 }",
        ),
        (
            "struct field",
            "struct S { t: Text<CAP> }\nfn main() -> Word { 1 }",
        ),
        (
            "struct field, inside an array",
            "struct S { t: [Text<CAP>; 2] }\nfn main() -> Word { 1 }",
        ),
        (
            "enum variant payload",
            "enum E { V(Text<CAP>) }\nfn main() -> Word { 1 }",
        ),
        (
            "cast target",
            "fn main() -> Word { let s = \"\" as Text<CAP>; 1 }",
        ),
        (
            "private data field",
            "private data d { t: Text<CAP> }\nfn main() -> Word { d.t = \"\" as Text<CAP>; 1 }",
        ),
        (
            "trait method signature",
            "trait T { fn f(t: Text<CAP>) -> Word; }\nfn main() -> Word { 1 }",
        ),
        (
            "trait method signature and its impl",
            "trait T { fn f(t: Text<CAP>) -> Word; }\nstruct S { a: Word }\n\
             impl T for S { fn f(t: Text<CAP>) -> Word { 1 } }\nfn main() -> Word { 1 }",
        ),
    ];

    let mut legal_refused: Vec<String> = Vec::new();
    let mut zero_admitted: Vec<&str> = Vec::new();
    let mut zero_refused = 0usize;
    for (name, template) in positions {
        let legal = template.replace("CAP", "8");
        let zero = template.replace("CAP", "0");
        // A lex or parse failure panics here rather than counting as a refusal,
        // so a syntax change cannot quietly empty the population.
        let compiles = |src: &str| {
            let program = parse(&tokenize(src).expect("lex")).expect("parse");
            compile(&program).map(|_| ()).map_err(|e| e.message)
        };
        if let Err(msg) = compiles(&legal) {
            legal_refused.push(format!("{name}: {msg}"));
        }
        match compiles(&zero) {
            Ok(()) => zero_admitted.push(name),
            Err(msg) => {
                assert!(
                    msg.contains("Text<0>"),
                    "{name}: the refusal must name the illegal type: {msg}"
                );
                zero_refused += 1;
            }
        }
    }

    assert!(
        legal_refused.is_empty(),
        "these positions refused a LEGAL capacity, so their Text<0> refusal proves \
         nothing: {legal_refused:?}"
    );
    assert!(
        zero_admitted.is_empty(),
        "these positions COMPILED Text<0>, which is not a legal type: {zero_admitted:?}"
    );
    assert_eq!(
        zero_refused,
        positions.len(),
        "every fixture must reach the compiler and be refused"
    );
}
