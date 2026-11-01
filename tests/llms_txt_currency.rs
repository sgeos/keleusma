//! **THE AGENT-FACING MANIFEST STILL DESCRIBES THE TREE.**
//!
//! `llms.txt` is 58 lines, **ships in the crate tarball**, and states its own purpose: models
//! consuming or extending Keleusma should "prefer consulting the agent-onboarding file and the
//! architecture description first". A wrong claim there propagates into generated code and into
//! other agents' context.
//!
//! **NOTHING CHECKED IT, AND `documentation_links.rs` CANNOT.** That guard resolves RELATIVE
//! markdown links; every link in this file is an absolute GitHub URL, so the file was outside its
//! reach by construction rather than by oversight.
//!
//! Measured 2026-09-27, before this file existed, four claims were wrong:
//!
//! - the instruction set was given as **69** opcodes against the specification's **66**. That
//!   claim was TRUE of V0.2.0; the V0.2.1 `NewComposite` consolidation retired four construct
//!   opcodes and added one. It is a version-scoped statement that went stale, which is a
//!   different diagnosis from confusing it with the maximum live wire id, coincidentally also 69.
//! - `docs/guide/COOKBOOK.md` and `docs/guide/FAQ.md` did not exist. Commit `f745b16e` ported the
//!   guide to an mdbook and they are now under `book/src/`. Two of twenty-nine referenced paths
//!   were dead, and the file had not been touched since before that port.
//! - the branching model was described as "Trunk-based development with short-lived feature
//!   branches", while `GIT_STRATEGY.md` — the file that link points at — opens with "a
//!   **release-branch model** with a four-level hierarchy".
//!
//! **THE PATH CHECK IS DELIBERATELY OFFLINE.** It strips the GitHub prefix and tests the path
//! against the working tree. What rots is the tree, not the host, and a networked check would not
//! run in a sandbox.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn manifest() -> String {
    std::fs::read_to_string(root().join("llms.txt")).expect("the agent-facing manifest llms.txt")
}

/// Every repository path the manifest references, from a `blob/main/` or `tree/main/` URL.
fn referenced_paths(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for marker in ["blob/main/", "tree/main/"] {
        let mut from = 0usize;
        while let Some(rel) = text[from..].find(marker) {
            let at = from + rel + marker.len();
            let rest = &text[at..];
            let end = rest
                .find(|c: char| c == ')' || c.is_whitespace())
                .unwrap_or(rest.len());
            let p = rest[..end].to_string();
            if !p.is_empty() && !out.contains(&p) {
                out.push(p);
            }
            from = at;
        }
    }
    out
}

/// The first integer immediately preceding `needle` in `text`.
fn figure_before(text: &str, needle: &str) -> Option<usize> {
    let at = text.find(needle)?;
    let head = &text[..at];
    let digits: String = head
        .chars()
        .rev()
        .skip_while(|c| c.is_whitespace() || *c == '-')
        .take_while(char::is_ascii_digit)
        .collect();
    digits.chars().rev().collect::<String>().parse().ok()
}

/// **EVERY REFERENCED PATH EXISTS.**
///
/// A dead link in a file whose job is orientation sends a reader, human or model, to a 404 and
/// leaves them guessing where the material went.
#[test]
fn every_path_the_manifest_references_exists() {
    let text = manifest();
    let paths = referenced_paths(&text);

    // NON-VACUITY. An extraction that found nothing would satisfy the loop below trivially, which
    // is the shape of failure this repository keeps meeting.
    assert!(
        paths.len() >= 20,
        "extracted only {} referenced paths, so the extraction has broken rather than the file \
         having shrunk",
        paths.len()
    );

    let missing: Vec<&String> = paths.iter().filter(|p| !root().join(p).exists()).collect();
    assert!(
        missing.is_empty(),
        "{} path(s) referenced by llms.txt do not exist: {:?}. This file ships in the crate \
         tarball and exists to orient automated consumers, so a dead path misdirects them.",
        missing.len(),
        missing
    );
}

/// **THE STATED OPCODE COUNT AGREES WITH THE SPECIFICATION.**
///
/// The specification is authoritative; the manifest is a summary of it. Both figures are read
/// from their files so neither can drift while the other is edited.
#[test]
fn the_manifest_states_the_opcode_count_the_specification_states() {
    let text = manifest();
    let spec = std::fs::read_to_string(root().join("docs/spec/INSTRUCTION_SET.md"))
        .expect("the instruction-set specification");

    let claimed = figure_before(&text, "-opcode instruction set").expect(
        "llms.txt no longer states an opcode count in the expected form; update this \
                 extraction rather than leaving the check unable to find anything",
    );
    let authoritative = figure_before(&spec, " opcodes. The B28 consolidation")
        .or_else(|| figure_before(&spec, " opcodes."))
        .expect("the specification no longer states its opcode total in the expected form");

    // NON-VACUITY is carried by the two `expect`s above: a figure that cannot be found is a
    // failure, not a silently satisfied comparison.
    assert_eq!(
        claimed, authoritative,
        "llms.txt states {claimed} opcodes and the specification states {authoritative}. The \
         manifest ships in the crate tarball and is read by models; a wrong instruction-set size \
         propagates into generated code."
    );
}

/// **THE MANIFEST DOES NOT CONTRADICT THE DOCUMENT IT LINKS TO.**
///
/// Asserting that two documents describe a model "the same way" is not mechanisable. This asserts
/// the specific contradiction that was there: one called it trunk-based while the other opens by
/// calling it a release-branch model.
#[test]
fn the_manifest_does_not_call_the_branching_model_trunk_based() {
    let text = manifest().to_lowercase();
    let strategy = std::fs::read_to_string(root().join("docs/process/GIT_STRATEGY.md"))
        .expect("the git strategy document")
        .to_lowercase();

    // Non-vacuity: the contradiction only exists if the strategy really says this.
    assert!(
        strategy.contains("release-branch model"),
        "GIT_STRATEGY.md no longer describes a release-branch model, so this check's premise has \
         changed and it must be re-derived rather than left asserting a stale contrast"
    );
    assert!(
        !text.contains("trunk-based"),
        "llms.txt calls the branching model trunk-based while GIT_STRATEGY.md, the file that link \
         points at, describes a release-branch model with a four-level hierarchy"
    );
}
