//! **A LIGHT PULL REQUEST RUNS EVERY TEST THAT READS WHAT IT MAY CHANGE.**
//!
//! Since 2026-10-25 a pull request that changes only documentation, the book, process
//! scripts and a few unbuilt files runs a light set of jobs. `scripts/ci_classify.py`
//! decides, and the `guards` job of `ci.yml` runs the test binaries that read those
//! files. A test that reads documentation and is missing from that selection would pass
//! on the light run by not running, and fail only after the merge. This file is the
//! check for that case, and it runs in the full matrix, which is the run that adds any
//! new test.
//!
//! Two rules follow.
//!
//! 1. Every test binary that reads a documentation, book, workflow or script path, or a
//!    top-level Markdown file, is named by a `--test` selector in the `guards` job.
//! 2. A heavy `selfhost_*` binary is not run by the light path, so every such path it
//!    reads must be one of the classifier's guarded documents, or end in one of its
//!    full-run suffixes such as `.kel`, either of which forces a full run.
//!
//! The scan matches literal paths in code lines, and skips `//` comments, which name
//! paths constantly. A path assembled at run time is invisible to it. That residual is
//! why the branch run after a light pull request stays full. The reach assertions fail
//! when the scan finds too little, because a scan that stopped matching would pass.

use std::collections::BTreeSet;
use std::path::Path;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The code lines of a source, without `//` comment lines.
fn code_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter(|l| !l.trim_start().starts_with("//"))
}

/// The string literals of one line, taken as the segments between double quotes.
fn literals(line: &str) -> impl Iterator<Item = &str> {
    line.split('"').skip(1).step_by(2)
}

/// True when a literal names a path the classifier may treat as light.
fn names_light_path(literal: &str) -> bool {
    let path = literal.trim_start_matches("../");
    const PREFIXES: [&str; 4] = ["docs/", "book/", "scripts/", ".github/"];
    PREFIXES.iter().any(|p| path.starts_with(p))
        || matches!(path, "docs" | "book" | "scripts" | ".github" | "llms.txt")
        || (!path.contains('/') && path.ends_with(".md"))
}

/// The light-path literals a test source reads, in order of appearance.
fn light_reads(text: &str) -> Vec<String> {
    code_lines(text)
        .flat_map(literals)
        .filter(|l| names_light_path(l) || l.ends_with(".md"))
        .map(str::to_string)
        .collect()
}

/// The binaries the `guards` job selects with `--test name`.
fn guard_selection() -> BTreeSet<String> {
    let workflow = std::fs::read_to_string(root().join(".github/workflows/ci.yml"))
        .expect("read ci.yml; if the workflow moved, update this guard");
    let mut in_guards = false;
    let mut names = BTreeSet::new();
    for line in workflow.lines() {
        let is_job_key = line.starts_with("  ")
            && !line.starts_with("   ")
            && line.trim_end().ends_with(':')
            && !line.trim_start().starts_with('#');
        if is_job_key {
            in_guards = line.trim() == "guards:";
            continue;
        }
        if !in_guards || line.trim_start().starts_with('#') {
            continue;
        }
        let mut tokens = line.split_whitespace();
        while let Some(token) = tokens.next() {
            if token == "--test"
                && let Some(name) = tokens.next()
            {
                names.insert(name.to_string());
            }
        }
    }
    names
}

/// The suffixes the classifier always sends to a full run, read from its
/// `FULL_SUFFIXES = (...)` line, so the two cannot disagree.
fn full_suffixes(classifier: &str) -> Vec<String> {
    classifier
        .lines()
        .find(|l| l.starts_with("FULL_SUFFIXES = ("))
        .map(|l| {
            l.split('\'')
                .skip(1)
                .step_by(2)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Every integration-test source of this package, as (binary name, text).
fn test_sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root().join("tests")).expect("read tests/") {
        let path = entry.expect("a tests/ entry").path();
        if path.extension().is_some_and(|x| x == "rs") {
            let name = path
                .file_stem()
                .expect("a file name")
                .to_string_lossy()
                .into_owned();
            let text = std::fs::read_to_string(&path).expect("read a test source");
            out.push((name, text));
        }
    }
    out.sort();
    out
}

#[test]
fn every_test_that_reads_documentation_runs_on_the_light_path() {
    let selection = guard_selection();
    let classifier = std::fs::read_to_string(root().join("scripts/ci_classify.py"))
        .expect("read the change classifier");
    let suffixes = full_suffixes(&classifier);
    assert!(
        suffixes.iter().any(|s| s == ".kel"),
        "read no `.kel` from FULL_SUFFIXES in scripts/ci_classify.py, found {suffixes:?}"
    );
    let mut errors = Vec::new();
    let mut readers = 0usize;
    for (name, text) in test_sources() {
        let reads = light_reads(&text);
        if reads.is_empty() {
            continue;
        }
        readers += 1;
        if name.starts_with("selfhost") {
            for read in reads
                .iter()
                .filter(|r| names_light_path(r) && r.contains('/'))
            {
                let path = read.trim_start_matches("../");
                let forced_full = suffixes.iter().any(|s| path.ends_with(s.as_str()));
                if !forced_full && !classifier.contains(&format!("'{path}'")) {
                    errors.push(format!(
                        "heavy test `{name}` reads `{path}`, which is not a guarded document \
                         in scripts/ci_classify.py, so a light pull request changing it would \
                         skip this test"
                    ));
                }
            }
        } else if !selection.contains(&name) {
            errors.push(format!(
                "`{name}` reads {:?} but the `guards` job does not select `--test {name}`",
                reads.iter().take(3).collect::<Vec<_>>()
            ));
        }
    }
    // REACH. On 2026-10-25 the scan found 18 readers, one of them the heavy `selfhost_codegen`, and the selection named 17 binaries.
    assert!(
        readers >= 12,
        "the scan found only {readers} documentation readers"
    );
    assert!(
        selection.len() >= 12,
        "the guards job selects only {} binaries",
        selection.len()
    );
    for name in &selection {
        if !root().join("tests").join(format!("{name}.rs")).exists() {
            errors.push(format!(
                "the guards job selects `{name}`, which has no source in tests/"
            ));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

/// The scan itself sees a documentation read and ignores a comment. A guard that cannot
/// fail is not evidence.
#[test]
fn the_scan_sees_a_documentation_read_and_ignores_a_comment() {
    assert_eq!(
        light_reads("    let s = read(\"../docs/spec/GRAMMAR.md\");"),
        vec!["../docs/spec/GRAMMAR.md"]
    );
    assert_eq!(
        light_reads("    let p = root.join(\"book\").join(x);"),
        vec!["book"]
    );
    assert_eq!(
        light_reads("    let r = read(\"README.md\");"),
        vec!["README.md"]
    );
    assert!(light_reads("    // see \"docs/process/README.md\"").is_empty());
    assert!(light_reads("    let s = read(\"src/lib.rs\");").is_empty());
    assert!(light_reads("    let s = read(\"examples/scripts/a.kel\");").is_empty());
}
