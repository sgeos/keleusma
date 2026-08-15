#![cfg(all(feature = "compile", feature = "verify"))]
//! Block-form expressions (`if`, `if`/`else`, `match`, `loop`) are valid
//! statements without a trailing semicolon, as in Rust. This removes friction
//! from compiler-style dispatch-then-continue code. Guards the parser fix.

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;

fn parses(src: &str) -> bool {
    tokenize(src).ok().and_then(|t| parse(&t).ok()).is_some()
}
fn compiles(src: &str) -> bool {
    tokenize(src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .map(|p| compile(&p).is_ok())
        .unwrap_or(false)
}

/// The parse diagnostic, so a negative test can assert WHICH failure fired
/// rather than merely that one did.
fn parse_error(src: &str) -> String {
    let toks = tokenize(src).expect("the probe sources all lex");
    match parse(&toks) {
        Ok(_) => panic!("expected a parse rejection, got acceptance"),
        Err(e) => e.message,
    }
}

#[test]
fn block_form_if_is_a_statement_without_semicolon() {
    assert!(parses("fn f() -> Word { if 1 == 1 { let x = 1; } 0 }"));
    assert!(parses(
        "fn f() -> Word { if 1 == 1 { let x = 1; } else { let y = 2; } 0 }"
    ));
}

#[test]
fn block_form_match_is_a_statement_without_semicolon() {
    assert!(parses("fn f(x: Word) -> Word { match x { _ => 1, } 0 }"));
}

#[test]
fn semicolon_and_tail_forms_are_unchanged() {
    assert!(parses("fn f() -> Word { if 1 == 1 { 1 } else { 2 }; 0 }"));
    assert!(parses("fn f() -> Word { if 1 == 1 { 1 } else { 2 } }"));
}

#[test]
fn a_non_block_expression_statement_still_requires_a_semicolon() {
    assert!(!parses("fn f() -> Word { 1 + 2 3 }"));
}

#[test]
fn a_block_form_statement_program_compiles() {
    // The unit-valued `if` runs for effect, then 0 is returned.
    assert!(compiles(
        "shared data d { n: Word }\nfn f() -> Word { if 1 == 1 { d.n = 1; } 0 }"
    ));
}

/// The `break` statement documented in `docs/spec/GRAMMAR.md` is accepted.
///
/// Reported from the `v0.3.0` line as a grammar-versus-parser discrepancy, on
/// the reading that the documented form is rejected. It is not. The example in
/// the grammar's "Break Statement" section parses verbatim, and so does every
/// neighbouring shape. The rejection that prompted the report had another
/// cause, pinned by
/// `a_trailing_semicolon_after_for_is_rejected_where_after_if_it_is_accepted`
/// below.
#[test]
fn the_break_statement_documented_in_the_grammar_parses() {
    // The grammar's own example, transcribed with only a function wrapper added.
    assert!(parses(
        "fn f(channels: [Float; 8]) -> Word {\n\
           for i in 0..8 {\n\
             if channels[i] > 0.0 {\n\
               break;\n\
             }\n\
             audio::set_volume(i, 1.0);\n\
           }\n\
           0\n\
         }"
    ));

    // `break` as the whole loop body, and as the whole body of a conditional
    // that is itself the whole loop body. Neither needs a trailing statement to
    // keep the conditional out of value position.
    assert!(parses("fn f() -> Word { for i in 0..8 { break; } 0 }"));
    assert!(parses(
        "fn f(a: [Word; 8]) -> Word { for i in 0..8 { if a[i] > 0 { break; } } 0 }"
    ));

    // The semicolon is required, which is the one thing about the form that a
    // reader could get wrong from the grammar alone.
    assert_eq!(
        parse_error("fn f(a: [Word; 8]) -> Word { for i in 0..8 { if a[i] > 0 { break } 0 } 0 }"),
        "expected Semicolon"
    );
}

/// A conditional `break` reaches `Op::BreakIf` from the documented form.
///
/// The `v0.3.0` opcode audit recorded `BreakIf` as unreachable from any
/// documented source form, and left it unisolated for that reason. It is
/// reachable. The audit's own probe source carries a stray semicolon after the
/// `for` block, and that is what the parser rejected.
#[test]
fn a_conditional_break_reaches_the_breakif_opcode() {
    use keleusma::bytecode::Op;

    // The `v0.3.0` audit's `break_cond` probe with the stray semicolon after
    // the `for` block removed, and nothing else changed.
    let src = "data s { n: Word }\n\
               fn main(a: Word, b: Word) -> Word { \
                 let xs = [1, 2, 3, 4]; \
                 for x in xs { if x > a { break; } s.n = x; } \
                 b \
               }";
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let main = module
        .chunks
        .iter()
        .find(|c| c.name == "main")
        .expect("no chunk named main");

    assert!(
        main.ops.iter().any(|op| matches!(op, Op::BreakIf(_))),
        "the conditional break must lower to BreakIf, ops were {:?}",
        main.ops
    );
    assert!(
        main.ops.iter().any(|op| matches!(op, Op::Break(_))),
        "the loop's own exit must lower to Break, ops were {:?}",
        main.ops
    );
}

/// A trailing semicolon after `for` is rejected where after `if` it is accepted.
///
/// This is the real defect behind the reported `break` discrepancy, and it has
/// nothing to do with `break`. The second case below removes `break` entirely
/// and fails identically, which is what establishes that.
///
/// `semicolon_and_tail_forms_are_unchanged` above pins the accepting half for
/// `if`, so the two tests together state an asymmetry rather than a rule.
///
/// PINNED, NOT REPAIRED. Accepting the trailing semicolon widens the admitted
/// language, and widening it is the operator's call rather than a correctness
/// fix. This test records the behaviour that exists so a later change to it is
/// deliberate and visible.
#[test]
fn a_trailing_semicolon_after_for_is_rejected_where_after_if_it_is_accepted() {
    // Accepted after each of the three block forms, restating the other half of
    // the asymmetry here so the comparison is visible in one place. All three
    // are named in the grammar's rule, so all three are checked rather than
    // generalised from `if`.
    assert!(parses(
        "fn f(a: Word, b: Word) -> Word { if a > b { let z = 1; }; b }"
    ));
    assert!(parses("fn f(x: Word) -> Word { match x { _ => 1, }; 0 }"));
    assert!(parses(
        "loop main(r: Word) -> Word { loop { let x = yield r; }; }"
    ));

    // Rejected after `for`, with `break` present.
    assert_eq!(
        parse_error(
            "data s { n: Word }\n\
             fn main(a: Word, b: Word) -> Word { \
               let xs = [1, 2, 3, 4]; \
               for x in xs { if x > a { break; } s.n = x; }; \
               b \
             }"
        ),
        "unexpected token Semicolon in expression"
    );

    // Rejected after `for` with `break` removed, which is the control. The
    // failure is identical, so `break` is not what the parser objected to.
    assert_eq!(
        parse_error(
            "data s { n: Word }\n\
             fn main(a: Word, b: Word) -> Word { \
               let xs = [1, 2, 3, 4]; \
               for x in xs { s.n = x; }; \
               b \
             }"
        ),
        "unexpected token Semicolon in expression"
    );
}
