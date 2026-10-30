//! The typed verifier refuses a Text read from a field the program built as
//! another kind.
//!
//! # The defect
//!
//! The typed pass knew a flat body only by kind and size, so `(Word, Text)` and
//! `(Text, Word)` were the same value to it. A hand-built module could read a
//! `Text` field at an offset that holds a `Word`, and the VM-internal read would
//! rebuild a pointer from the integer. The operator ruled on 2026-10-28 that the
//! verifier prove every flat Text slot is written only from a Text value. See
//! `docs/decisions/FLAT_TEXT_LAYOUT_PROOF.md`.
//!
//! # What these tests do NOT establish
//!
//! A body the program did not build, such as an entry argument, has no known
//! layout, and its reads are not checked by this pass. That boundary is a
//! question to the operator.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::bytecode::{Module, Op, StructField};
use keleusma::value_layout::ScalarKind;
use keleusma::verify::verify;

fn compiled(source: &str) -> Module {
    let tokens = keleusma::lexer::tokenize(source).expect("lex");
    keleusma::compiler::compile(&keleusma::parser::parse(&tokens).expect("parse")).expect("compile")
}

const SRC: &str =
    "struct P { a: Word, b: Text }\nfn main() -> Text { let p = P { a: 1, b: \"x\" }; p.b }";

fn text_read(m: &Module) -> (usize, usize, u16) {
    for (ci, c) in m.chunks.iter().enumerate() {
        for (i, op) in c.ops.iter().enumerate() {
            if let Op::GetField(StructField::Flat {
                offset,
                kind: ScalarKind::Text,
            }) = op
            {
                return (ci, i, *offset);
            }
        }
    }
    panic!("the probe lost its Text field read")
}

#[test]
fn a_compiled_text_read_verifies() {
    let m = compiled(SRC);
    let (_, _, offset) = text_read(&m);
    assert_ne!(offset, 0, "the Text field follows the Word");
    verify(&m).expect("the compiled module verifies");
}

#[test]
fn a_text_read_retargeted_to_a_word_field_is_refused() {
    let mut m = compiled(SRC);
    let (ci, i, _) = text_read(&m);
    m.chunks[ci].ops[i] = Op::GetField(StructField::Flat {
        offset: 0,
        kind: ScalarKind::Text,
    });
    let err = verify(&m).expect_err("a Text read of the Word field");
    assert!(err.message.contains("TextFieldMismatch"), "{}", err.message);
}
