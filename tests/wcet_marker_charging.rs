#![cfg(all(feature = "compile", feature = "verify"))]
//! The verified time bound charges every dispatched instruction.
//!
//! Until 2026-10-25 the time walk charged nothing for `Else`, `EndIf` and `EndLoop`, and it took
//! the maximum of a counted loop's full passes and its break path rather than their sum, so the
//! final header pass went uncharged. The nominal cost table charges each marker one cycle and the
//! virtual machine dispatches each one, so the bound fell short of the cost model. The hosted
//! runtime's instruction count exposed it, and the operator ruled first-hand that day that it be
//! fixed. Each expected figure below is derived by hand from the instruction listing in its
//! comment, so a later change to the walk is checked against the cost model and not against the
//! walk's own earlier output.

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify::wcet_whole_chunk;

fn bound(src: &str) -> u32 {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    wcet_whole_chunk(&module.chunks[0]).expect("a bound")
}

#[test]
fn an_if_else_charges_the_else_and_the_endif() {
    // PushImmediate 1, If 1, then Const 1 and Else 1, EndIf 1, Return 2. The then-path is the
    // longer at 7 cycles. The bound was 5.
    assert_eq!(bound("fn main() -> Word { if true { 1 } else { 2 } }"), 7);
}

#[test]
fn a_statement_if_charges_its_markers() {
    // GetLocal 1, If 1, then Const PopN PushImmediate 3 and Else 1, EndIf 1, then PopN Const 2
    // and Return 2. The then-path is 11 cycles.
    assert_eq!(bound("fn main(a: bool) -> Word { if a { 1; } 5 }"), 11);
}

#[test]
fn a_counted_loop_charges_each_back_edge_and_the_final_header() {
    // Setup Const SetLocal Const SetLocal and Loop, 5. Each full pass is the header GetLocal
    // GetLocal CmpGe BreakIf, 5, the empty body PushImmediate PopN, 2, the increment GetLocal
    // Const CheckedAdd PopN SetLocal, 6, and EndLoop 1, so 14, three times, 42. The final pass
    // runs only the header, 5. Then GetLocal 1 and Return 2. Total 55. The bound was 47.
    assert_eq!(
        bound("fn main(n: Word) -> Word { for i in 0..3 { } n }"),
        55
    );
}

#[test]
fn an_empty_counted_loop_still_charges_its_header() {
    // A zero-pass loop runs its header once and breaks. Setup 5, header 5, tail 3.
    assert_eq!(
        bound("fn main(n: Word) -> Word { for i in 0..0 { } n }"),
        13
    );
}
