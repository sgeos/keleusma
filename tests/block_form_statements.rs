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
