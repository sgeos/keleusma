#![cfg(all(feature = "compile", feature = "verify"))]
//! `docs/decisions/COMPOSITE_REGION_EVIDENCE.md` must keep pointing at things
//! that exist.
//!
//! # Why this is guarded rather than trusted
//!
//! That document is written for a session drafting a proof, on another branch,
//! who will not be in a position to notice that a citation has gone stale. It
//! tells its reader which claims are execution-backed and names the tests that
//! back them, so **a renamed test turns the document from evidence into a
//! confident-sounding dead end** — worse than having written nothing.
//!
//! This repository has already paid for the general form of this: a defect
//! report citing `mod.rs:718` outlived the line it named, and the citation had
//! to be replaced with a symbol. Line numbers are kept here because they are
//! genuinely useful to a reader navigating a 5,000-line file, and they are
//! pinned in BOTH directions so that keeping them costs nothing later.

use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn evidence() -> String {
    fs::read_to_string(root().join("docs/decisions/COMPOSITE_REGION_EVIDENCE.md"))
        .expect("the evidence index is present")
}

/// Every test the document names as backing a claim.
///
/// A claim marked EXECUTED whose test no longer exists is the failure this
/// guards: the document would still read as evidence.
const CITED_TESTS: &[&str] = &[
    "two_iterations_composites_are_live_together_and_distinct",
    "a_yielded_composite_outlives_its_iteration_and_dies_at_reset",
    "reset_is_once_per_stream_cycle_not_once_per_loop_iteration",
    "a_composite_written_to_private_data_is_copied_not_aliased",
    "nesting_a_composite_into_a_flat_one_copies_its_bytes_inline",
    "no_compiled_stream_chunk_emits_return",
    "a_stream_calling_a_stream_compiles_verifies_and_runs",
    "a_loop_body_may_not_consume_from_below_its_entry_height",
    "compiled_loops_really_do_carry_a_non_empty_entry_stack",
    "the_instruction_set_has_no_write_accessor_into_a_composite",
    "a_dispatch_break_may_carry_a_value_past_the_loop_entry_height",
    "composite_equality_is_content_derived_not_address_derived",
    "a_composite_written_to_an_indexed_data_slot_is_copied_not_aliased",
];

/// The `src/verify.rs` citations, as (line, the text that line must contain).
///
/// Pinned as content AND position. A proof author reading the document will go
/// to the line; if the code moved, they must be sent to a failing test rather
/// than to whatever now occupies it.
const CITED_LINES: &[(usize, &str)] = &[
    (992, "then_branch.heap_total.max(else_branch.heap_total)"),
    (1079, "body_heap_one.saturating_mul(iter_count)"),
    (1087, "body_heap.max(break_heap)"),
];

#[test]
fn every_test_the_evidence_index_names_still_exists() {
    let doc = evidence();
    let tests_dir = root().join("tests");
    let all: String = fs::read_dir(&tests_dir)
        .expect("tests/ is readable")
        .flatten()
        .map(|e| fs::read_to_string(e.path()).unwrap_or_default())
        .collect();

    for name in CITED_TESTS {
        assert!(
            doc.contains(name),
            "{name} is pinned here as a citation but the evidence index no \
             longer names it; the pin and the document have parted"
        );
        assert!(
            all.contains(&format!("fn {name}(")),
            "the evidence index names `{name}` as backing an EXECUTED claim, \
             and no such test exists. The document would still read as evidence \
             to someone who cannot check it."
        );
    }
}

#[test]
fn the_verify_citations_point_at_what_the_document_says_they_do() {
    let doc = evidence();
    let verify = fs::read_to_string(root().join("src/verify.rs")).expect("src/verify.rs readable");
    let lines: Vec<&str> = verify.lines().collect();

    for (line, expected) in CITED_LINES {
        assert!(
            doc.contains(&format!("src/verify.rs:{line}")),
            "the evidence index no longer cites src/verify.rs:{line}; update \
             this pin deliberately rather than letting the two drift"
        );
        let actual = lines
            .get(line - 1)
            .unwrap_or_else(|| panic!("src/verify.rs has no line {line}"));
        assert!(
            actual.contains(expected),
            "src/verify.rs:{line} no longer contains {expected:?}; it is now \
             {actual:?}. The evidence index sends a proof author to that line to \
             see what adopting the theorem would change."
        );
    }
}

#[test]
fn the_index_states_its_limits_and_its_ownership() {
    let doc = evidence();

    // The document's value depends on a reader being able to tell an executed
    // claim from a read one, and on not editing another line's files. Both are
    // easy to lose in an edit that only meant to tidy.
    for required in [
        "read from dispatch",
        "trust boundary",
        "has NOT established",
        "not an edit",
        "BOXED",
    ] {
        assert!(
            doc.contains(required),
            "the evidence index no longer contains {required:?}. Every one of \
             these marks a limit on what it establishes; without them it reads \
             as a stronger document than it is."
        );
    }
}
