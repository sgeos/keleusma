//! The self-hosted resource-bound validator, checked against the Rust reference.
//!
//! For each of the five self-hosted stage sources, this asserts that the ported
//! `validate_module_via_kel` (analyze.kel driven transitively) agrees with the
//! reference `keleusma::verify::verify_resource_bounds` at three arena capacities:
//! just below, exactly at, and just above the module's tightest Stream-chunk
//! transitive budget. The stages carry `main -> helper` calls, so the check exercises
//! the transitive (callee-folded) path, not just leaf chunks.
//!
//! Paths resolve from the `compiler/` package directory, so stages are read as
//! `kel/<stage>.kel`.

use keleusma::bytecode::{BlockType, Module};
use keleusma_selfhost::selfhost::{compile_src, validate_module_via_kel};

/// The module's tightest Stream-chunk transitive budget: the maximum over Stream chunks
/// of `stack + heap` from the reference `module_wcmu` (empty native table). This is the
/// capacity boundary `verify_resource_bounds` admits at.
fn stream_budget(module: &Module) -> i64 {
    let per_chunk = keleusma::verify::module_wcmu(module, &[]).expect("module_wcmu");
    module
        .chunks
        .iter()
        .zip(per_chunk.iter())
        .filter(|(c, _)| c.block_type == BlockType::Stream)
        .map(|(_, &(stack, heap))| stack as i64 + heap as i64)
        .max()
        .expect("at least one Stream chunk")
}

fn assert_agrees(path: &str) {
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let module = compile_src(&src);
    let budget = stream_budget(&module);
    // Below, at, and above the budget. The reference admits iff the capacity is at least
    // the budget (per-iteration transitive stack+heap must fit).
    for cap in [budget - 1, budget, budget + 1] {
        let kel = validate_module_via_kel(&module, cap);
        let reference = keleusma::verify::verify_resource_bounds(&module, cap as usize).is_ok();
        assert_eq!(
            kel, reference,
            "validator disagreement for {path} at capacity {cap} (budget {budget}): \
             self-hosted={kel}, reference={reference}"
        );
    }
}

#[test]
fn validate_module_via_kel_matches_reference_lexer() {
    assert_agrees("kel/lexer.kel");
}

#[test]
fn validate_module_via_kel_matches_reference_parse() {
    assert_agrees("kel/parse.kel");
}

#[test]
fn validate_module_via_kel_matches_reference_reconstruct() {
    assert_agrees("kel/reconstruct.kel");
}

#[test]
fn validate_module_via_kel_matches_reference_codegen() {
    assert_agrees("kel/codegen.kel");
}

#[test]
fn validate_module_via_kel_matches_reference_analyze() {
    assert_agrees("kel/analyze.kel");
}
