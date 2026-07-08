#![cfg(all(feature = "compile", feature = "verify"))]
//! audit C7: the loop-bound extractor now verifies the induction variable
//! is advanced by the body. This guards that real compiler-emitted for-range
//! and for-in-array loops still verify (no false rejects); the hostile
//! increment-less case is pinned by the `verify` unit tests.
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify::verify;

fn ok(src: &str) {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    assert!(verify(&m).is_ok(), "should verify: {src}");
}

#[test]
fn compiler_for_loops_still_verify() {
    ok("fn main() -> Word { for i in 0..5 { i } 0 }");
    ok("fn main() -> Word { let a = [10, 20, 30]; for x in a { x } 0 }");
    ok("fn main() -> Word { for i in 0..0 { i } 0 }");
}
