//! **THE NATIVE WORKFLOW'S PATH FILTER MUST COVER EVERY INPUT THE SUITE READS.**
//!
//! # Why a path filter needs a guard
//!
//! `.github/workflows/native.yml` runs only when a listed path changes. That is
//! a saving only while the list is complete. A path the suite reads and the list
//! omits is a change that merges without the native gate ever running, and it
//! would look exactly like a quiet day: nothing red, because nothing ran.
//!
//! The read set is not maintained by hand here. It is `REACH` in
//! `tools/gate-status.sh`, which `reach_census.rs` already checks against every
//! external path the suite's sources name. This file closes the remaining link:
//! every `REACH` entry must appear in both trigger lists of the workflow.
//!
//! # What this does NOT establish
//!
//! **Build inputs are outside `REACH`**, since `REACH` is about the inputs the
//! suite reads rather than the ones the compiler does. The workflow lists those
//! separately, and this guard checks only that the two lists agree with each
//! other and cover `REACH`. A new path dependency of the root crate would need
//! adding by hand, and nothing here would notice its absence.

use std::collections::BTreeSet;

fn workflow() -> String {
    std::fs::read_to_string("../.github/workflows/native.yml").expect("the native workflow")
}

/// The `REACH=( ... )` entries of `tools/gate-status.sh`, as written there.
fn reach() -> Vec<String> {
    let script = std::fs::read_to_string("tools/gate-status.sh").expect("the reporter");
    let start = script.find("REACH=(").expect("a REACH array");
    let body = &script[start + "REACH=(".len()..];
    let end = body.find(')').expect("the REACH array closes");
    // A pattern entry is quoted so the shell passes it to git unexpanded.
    body[..end]
        .split_whitespace()
        .map(|e| e.trim_matches('"').to_string())
        .collect()
}

/// The quoted entries under each `paths:` key, in order of appearance.
fn path_lists(text: &str) -> Vec<BTreeSet<String>> {
    let mut lists = Vec::new();
    let mut current: Option<BTreeSet<String>> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "paths:" {
            if let Some(done) = current.take() {
                lists.push(done);
            }
            current = Some(BTreeSet::new());
            continue;
        }
        if let Some(list) = current.as_mut() {
            match trimmed.strip_prefix("- ") {
                Some(item) => {
                    list.insert(item.trim_matches('\'').to_string());
                }
                None => lists.push(current.take().expect("an open list")),
            }
        }
    }
    lists.extend(current);
    lists
}

/// The workflow pattern that must cover one `REACH` entry.
fn pattern_for(entry: &str) -> String {
    if entry == "." {
        return "native_codegen/**".to_string();
    }
    let path = entry
        .strip_prefix("../")
        .unwrap_or_else(|| panic!("REACH entry {entry} is neither the package nor external"));
    if std::path::Path::new(path).extension().is_some() {
        path.to_string()
    } else {
        format!("{path}/**")
    }
}

#[test]
fn the_path_filter_covers_every_input_the_suite_reads() {
    let reach = reach();
    // **NON-VACUITY.** A parse that found nothing would cover everything.
    assert!(
        reach.len() >= 7,
        "only {} REACH entries parsed",
        reach.len()
    );
    let lists = path_lists(&workflow());
    assert_eq!(
        lists.len(),
        2,
        "expected one paths list for push and one for pull_request"
    );
    assert_eq!(
        lists[0], lists[1],
        "the push and pull_request filters differ, so one event skips inputs the other runs on"
    );
    assert!(
        lists[0].len() >= reach.len(),
        "the parsed filter is implausibly short"
    );
    let missing: Vec<String> = reach
        .iter()
        .map(|e| pattern_for(e))
        .filter(|p| !lists[0].contains(p))
        .collect();
    assert!(
        missing.is_empty(),
        "the native workflow does not run when {missing:?} changes, although the \
         suite reads it. Add it to both paths lists."
    );
}

/// MUST-FIRE: a filter missing one entry is reported.
#[test]
fn a_filter_missing_an_input_is_detected() {
    let text = workflow().replace("      - 'docs/decisions/ABSORPTION_*_BRIEF.md'\n", "");
    let lists = path_lists(&text);
    assert!(
        !lists[0].contains(&pattern_for("../docs/decisions/ABSORPTION_*_BRIEF.md")),
        "the control did not remove the entry, so it proves nothing"
    );
}

/// Both float configurations run the COMPLETE gate script, not a subset.
#[test]
fn both_configurations_run_the_complete_gate() {
    let text = workflow();
    assert!(
        text.contains("run: tools/backend-gate.sh ${{ matrix.flag }}"),
        "the workflow no longer runs the complete gate script"
    );
    for flag in ["flag: ''", "flag: '--narrow'"] {
        assert!(
            text.contains(flag),
            "the matrix no longer carries `{flag}`, so one configuration is ungated"
        );
    }
    assert!(
        text.contains("KEL_REQUIRE_LLVM_TOOLS: '1'"),
        "tests needing llc and llvm-readobj could skip in continuous integration"
    );
}

/// The gate's feature array is EMPTY in the default configuration, and the
/// hosted runner's `env bash` is the macOS system bash 3.2, which treats an
/// empty array as unbound under `set -u`. The first hosted run died at Clippy
/// on exactly that, while every local run under a newer bash had passed.
#[test]
fn the_gate_never_expands_its_feature_array_unguarded() {
    let script = std::fs::read_to_string("tools/backend-gate.sh").expect("the gate");
    let guarded = script.matches(r#"${feat[@]+"${feat[@]}"}"#).count();
    // **NON-VACUITY.** Clippy and the three suite runs expand it.
    assert!(guarded >= 4, "only {guarded} guarded expansions found");
    let bare = script.matches(r#""${feat[@]}""#).count();
    assert_eq!(
        bare, guarded,
        "an unguarded \"${{feat[@]}}\" fails the default configuration under bash 3.2"
    );
}
