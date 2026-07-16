//! Conformance gate for the self-hosted structural-verifier block-nesting-and-target pass
//! (`compiler/kel/verify_structural.kel`, driven by `selfhost::structural_reject_*_via_kel`).
//!
//! The stage reproduces the whole of `verify.rs`'s first-pass block-nesting-and-target checks,
//! decidable from the marshalled `(class, arg)` op table alone: the block-nesting and
//! branch-target-bounds subset (slice 1) and the exact target-equality checks (slice 2,
//! reference audits D2 and E1). Two oracles bound it:
//!
//!   * POSITIVE: every self-hosted stage source plus a minimal ephemeral program compiles to a
//!     module the reference `verify()` accepts; the stage must therefore reject none of their
//!     chunks (a spurious reject would fail here). Two well-formed nested-control fragments
//!     guard the slice-2 target-equality checks against a false positive at the chunk level.
//!   * NEGATIVE: hand-built op sequences that violate exactly one invariant must be rejected by
//!     the stage, and the reference `verify()` must reject the same mutated module (a missed
//!     reject, or a rejection that does not track a real reference rejection, fails here). This
//!     mirrors the typed-verifier conformance corpus, which mutates real bytecode.

use keleusma::bytecode::{Module, Op};
use keleusma::verify::verify;
use keleusma_selfhost::selfhost::{
    compile_src, structural_reject_chunk_via_kel, structural_reject_module_via_kel,
};

/// The private-data-free ephemeral program the scaffold tests also use; a well-nested loop.
const EPHEMERAL_SRC: &str = "require word >= 32;\n\
                             shared data io { out: Word }\n\
                             loop main(r: Word) -> Word { io.out = r + 1; yield io.out }";

fn read_stage(rel: &str) -> String {
    std::fs::read_to_string(rel)
        .or_else(|_| std::fs::read_to_string(format!("compiler/{rel}")))
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// A base module with a single well-nested chunk, cloned and mutated to build the negatives.
fn base_module() -> Module {
    compile_src("loop main(r: Word) -> Word { yield r }")
}

/// The base module's entry chunk with its op vector replaced by `ops` (all other chunk
/// metadata retained). Used both to feed the stage directly and, wrapped back into the module,
/// to cross-check the reference verdict.
fn mutated_module(ops: Vec<Op>) -> Module {
    let mut m = base_module();
    m.chunks[0].ops = ops;
    m
}

// ---- POSITIVE: real, reference-accepted modules must not be rejected ----------------------

fn assert_well_nested(label: &str, src: &str) {
    let m = compile_src(src);
    assert!(
        verify(&m).is_ok(),
        "{label}: the reference verifier must accept this module for the positive oracle to bind"
    );
    assert!(
        !structural_reject_module_via_kel(&m),
        "{label}: the structural stage must not reject a reference-accepted module"
    );
}

#[test]
fn lexer_kel_is_well_nested() {
    assert_well_nested("lexer.kel", &read_stage("kel/lexer.kel"));
}

#[test]
fn parse_kel_is_well_nested() {
    assert_well_nested("parse.kel", &read_stage("kel/parse.kel"));
}

#[test]
fn reconstruct_kel_is_well_nested() {
    assert_well_nested("reconstruct.kel", &read_stage("kel/reconstruct.kel"));
}

#[test]
fn codegen_kel_is_well_nested() {
    assert_well_nested("codegen.kel", &read_stage("kel/codegen.kel"));
}

#[test]
fn analyze_kel_is_well_nested() {
    assert_well_nested("analyze.kel", &read_stage("kel/analyze.kel"));
}

#[test]
fn ephemeral_program_is_well_nested() {
    assert_well_nested("ephemeral", EPHEMERAL_SRC);
}

// ---- NEGATIVE: each slice-1 invariant, violated in isolation ------------------------------

/// The stage must reject `ops`, and the reference must reject the same mutated module, so the
/// self-hosted rejection tracks a genuine reference rejection rather than a phantom.
fn assert_rejected(label: &str, ops: Vec<Op>) {
    let mut chunk = base_module().chunks[0].clone();
    chunk.ops = ops.clone();
    assert!(
        structural_reject_chunk_via_kel(&chunk),
        "{label}: the structural stage must reject this chunk"
    );
    let m = mutated_module(ops);
    assert!(
        verify(&m).is_err(),
        "{label}: the reference verifier must also reject the mutated module"
    );
    assert!(
        structural_reject_module_via_kel(&m),
        "{label}: the module-level stage must reject the mutated module"
    );
}

#[test]
fn if_branch_target_out_of_bounds_is_rejected() {
    // If target 9999 far exceeds op_count 2.
    assert_rejected("if-target-oob", vec![Op::If(9999), Op::EndIf]);
}

#[test]
fn unclosed_loop_is_rejected() {
    // A Loop with no matching EndLoop leaves the block stack non-empty at chunk end.
    assert_rejected("unclosed-loop", vec![Op::Loop(2), Op::Add]);
}

#[test]
fn break_outside_loop_is_rejected() {
    // A Break with no enclosing Loop (loop_depth == 0).
    assert_rejected("break-outside-loop", vec![Op::Break(0), Op::Add]);
}

#[test]
fn else_without_matching_if_is_rejected() {
    // An Else with no open If on the block stack.
    assert_rejected("else-without-if", vec![Op::Else(0), Op::Add]);
}

#[test]
fn endloop_without_loop_is_rejected() {
    // An EndLoop with no open Loop on the block stack.
    assert_rejected("endloop-without-loop", vec![Op::EndLoop(0), Op::Add]);
}

// ---- NEGATIVE (slice 2): the exact target-equality checks -----------------------------------
//
// Each of these is well-nested and in-bounds -- the slice-1 checks accept it -- but violates
// one target-equality invariant. It therefore exercises the checks slice 2 adds over slice 1.

#[test]
fn no_else_if_target_not_endif_is_rejected() {
    // A no-Else If must target its EndIf (at 1); this one targets 0.
    assert_rejected("no-else-if-target", vec![Op::If(0), Op::EndIf]);
}

#[test]
fn if_with_else_target_not_else_body_is_rejected() {
    // An If with an Else must target the else-body start (else_ip + 1 = 2); this targets 0.
    assert_rejected("if-else-target", vec![Op::If(0), Op::Else(2), Op::EndIf]);
}

#[test]
fn else_target_not_endif_is_rejected() {
    // The Else's own target (4) is in bounds but is an Add, not the EndIf at 3.
    assert_rejected(
        "else-target-not-endif",
        vec![Op::If(2), Op::Else(4), Op::Add, Op::EndIf, Op::Add],
    );
}

#[test]
fn endloop_back_edge_wrong_is_rejected() {
    // The EndLoop back-edge (0) must be loop_ip + 1 = 1.
    assert_rejected("endloop-back-edge", vec![Op::Loop(2), Op::EndLoop(0)]);
}

#[test]
fn loop_exit_not_after_endloop_is_rejected() {
    // The Loop exit (1) must be endloop_ip + 1 = 2 (audit E1). Back-edge (1) is correct.
    assert_rejected("loop-exit", vec![Op::Loop(1), Op::EndLoop(1)]);
}

#[test]
fn break_target_not_loop_exit_is_rejected() {
    // The Break target (0) must equal the enclosing loop's exit (3).
    assert_rejected(
        "break-target",
        vec![Op::Loop(3), Op::Break(0), Op::EndLoop(1)],
    );
}

// ---- POSITIVE (slice 2): well-formed nested control at the chunk level ----------------------
//
// Well-nested fragments that satisfy every target-equality invariant must not be rejected,
// guarding against a slice-2 false positive on structurally valid control flow. These are
// checked at the chunk level (they are not whole programs the reference `verify()` accepts).

fn assert_chunk_well_formed(label: &str, ops: Vec<Op>) {
    let mut chunk = base_module().chunks[0].clone();
    chunk.ops = ops;
    assert!(
        !structural_reject_chunk_via_kel(&chunk),
        "{label}: the structural stage must accept this well-formed chunk"
    );
}

#[test]
fn well_formed_if_else_is_accepted() {
    // If(2) targets the else body at 2; Else(3) targets the EndIf at 3.
    assert_chunk_well_formed("if-else", vec![Op::If(2), Op::Else(3), Op::Add, Op::EndIf]);
}

#[test]
fn well_formed_loop_with_breakif_is_accepted() {
    // Loop(3) exits after the EndLoop at 2; BreakIf(3) targets that exit; EndLoop(1) back-edges
    // to the loop body at 1.
    assert_chunk_well_formed(
        "loop-breakif",
        vec![Op::Loop(3), Op::BreakIf(3), Op::EndLoop(1)],
    );
}
