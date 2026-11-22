#![cfg(feature = "compile")]
//! Information-flow labels on data fields.
//!
//! `docs/spec/GRAMMAR.md` states that the label-flow rule at every position is
//! `source.labels ⊆ target.labels`, that labels propagate through conditional
//! branches, and that the check fires at every data-field assignment. A downstream
//! host crate reported that a labelled shared field could be copied, directly or
//! through a branch or a local, into an unlabelled shared field. These cases pin the
//! rule at data fields, each with the flow the report named.

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;

const DATA: &str =
    "shared data edge { source_rank: Word, target_rank: Word@Content, code: Word, }\n";

fn outcome(body: &str, extra: &str) -> Result<(), String> {
    let src = format!(
        "{DATA}{extra}loop main(_r: Word) -> Word {{ {body} let _ = yield edge.code; 0 }}\n"
    );
    let tokens = tokenize(&src).map_err(|e| format!("lex: {}", e.message))?;
    let program = parse(&tokens).map_err(|e| format!("parse: {}", e.message))?;
    compile(&program).map(|_| ()).map_err(|e| e.message)
}

fn refused(name: &str, body: &str, extra: &str) {
    match outcome(body, extra) {
        Ok(()) => panic!("{name}: a flow from a labelled field into an unlabelled one compiled"),
        Err(m) => assert!(
            m.contains("Content") || m.contains("label"),
            "{name}: refused, but not for the label: {m}"
        ),
    }
}

fn admitted(name: &str, body: &str, extra: &str) {
    if let Err(m) = outcome(body, extra) {
        panic!("{name}: a flow the rule admits was refused: {m}");
    }
}

#[test]
fn v2_an_explicit_copy_into_an_unlabelled_field_is_refused() {
    refused("V2", "edge.code = edge.target_rank;", "");
}

#[test]
fn v3_a_copy_through_an_unlabelled_typed_local_is_refused() {
    refused("V3", "let t: Word = edge.target_rank; edge.code = t;", "");
}

#[test]
fn v1_an_implicit_flow_through_a_branch_is_refused() {
    refused(
        "V1",
        "edge.code = 0; if edge.target_rank < edge.source_rank { edge.code = 5; } else {};",
        "",
    );
}

#[test]
fn v6_an_implicit_flow_from_a_classified_local_is_refused() {
    refused(
        "V6",
        "edge.code = 0; let r = classify edge.source_rank@Content; if r < 3 { edge.code = 5; } else {};",
        "",
    );
}

#[test]
fn v4_a_labelled_parameter_returned_unlabelled_is_refused() {
    refused("V4", "", "fn f(x: Word@Content) -> Word { x }\n");
}

#[test]
fn an_unlabelled_copy_and_a_declassified_copy_are_admitted() {
    admitted("plain", "edge.code = edge.source_rank;", "");
    admitted(
        "declassified",
        "edge.code = declassify edge.target_rank@Content;",
        "",
    );
}

#[test]
fn an_implicit_flow_through_a_match_scrutinee_is_refused() {
    refused(
        "match",
        "edge.code = 0; match edge.target_rank { 0 => if true { edge.code = 5; } else {}, _ => () };",
        "",
    );
}

#[test]
fn an_implicit_flow_through_a_match_guard_is_refused() {
    refused(
        "guard",
        "edge.code = 0; match edge.source_rank { n when n < edge.target_rank => if true { edge.code = 5; } else {}, _ => () };",
        "",
    );
}

#[test]
fn an_implicit_flow_into_an_indexed_field_is_refused() {
    let data = "shared data box { secret: Word@Content, out: [Word; 2], }\n";
    let src = format!(
        "{data}loop main(_r: Word) -> Word {{ if box.secret < 3 {{ box.out[0] = 1; }} else {{}}; let _ = yield 0; 0 }}\n"
    );
    let program = parse(&tokenize(&src).expect("lex")).expect("parse");
    let Err(err) = compile(&program) else {
        panic!("an implicit flow into an array element compiled");
    };
    assert!(err.message.contains("Content"), "{}", err.message);
}

#[test]
fn a_write_into_a_field_carrying_the_condition_label_is_admitted() {
    let data = "shared data box { secret: Word@Content, out: Word@Content, }\n";
    let src = format!(
        "{data}loop main(_r: Word) -> Word {{ if box.secret < 3 {{ box.out = 1; }} else {{}}; let _ = yield 0; 0 }}\n"
    );
    let program = parse(&tokenize(&src).expect("lex")).expect("parse");
    if let Err(e) = compile(&program) {
        panic!("a write the label rule admits was refused: {}", e.message);
    }
}

#[test]
fn a_let_may_raise_a_plain_value_to_a_labelled_annotation() {
    // The reversed check refused this before 2026-11-22. A value flows into a
    // target that carries more labels than it does.
    admitted(
        "raise",
        "let t: Word@Content = edge.source_rank; let _ = t;",
        "",
    );
}

fn compile_data(data: &str, body: &str) -> Result<(), String> {
    let src = format!("{data}loop main(_r: Word) -> Word {{ {body} let _ = yield 0; 0 }}\n");
    let program = parse(&tokenize(&src).expect("lex")).expect("parse");
    compile(&program).map(|_| ()).map_err(|e| e.message)
}

#[test]
fn a_negative_labelled_field_admits_a_label_it_does_not_list() {
    // The specification relaxes the positive rule at a `!`-labelled boundary.
    let data = "shared data b { other: Word@Other, open: Word@!Secret, }\n";
    if let Err(m) = compile_data(data, "b.open = b.other;") {
        panic!("an unlisted label was refused at a negative boundary: {m}");
    }
}

#[test]
fn a_negative_labelled_field_refuses_its_label_through_a_condition() {
    let data = "shared data b { secret: Word@Secret, open: Word@!Secret, }\n";
    match compile_data(data, "if b.secret < 3 { b.open = 1; } else {};") {
        Ok(()) => panic!("an implicit flow of a forbidden label compiled"),
        Err(m) => assert!(m.contains("Secret"), "{m}"),
    }
}
