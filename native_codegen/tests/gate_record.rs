//! Guard: the backend gate's provenance record is well formed and truthful.
//!
//! # Why this guard exists, and what it deliberately does not check
//!
//! `native_codegen` is a detached package. Measured on 2026-09-24: continuous
//! integration does not mention it (zero occurrences across the workflow's
//! fourteen jobs), and the shared pre-push hook runs a `--workspace` selector,
//! which cannot reach a detached package. `tools/backend-gate.sh` is therefore the
//! only instrument covering this code, and it runs only when a human chooses to.
//!
//! That made the line's most-repeated claim -- "green in both float
//! configurations" -- unverifiable from the tree. `GATE_RECORD.md` makes it a
//! fact a reader can check, and this guard keeps the record honest.
//!
//! **It does not check staleness.** A record cannot name the commit that carries
//! it, so "recorded commit == HEAD" is false almost always; a guard built on that
//! would be permanently red, and a normally-red guard trains its reader to ignore
//! it. Relevance is reported by `tools/gate-status.sh` instead, and
//! [`guard_does_not_enforce_staleness`] pins that this guard stays out of it.
//!
//! **The checker's reach is demonstrated, not assumed.** A passing guard is
//! evidence about the guard before it is evidence about the tree, so the
//! malformed-input tests below exercise every rejection branch.

use std::process::Command;

const CONFIGS: [&str; 2] = ["default features", "narrow-float-32"];

/// Validate the record's rows. Pure over the text, with commit existence supplied
/// by the caller so the rejection branches are testable without inventing commits.
///
/// Returns the number of configuration rows validated.
fn check_record(text: &str, commit_exists: &dyn Fn(&str) -> bool) -> Result<usize, String> {
    let mut seen: Vec<&str> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        // Only configuration rows are validated; prose and the header pass through.
        if !CONFIGS
            .iter()
            .any(|c| line.starts_with(&format!("| {c} |")))
        {
            continue;
        }
        let f: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if f.len() != 5 {
            return Err(format!("row has {} fields, expected 5: {line}", f.len()));
        }
        let (cfg, commit, tree, verdict, when) = (f[0], f[1], f[2], f[3], f[4]);

        if seen.contains(&cfg) {
            return Err(format!("configuration `{cfg}` appears twice"));
        }
        seen.push(cfg);

        if commit.len() != 40 || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("`{commit}` is not a full 40-hex commit id"));
        }
        if !commit_exists(commit) {
            return Err(format!(
                "commit `{commit}` is not present in this repository"
            ));
        }
        let tree_ok = tree == "clean"
            || (tree.starts_with("dirty(")
                && tree.ends_with(')')
                && tree[6..tree.len() - 1].parse::<u32>().is_ok_and(|n| n > 0));
        if !tree_ok {
            return Err(format!("`{tree}` is not `clean` or `dirty(N)` with N > 0"));
        }
        if verdict != "PASS" && verdict != "FAIL" {
            return Err(format!("`{verdict}` is not PASS or FAIL"));
        }
        // YYYY-MM-DDTHH:MM:SSZ
        let shape_ok = when.len() == 20
            && when.ends_with('Z')
            && when.as_bytes()[10] == b'T'
            && when.chars().filter(|c| c.is_ascii_digit()).count() == 14;
        if !shape_ok {
            return Err(format!(
                "`{when}` is not a UTC instant of the recorded shape"
            ));
        }
    }
    Ok(seen.len())
}

fn record_text() -> String {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/GATE_RECORD.md");
    std::fs::read_to_string(p).unwrap_or_else(|e| {
        panic!(
            "the backend's only instrument must leave a record in the tree, but \
             {p} could not be read: {e}. Run tools/backend-gate.sh."
        )
    })
}

fn git_has(commit: &str) -> bool {
    Command::new("git")
        .args(["cat-file", "-e", &format!("{commit}^{{commit}}")])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// The record in the tree is well formed, and every commit it names is real.
#[test]
fn gate_record_is_well_formed() {
    let text = record_text();
    match check_record(&text, &git_has) {
        Ok(n) => assert!(n > 0, "the record carries no configuration rows"),
        Err(e) => panic!("GATE_RECORD.md is not honest: {e}"),
    }
}

/// The record states its own limits, so it cannot be cited as a present-tense
/// warrant. This is prose, and prose is exactly what drifted before.
#[test]
fn gate_record_states_its_limits() {
    let t = record_text().to_lowercase();
    for needle in ["does not attest", "dirty", "gate-status.sh"] {
        assert!(
            t.contains(needle),
            "the record must explain `{needle}`; without it a reader may read a \
             past verdict as a present guarantee"
        );
    }
}

// ---------------------------------------------------------------------------
// The checker's reach, demonstrated on input designed to defeat it.
// ---------------------------------------------------------------------------

const GOOD_SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn all_exist(_: &str) -> bool {
    true
}

fn row(commit: &str, tree: &str, verdict: &str, when: &str) -> String {
    format!("| default features | {commit} | {tree} | {verdict} | {when} |")
}

#[test]
fn guard_rejects_a_fabricated_commit() {
    let text = row(GOOD_SHA, "clean", "PASS", "2026-09-24T12:00:00Z");
    // Exists under a permissive oracle, rejected under a truthful one.
    assert!(check_record(&text, &all_exist).is_ok());
    let e = check_record(&text, &|_| false).unwrap_err();
    assert!(e.contains("not present"), "unexpected: {e}");
}

#[test]
fn guard_rejects_malformed_rows() {
    let bad = [
        // truncated / non-hex commit ids
        row("deadbeef", "clean", "PASS", "2026-09-24T12:00:00Z"),
        row(&"z".repeat(40), "clean", "PASS", "2026-09-24T12:00:00Z"),
        // a worktree state that hides a modified tree behind a zero
        row(GOOD_SHA, "dirty(0)", "PASS", "2026-09-24T12:00:00Z"),
        row(GOOD_SHA, "dirty(lots)", "PASS", "2026-09-24T12:00:00Z"),
        row(GOOD_SHA, "probably fine", "PASS", "2026-09-24T12:00:00Z"),
        // a verdict that is neither
        row(GOOD_SHA, "clean", "MOSTLY", "2026-09-24T12:00:00Z"),
        // timestamps that are not instants
        row(GOOD_SHA, "clean", "PASS", "yesterday"),
        row(GOOD_SHA, "clean", "PASS", "2026-09-24"),
        // a row missing a field entirely
        "| default features | clean | PASS |".to_string(),
    ];
    for t in bad {
        assert!(
            check_record(&t, &all_exist).is_err(),
            "the guard accepted a defective row: {t}"
        );
    }
}

#[test]
fn guard_rejects_a_duplicated_configuration() {
    let t = format!(
        "{}\n{}",
        row(GOOD_SHA, "clean", "PASS", "2026-09-24T12:00:00Z"),
        row(GOOD_SHA, "clean", "FAIL", "2026-09-24T13:00:00Z")
    );
    let e = check_record(&t, &all_exist).unwrap_err();
    assert!(e.contains("twice"), "unexpected: {e}");
}

/// A record naming a real commit that is not `HEAD` is ACCEPTED. Committing a
/// record necessarily produces such a state, so a guard that rejected it would be
/// red in the ordinary case.
#[test]
fn guard_does_not_enforce_staleness() {
    let t = row(GOOD_SHA, "clean", "PASS", "2026-09-24T12:00:00Z");
    assert_eq!(check_record(&t, &all_exist).unwrap(), 1);
}

/// A FAIL row is a legitimate record. The guard reports on the record's honesty,
/// not on the gate's verdict; suppressing a failure would be the actual defect.
#[test]
fn guard_accepts_a_recorded_failure() {
    let t = row(GOOD_SHA, "dirty(3)", "FAIL", "2026-09-24T12:00:00Z");
    assert_eq!(check_record(&t, &all_exist).unwrap(), 1);
}
