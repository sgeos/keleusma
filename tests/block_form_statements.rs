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
/// cause — a stray semicolon after the `for` block — and that form is
/// **accepted**, which `a_trailing_semicolon_after_for_is_accepted_as_it_is_after_if`
/// below pins.
///
/// **The citation here previously named a test asserting the REJECTION**, which
/// is the opposite of what the tree does. A pointer that resolves into a
/// contradiction is worse than one that resolves to nothing, because it looks
/// like a reference.
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

/// A trailing semicolon after `for` is accepted, as it is after the other three.
///
/// **THE ASYMMETRY THIS TEST ONCE PINNED IS REMOVED**, on the operator's
/// direction, by admitting a bare semicolon as an empty statement. The test is
/// kept and inverted rather than deleted: it names the exact forms that changed
/// meaning, so a regression is reported here rather than as a puzzling failure
/// somewhere downstream.
///
/// # What the asymmetry was, and why it cost more than it looked
///
/// `for` is dispatched as a STATEMENT and consumes exactly the loop, so a
/// following `;` reached the expression parser and was reported as
/// `unexpected token Semicolon in expression`. `if`, `match` and `loop` are
/// EXPRESSIONS, so in statement position they take the expression-statement
/// path, which already consumed a terminator.
///
/// The diagnostic named a construct the author had not written, and that is what
/// made it expensive. It produced two incorrect reports from the parallel line:
/// that the grammar's documented `break` form was rejected, and that
/// `Op::BreakIf` was unreachable from any documented source form. Both probe
/// sources carried a stray semicolon after a `for` block. Neither report was
/// about `break`, which
/// `the_break_statement_documented_in_the_grammar_parses` and
/// `a_conditional_break_reaches_the_breakif_opcode` above establish separately.
#[test]
fn a_trailing_semicolon_after_for_is_accepted_as_it_is_after_if() {
    // Accepted after each of the three block forms, unchanged. All three are
    // named in the grammar's rule, so all three are checked rather than
    // generalised from `if`.
    assert!(parses(
        "fn f(a: Word, b: Word) -> Word { if a > b { let z = 1; }; b }"
    ));
    assert!(parses("fn f(x: Word) -> Word { match x { _ => 1, }; 0 }"));
    assert!(parses(
        "loop main(r: Word) -> Word { loop { let x = yield r; }; }"
    ));

    // AND NOW AFTER `for`, which is the change. With `break` present, which is
    // the shape the parallel line's two reports actually tripped over.
    assert!(parses(
        "data s { n: Word }\n\
         fn main(a: Word, b: Word) -> Word { \
           let xs = [1, 2, 3, 4]; \
           for x in xs { if x > a { break; } s.n = x; }; \
           b \
         }"
    ));

    // And with `break` removed, the control that showed `break` was never the
    // thing the parser objected to.
    assert!(parses(
        "data s { n: Word }\n\
         fn main(a: Word, b: Word) -> Word { \
           let xs = [1, 2, 3, 4]; \
           for x in xs { s.n = x; }; \
           b \
         }"
    ));
}

/// The widening admits an empty statement and nothing else.
///
/// **MUST-NOT-FIRE.** An empty statement is the smallest change that removes the
/// asymmetry, and the risk of admitting one is that it masks a genuine error by
/// letting a malformed block parse. These cases assert the parser still refuses
/// what it refused before, so the widening is bounded rather than merely
/// convenient.
#[test]
fn the_empty_statement_does_not_admit_a_malformed_block() {
    // A semicolon does not stand in for a missing tail expression. The block
    // parses, and the TYPE CHECKER rejects it, which is the right layer.
    assert!(!compiles("fn f() -> Word { ; }"));
    assert!(!compiles("fn f() -> Word { let x = 1; ; }"));

    // A `break` still requires its terminator, so the empty statement has not
    // made the semicolon optional anywhere it was mandatory.
    assert_eq!(
        parse_error("fn f(a: [Word; 8]) -> Word { for i in 0..8 { if a[i] > 0 { break } 0 } 0 }"),
        "expected Semicolon"
    );

    // A stray semicolon does not make a broken expression parse.
    assert_eq!(
        parse_error("fn f() -> Word { let x = ; 0 }"),
        "unexpected token Semicolon in expression"
    );
}

/// Repeated and leading empty statements parse, which is the form the change
/// actually admits.
///
/// Stated explicitly because it is the part a reader would not predict from the
/// motivating case: the arm fires in statement-start position, so it accepts a
/// run of semicolons and one before any other statement, exactly as Rust does.
#[test]
fn a_run_of_empty_statements_parses() {
    assert!(parses("fn f() -> Word { ;;; 0 }"));
    assert!(parses("fn f() -> Word { let x = 1;;; x }"));
    assert!(parses(
        "fn f(a: [Word; 4]) -> Word { for i in 0..4 { };; 0 }"
    ));
}

/// The user guide's claim that a trailing semicolon is REQUIRED after an
/// if-else at statement position, checked rather than trusted.
///
/// `book/src/FAQ.md` carries an entry titled "If-else at statement position
/// requires a trailing semicolon", whose example marks the semicolon with
/// `<-- this semicolon is required`. `block_form_if_is_a_statement_without_semicolon`
/// above shows the semicolon is optional before a block's TAIL EXPRESSION, which
/// is a different position from the one the guide describes: there the if-else
/// is followed by another STATEMENT.
///
/// This test covers the guide's exact shape, so the documentation rests on an
/// executed check rather than on a plausible reading of a neighbouring one.
#[test]
fn an_if_else_before_another_statement_needs_no_semicolon() {
    // The guide's own example, reduced to the two statements it is about.
    assert!(parses(
        "data state { rem0: Word, rem1: Word }\n\
         fn f() -> Word { \
           if state.rem0 == 0 { state.rem0 = 1; } else { state.rem0 = state.rem0 - 1; } \
           state.rem1 = state.rem1 - 1; \
           0 \
         }"
    ));

    // And with the semicolon, which must remain accepted either way.
    assert!(parses(
        "data state { rem0: Word, rem1: Word }\n\
         fn f() -> Word { \
           if state.rem0 == 0 { state.rem0 = 1; } else { state.rem0 = state.rem0 - 1; }; \
           state.rem1 = state.rem1 - 1; \
           0 \
         }"
    ));
}
