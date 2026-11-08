//! **THE NATIVE JOBS' PATH FILTER MUST COVER EVERY INPUT THE SUITE READS.**
//!
//! # Why a path filter needs a guard
//!
//! The native jobs of `.github/workflows/ci.yml` run only when the `changes` job
//! reports `native=true`, which `scripts/ci_classify.py` decides from
//! `NATIVE_PATTERNS`. That is a saving only while the list is complete. A path the
//! suite reads and the list omits is a change that merges without the native gate
//! ever running, and it would look exactly like a quiet day: nothing red, because
//! nothing ran.
//!
//! The read set is not maintained by hand here. It is `REACH` in
//! `tools/gate-status.sh`, which `reach_census.rs` already checks against every
//! external path the suite's sources name. This file closes the remaining link:
//! every `REACH` entry must be matched by `NATIVE_PATTERNS`.
//!
//! # Why the workflow layout needs a guard too
//!
//! Until 2026-11-04 the native jobs were a separate workflow that ran only for
//! `main` and `v0.3.0` and that `CI result` never saw, and three native tests
//! drifted on `v0.2.3` with nothing red. The operator ruled that day that all
//! continuous integration is unified. This file therefore also checks that the
//! jobs live in `ci.yml`, which runs for `main` and every version branch, that
//! they read the classifier's `native` output, and that `CI result` and
//! `certify` both need them, so a red native gate fails the required check.
//!
//! # What this does NOT establish
//!
//! **Build inputs are outside `REACH`**, since `REACH` is about the inputs the
//! suite reads rather than the ones the compiler does. `NATIVE_PATTERNS` lists
//! those separately, and this guard checks only that it covers `REACH`. A new path
//! dependency of the root crate would need adding by hand, and nothing here would
//! notice its absence. The classifier's matching itself is tested by
//! `scripts/test_ci_classify.py`, not here.

fn workflow() -> String {
    std::fs::read_to_string("../.github/workflows/ci.yml").expect("the root workflow")
}

fn classifier() -> String {
    std::fs::read_to_string("../scripts/ci_classify.py").expect("the change classifier")
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

/// The quoted entries of `NATIVE_PATTERNS`, one per line as the classifier writes them.
fn native_patterns(text: &str) -> Vec<String> {
    let start = text
        .find("\nNATIVE_PATTERNS = (\n")
        .expect("the classifier declares NATIVE_PATTERNS");
    let body = &text[start + "\nNATIVE_PATTERNS = (\n".len()..];
    let end = body.find("\n)").expect("NATIVE_PATTERNS closes");
    body[..end]
        .lines()
        .map(|l| {
            l.trim()
                .trim_end_matches(',')
                .trim_matches('\'')
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect()
}

/// The classifier pattern that must cover one `REACH` entry.
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

/// The lines of one top-level job of `ci.yml`, from its key to the next line at
/// job indentation, which is the next job or the comment block above it.
fn job(text: &str, key: &str) -> Option<String> {
    let header = format!("  {key}:");
    let mut lines = text.lines().skip_while(|l| *l != header);
    lines.next()?;
    let body: Vec<&str> = lines
        .take_while(|l| !(l.starts_with("  ") && !l.starts_with("   ")))
        .collect();
    Some(body.join("\n"))
}

/// The jobs named on a job's `needs:` line.
fn needs(text: &str, key: &str) -> Vec<String> {
    job(text, key)
        .and_then(|j| {
            j.lines()
                .map(str::trim)
                .find_map(|l| l.strip_prefix("needs:").map(str::to_string))
        })
        .map(|n| {
            n.trim()
                .trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Every way the workflow layout can drop the native gate, as messages.
fn layout_errors(text: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let branches: Vec<&str> = text
        .lines()
        .take_while(|l| !l.starts_with("jobs:"))
        .filter_map(|l| l.trim().strip_prefix("branches:"))
        .map(str::trim)
        .collect();
    if branches.len() != 2 || branches.iter().any(|b| *b != "[main, 'v*']") {
        errors.push(format!(
            "the push and pull_request triggers are {branches:?}, not main and every version branch"
        ));
    }
    let changes = job(text, "changes").unwrap_or_default();
    if !changes.contains("native: ${{ steps.classify.outputs.native }}") {
        errors.push("the changes job no longer exposes the classifier's native output".into());
    }
    let condition =
        "if: needs.changes.outputs.native == 'true' && needs.changes.outputs.mode != 'certified'";
    for key in ["native", "native-linux"] {
        match job(text, key) {
            None => errors.push(format!("ci.yml has no `{key}` job")),
            Some(j) if !j.contains(condition) => errors.push(format!(
                "`{key}` no longer runs on the classifier's native output"
            )),
            Some(_) => {}
        }
        for aggregate in ["ci-result", "certify"] {
            if !needs(text, aggregate).iter().any(|n| n == key) {
                errors.push(format!(
                    "`{aggregate}` does not need `{key}`, so a red native gate would not fail it"
                ));
            }
        }
    }
    errors
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
    let patterns = native_patterns(&classifier());
    assert!(
        patterns.len() >= reach.len(),
        "the parsed filter is implausibly short: {patterns:?}"
    );
    let missing: Vec<String> = reach
        .iter()
        .map(|e| pattern_for(e))
        .filter(|p| !patterns.contains(p))
        .collect();
    assert!(
        missing.is_empty(),
        "the native jobs do not run when {missing:?} changes, although the suite \
         reads it. Add it to NATIVE_PATTERNS in scripts/ci_classify.py."
    );

    let errors = layout_errors(&workflow());
    assert!(errors.is_empty(), "{errors:#?}");
}

/// MUST-FIRE: a filter missing one entry is reported, and so is a layout in which
/// `CI result` no longer needs the native gate or the triggers lose the version
/// branches.
#[test]
fn a_filter_missing_an_input_is_detected() {
    let text = classifier().replace("    'docs/decisions/ABSORPTION_*_BRIEF.md',\n", "");
    let patterns = native_patterns(&text);
    assert!(
        !patterns.contains(&pattern_for("../docs/decisions/ABSORPTION_*_BRIEF.md")),
        "the control did not remove the entry, so it proves nothing"
    );

    let original = workflow();
    let unneeded = original.replace(", native, native-linux, certify]", ", certify]");
    assert_ne!(unneeded, original, "the control did not edit `CI result`");
    assert!(
        layout_errors(&unneeded)
            .iter()
            .any(|e| e.contains("`ci-result` does not need `native`")),
        "a `CI result` without the native jobs was not reported"
    );
    let main_only = original.replace("branches: [main, 'v*']", "branches: [main]");
    assert_ne!(main_only, original, "the control did not edit the triggers");
    assert!(
        !layout_errors(&main_only).is_empty(),
        "triggers without the version branches were not reported"
    );
}

/// Both float configurations run the COMPLETE gate script, not a subset.
#[test]
fn both_configurations_run_the_complete_gate() {
    let text = job(&workflow(), "native").expect("ci.yml has a native job");
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
    let linux = job(&workflow(), "native-linux").expect("ci.yml has a native-linux job");
    assert!(
        linux.contains("KEL_REQUIRE_LLVM_TOOLS: '1'")
            && linux.contains("--test tier_one_targets --test kel_native_cli"),
        "the Linux targets no longer build, link and run their two tests"
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
