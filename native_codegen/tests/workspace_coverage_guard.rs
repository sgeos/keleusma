//! Reach proof for `tools/workspace-coverage.sh`.
//!
//! The guard answers whether a recorded workspace-suite result still describes the
//! current tree. Every absorption prediction on this line names only `native_codegen`
//! figures, so no prediction can express staleness in the workspace, and the check is
//! missed by construction rather than by oversight.
//!
//! **Why these cases exist.** A guard that returns CURRENT on a clean tree has
//! demonstrated nothing. Each verdict is exercised here against a case whose correct
//! answer is known independently, and the inert class is exercised against a commit
//! that changed real files rather than an empty difference. That last case is the one
//! that would otherwise pass vacuously, which is the specific way a guard on this line
//! has been wrong before.
//!
//! These tests pin historical commits. If the history is absent the test FAILS rather
//! than skipping, because a silently skipped reach proof is indistinguishable from a
//! guard that never worked.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Last workspace run before the absorptions that followed it.
const LAST_WORKSPACE_RUN: &str = "03e4917f";
/// A documentation-only commit and its parent.
const DOCS_ONLY: &str = "1b03e270";
const DOCS_ONLY_PARENT: &str = "35757cce";
/// A commit touching `native_codegen/` and nothing else.
const INERT_ONLY: &str = "85d20963";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("native_codegen sits inside the repository")
        .to_path_buf()
}

fn guard(args: &[&str]) -> (i32, String) {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools")
        .join("workspace-coverage.sh");
    let out = Command::new("bash")
        .arg(&script)
        .args(args)
        .current_dir(repo())
        .output()
        .expect("run the workspace-coverage guard");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap_or(-1), text)
}

fn rev_parse(rev: &str) -> String {
    let out = Command::new("git")
        .args(["rev-parse", rev])
        .current_dir(repo())
        .output()
        .expect("run git rev-parse");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn require_commit(sha: &str) {
    let ok = Command::new("git")
        .args(["cat-file", "-e", &format!("{sha}^{{commit}}")])
        .current_dir(repo())
        .status()
        .expect("run git")
        .success();
    assert!(
        ok,
        "commit {sha} is absent, so this reach proof cannot run. Full history is \
         required; a skipped reach proof is indistinguishable from a broken guard."
    );
}

fn files_touched(sha: &str) -> Vec<String> {
    let out = Command::new("git")
        .args(["diff-tree", "--no-commit-id", "--name-only", "-r", sha])
        .current_dir(repo())
        .output()
        .expect("run git diff-tree");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Files changed between two revisions, as git reports them.
fn changed_between(since: &str, at: &str) -> Vec<String> {
    let out = Command::new("git")
        .args(["diff", "--name-only", since, at])
        .current_dir(repo())
        .output()
        .expect("run git diff");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

fn reported_count(text: &str, label: &str) -> usize {
    text.lines()
        .find(|l| l.contains(label))
        .and_then(|l| l.rsplit(':').next())
        .map(|n| n.trim().parse().expect("a count"))
        .unwrap_or_else(|| panic!("the guard did not report {label}:\n{text}"))
}

#[test]
fn source_movement_since_the_stamp_reports_compiled_staleness() {
    require_commit(LAST_WORKSPACE_RUN);

    // Resolve HEAD ONCE. The guard and the cross-check below must range over the
    // same revision. Reading `HEAD` separately in each would make this test race a
    // concurrent commit and fail for a reason that is not a defect in the guard.
    let head = rev_parse("HEAD");
    let (code, text) = guard(&["check", "--since", LAST_WORKSPACE_RUN, "--at", &head]);
    assert_eq!(
        code, 1,
        "expected the compiled-staleness exit code:\n{text}"
    );
    assert!(
        text.contains("VERDICT: STALE-COMPILED"),
        "source moved since the stamp, so the guard must say so:\n{text}"
    );

    // The verdict alone is too weak to be a reach proof. A guard that wrongly
    // classified `src/` as inert would STILL return STALE-COMPILED here, because
    // other compiled paths also moved in this range, and the test would pass for
    // the wrong reason. That exact mutation was applied and did pass before this
    // cross-check was added. So compare against git's own answer.
    let changed = changed_between(LAST_WORKSPACE_RUN, &head);
    let expected_compiled = changed
        .iter()
        .filter(|p| !p.starts_with("native_codegen/") && !p.starts_with("docs/"))
        .count();
    let expected_docs = changed.iter().filter(|p| p.starts_with("docs/")).count();

    assert_eq!(
        reported_count(&text, "compiled by the workspace suite"),
        expected_compiled,
        "the guard's compiled count must equal git's, or it is mis-classifying:\n{text}"
    );
    assert_eq!(
        reported_count(&text, "read by workspace tests only"),
        expected_docs,
        "the guard's documentation count must equal git's:\n{text}"
    );

    // And the source tree specifically must be inside the compiled class.
    assert!(
        changed.iter().any(|p| p.starts_with("src/")),
        "this range must contain src/ changes or it cannot test their classification"
    );
}

#[test]
fn documentation_movement_alone_is_reported_as_a_weaker_class() {
    require_commit(DOCS_ONLY);
    require_commit(DOCS_ONLY_PARENT);

    // Reach: the range must actually be documentation-only, or this proves nothing.
    let touched = files_touched(DOCS_ONLY);
    assert!(!touched.is_empty(), "the range must not be empty");
    assert!(
        touched.iter().all(|p| p.starts_with("docs/")),
        "this case requires a documentation-only commit; it touched {touched:?}"
    );

    let (code, text) = guard(&["check", "--since", DOCS_ONLY_PARENT, "--at", DOCS_ONLY]);
    assert_eq!(
        code, 2,
        "expected the read-only staleness exit code:\n{text}"
    );
    assert!(
        text.contains("VERDICT: STALE-READ-ONLY"),
        "documentation is read by workspace tests but not compiled by them:\n{text}"
    );
}

#[test]
fn the_inert_class_suppresses_a_real_change_and_not_merely_an_empty_one() {
    require_commit(INERT_ONLY);

    // THIS is the assertion that keeps the case from passing vacuously. A commit that
    // changed nothing would also yield CURRENT, and would prove nothing about the
    // inert list.
    let touched = files_touched(INERT_ONLY);
    assert!(
        !touched.is_empty(),
        "the inert case must suppress a REAL change; this commit touched nothing"
    );
    assert!(
        touched.iter().all(|p| p.starts_with("native_codegen/")),
        "this case requires a native_codegen-only commit; it touched {touched:?}"
    );

    let (code, text) = guard(&[
        "check",
        "--since",
        &format!("{INERT_ONLY}^"),
        "--at",
        INERT_ONLY,
    ]);
    assert_eq!(
        code, 0,
        "the detached package is invisible to the workspace suite:\n{text}"
    );
    assert!(text.contains("VERDICT: CURRENT"), "{text}");
}

#[test]
fn an_unmoved_tree_is_current() {
    let (code, text) = guard(&["check", "--since", DOCS_ONLY, "--at", DOCS_ONLY]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("VERDICT: CURRENT"), "{text}");
}

#[test]
fn an_unrecognised_path_is_treated_as_compiled_so_the_guard_fails_safe() {
    // The guard classifies by prefix. Only `native_codegen/` is inert and only `docs/`
    // is read-only, so any other path must land in the strongest class. Demonstrated
    // through a range that moved a top-level script, which is neither.
    require_commit(LAST_WORKSPACE_RUN);
    let (_code, text) = guard(&["check", "--since", LAST_WORKSPACE_RUN]);
    assert!(
        text.contains("scripts/") || text.contains("keleusma-macros/"),
        "a path on neither list must be counted as compiled:\n{text}"
    );
}
