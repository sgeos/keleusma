//! A comparison of two calls' `Word` results bears no text term once the callees' text-ness is
//! known.
//!
//! The self-hosted lexer's `intern_id` compares `peek_at(i) != peek_at(j)`. A chunk-only WCET
//! query must treat each call result as possibly unbounded text, so it reported that comparison
//! unboundable, and the native line could export no cost claim for the lexer. The operator ruled
//! first-hand on 2026-10-28 that the refusal is to be fixed. The callee-aware entry knows that
//! `peek_at` returns a `Word`, so the comparison costs only its flat cycles. The chunk-only entry
//! keeps its conservative refusal, which is the control showing the two differ.

#![cfg(feature = "self-host")]

use keleusma::bytecode::NOMINAL_COST_MODEL;
use keleusma::verify::{
    module_chunk_returns_text, module_wcet_with_bounds, wcet_whole_chunk_with_callees,
    wcet_whole_chunk_with_cost_model,
};

fn lexer() -> keleusma::bytecode::Module {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/selfhost/kel/lexer.kel"
    ))
    .expect("lexer source");
    keleusma::selfhost::compile_src(&src)
}

#[test]
fn intern_id_is_bounded_once_its_callees_are_known() {
    let module = lexer();
    let index = module
        .chunks
        .iter()
        .position(|c| c.name == "intern_id")
        .expect("the lexer has intern_id");
    let chunk = &module.chunks[index];
    // The control. Without callee knowledge the comparison is unboundable.
    let shallow = wcet_whole_chunk_with_cost_model(chunk, &NOMINAL_COST_MODEL, &[]);
    assert!(
        shallow
            .as_ref()
            .is_err_and(|e| e.message.contains("unbounded-length string")),
        "the chunk-only query no longer refuses, so this test no longer shows the repair: {shallow:?}"
    );
    let returns_text = module_chunk_returns_text(&module).expect("call graph");
    assert_eq!(returns_text.len(), module.chunks.len());
    let bounded = wcet_whole_chunk_with_callees(chunk, &NOMINAL_COST_MODEL, &[], &returns_text)
        .expect("intern_id is bounded with its callees known");
    assert!(bounded > 0 && bounded < u32::MAX, "{bounded}");
    // The module-level entry uses the callees too.
    let all = module_wcet_with_bounds(&module, &[], &NOMINAL_COST_MODEL)
        .expect("every lexer chunk is bounded at module level");
    assert_eq!(all[index], bounded);
}

/// A comparison of two text results is still unboundable, so the repair narrows only the
/// non-text case.
#[test]
fn a_comparison_of_two_text_results_stays_unbounded() {
    let src = "fn t(x: Text) -> Text { x }\nfn f(a: Text, b: Text) -> bool { t(a) == t(b) }\nfn main() -> Word { 0 }";
    let module = keleusma::selfhost::compile_src(src);
    let index = module.chunks.iter().position(|c| c.name == "f").expect("f");
    let returns_text = module_chunk_returns_text(&module).expect("call graph");
    let r = wcet_whole_chunk_with_callees(
        &module.chunks[index],
        &NOMINAL_COST_MODEL,
        &[],
        &returns_text,
    );
    assert!(r.is_err(), "a text comparison became bounded: {r:?}");
}
