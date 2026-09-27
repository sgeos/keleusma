//! **EVERY COMPLETE PROGRAM PRINTED IN THE BOOK IS CHECKED BY SOMETHING.**
//!
//! The book's examples are executed by `book/ci/verify_examples.py`. That script needs a
//! Python interpreter and a built CLI, so it runs in a continuous-integration job rather
//! than in this suite. This file holds the part that needs neither: the STRUCTURAL
//! property that no complete program is left unchecked, derived from `book/src` on every
//! branch and in every feature configuration this suite runs in.
//!
//! **Why a second, weaker guard is worth its cost.** The executing job answers "do the
//! book's programs still work". This answers "is every program still reachable by that
//! job", which is a different question and the one that failed silently. Measured on
//! 2026-09-26, before this file existed: the book held 60 complete programs and the
//! script asserted 26 of them, the other 34 passing through a `continue` that no count
//! recorded. An aggregate of "51 examples checked" concealed it.
//!
//! **The rules here are deliberately duplicated from the script rather than shared.**
//! There is no sound way to share them across a Python file and a Rust test, and a
//! divergence is itself detectable: the script's floors are parsed out of its source
//! below and compared against this file's own census, so a rule that stopped agreeing
//! shows up as a floor that no longer describes the tree.

use std::path::{Path, PathBuf};

fn book_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("book/src")
}

/// A bare fenced block: the book's convention for Keleusma code. Rust, shell, and text
/// snippets are language-tagged and are consumed and skipped, exactly as the script does.
struct Block {
    file: String,
    line: usize,
    code: String,
    /// The `verify:` expectation written within three lines above the opening fence.
    marker: Option<String>,
    /// Whether prose within fifteen lines below the closing fence states an output.
    claimed: bool,
}

fn fence_info(line: &str) -> Option<&str> {
    let t = line.trim();
    t.strip_prefix("```")
}

/// `fn main`, `yield main`, `loop main`, with any of the `signed` and `impure` modifiers.
/// Hand-rolled rather than a regex: this crate carries no regex dependency, and the shape
/// is small enough to match directly.
fn has_entry_point(code: &str) -> bool {
    for raw in code.lines() {
        let mut rest = raw.trim_start();
        // Consume leading modifiers, in any order and any number.
        loop {
            let before = rest;
            for m in ["signed ", "impure "] {
                if let Some(r) = rest.strip_prefix(m) {
                    rest = r.trim_start();
                }
            }
            if rest == before {
                break;
            }
        }
        for kw in ["fn ", "yield ", "loop "] {
            if let Some(r) = rest.strip_prefix(kw) {
                let r = r.trim_start();
                if let Some(after) = r.strip_prefix("main") {
                    // `main` must end here, so `mainline` does not match.
                    if !after.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn marker_in(line: &str) -> Option<String> {
    let at = line.find("<!--")?;
    let rest = &line[at + 4..];
    let end = rest.find("-->")?;
    let body = rest[..end].trim();
    let spec = body.strip_prefix("verify:")?;
    Some(spec.trim().to_string())
}

/// Whether prose states an output. Mirrors the script's three patterns; this only has to
/// agree on PRESENCE, since the value is compared by the script and not here.
fn states_an_output(window: &str) -> bool {
    let lower = window.to_lowercase();
    for pat in ["output is", "prints `"] {
        if let Some(at) = lower.find(pat) {
            // A backtick or a fenced value must follow within the same neighbourhood.
            if window[at..].contains('`') {
                return true;
            }
        }
    }
    false
}

fn blocks() -> Vec<Block> {
    let mut out = Vec::new();
    let dir = book_src();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the book source directory")
        .map(|e| e.expect("a directory entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    files.sort();
    for path in files {
        let name = path
            .file_name()
            .expect("a file name")
            .to_string_lossy()
            .to_string();
        let text = std::fs::read_to_string(&path).expect("read a book chapter");
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0usize;
        while i < lines.len() {
            let Some(info) = fence_info(lines[i]) else {
                i += 1;
                continue;
            };
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim() != "```" {
                j += 1;
            }
            if info.trim().is_empty() {
                let code = lines[i + 1..j.min(lines.len())].join("\n");
                let marker = (i.saturating_sub(3)..i)
                    .rev()
                    .find_map(|k| marker_in(lines[k]));
                let hi = (j + 16).min(lines.len());
                let window = if j + 1 < hi {
                    lines[j + 1..hi].join("\n")
                } else {
                    String::new()
                };
                out.push(Block {
                    file: name.clone(),
                    line: i + 1,
                    code,
                    marker,
                    claimed: states_an_output(&window),
                });
            }
            i = j + 1;
        }
    }
    out
}

/// **NO COMPLETE PROGRAM IS UNCHECKED.**
///
/// A block carrying a `main` entry point either states its output, which the executing job
/// compares, or carries an explicit expectation beside it. The failure mode this closes is
/// a program that is printed, read, copied, and never run by anything.
#[test]
fn every_complete_program_in_the_book_is_claimed_or_marked() {
    let all = blocks();

    // NON-VACUITY on the census itself. A walk that found nothing would satisfy every
    // assertion below, which is the shape of failure this whole file exists to reject.
    let programs: Vec<&Block> = all.iter().filter(|b| has_entry_point(&b.code)).collect();
    assert!(
        all.len() >= 140 && programs.len() >= 50,
        "the census found {} bare blocks and {} complete programs, so it has broken rather \
         than the book having shrunk. A census that finds nothing passes everything.",
        all.len(),
        programs.len()
    );

    let unchecked: Vec<String> = programs
        .iter()
        .filter(|b| !b.claimed && b.marker.is_none())
        .map(|b| format!("{}:{}", b.file, b.line))
        .collect();
    assert!(
        unchecked.is_empty(),
        "{} complete program(s) in the book are neither output-claimed nor carry a \
         `<!-- verify: ... -->` expectation, so nothing executes them: {:?}. Add an \
         expectation above the fence. It renders as nothing and is not translated.",
        unchecked.len(),
        unchecked
    );
}

/// **EVERY EXPECTATION IS ONE THE EXECUTING JOB UNDERSTANDS.**
///
/// A misspelled marker would otherwise read as coverage while the script rejected it, or
/// worse, while a future script silently ignored it.
#[test]
fn every_verify_marker_is_well_formed_and_on_a_program() {
    let all = blocks();
    let mut bad = Vec::new();
    let mut skips = Vec::new();
    for b in &all {
        let Some(m) = &b.marker else { continue };
        let at = format!("{}:{}", b.file, b.line);
        if !has_entry_point(&b.code) {
            bad.push(format!(
                "{at}: marker on a block with no `main`, nothing to execute"
            ));
            continue;
        }
        // An optional `prelude` prefix means "prepend the preceding complete program in
        // this chapter, with its entry point removed", for the several chapters that
        // advance by printing only a replacement `main`. The status follows it.
        let status = m
            .strip_prefix("prelude")
            .map(|r| r.trim().trim_start_matches(',').trim())
            .unwrap_or(m.as_str());
        let ok = status == "accept"
            || status == "compile"
            || status == "reject"
            || (status.starts_with("reject:") && !status["reject:".len()..].trim().is_empty())
            || (status.starts_with("skip:") && !status["skip:".len()..].trim().is_empty());
        if !ok {
            bad.push(format!("{at}: unrecognised or empty expectation {m:?}"));
        }
        if status.starts_with("skip:") {
            skips.push(at);
        }
    }
    assert!(bad.is_empty(), "malformed expectations: {bad:?}");

    // An excused program is a debt, not a category. The ceiling matches the script's.
    assert!(
        skips.len() <= 6,
        "{} programs are excused from checking, ceiling 6: {:?}. Each is a program the \
         guide prints and nothing verifies.",
        skips.len(),
        skips
    );
}

/// **THE EXECUTING SCRIPT STILL CARRIES FLOORS, AND THEY STILL DESCRIBE THE TREE.**
///
/// The script's own non-vacuity is what stops it reporting success having checked nothing.
/// A floor that is deleted, zeroed, or left far below the tree is that protection quietly
/// removed, and no other test would notice.
#[test]
fn the_executing_script_floors_still_describe_the_book() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("book/ci/verify_examples.py");
    let src = std::fs::read_to_string(&script).expect("the book example verifier");

    // A PRESENCE assertion, so a naive comment strip is the correct one: an early
    // truncation makes this fail loudly rather than pass silently. The repository's
    // comment-matching sweep records that direction rule.
    let value = |name: &str| -> usize {
        let at = src.find(&format!("{name} = ")).unwrap_or_else(|| {
            panic!("the script no longer defines {name}, so its non-vacuity floor is gone")
        });
        src[at..]
            .lines()
            .next()
            .expect("the assignment line")
            .rsplit(" = ")
            .next()
            .expect("a value")
            .split_whitespace()
            .next()
            .expect("a number")
            .parse()
            .expect("a numeric floor")
    };

    let min_blocks = value("MIN_BLOCKS");
    let min_programs = value("MIN_PROGRAMS");
    let min_claims = value("MIN_CLAIM_ASSERTIONS");
    let min_accepted = value("MIN_ACCEPTED");
    for (n, v) in [
        ("MIN_BLOCKS", min_blocks),
        ("MIN_PROGRAMS", min_programs),
        ("MIN_CLAIM_ASSERTIONS", min_claims),
        ("MIN_ACCEPTED", min_accepted),
    ] {
        assert!(v > 0, "{n} is {v}, which asserts nothing");
    }

    // The floors must still be BELOW the tree, or the script cannot pass; and not so far
    // below that a collapse would slip under them.
    let all = blocks();
    let programs = all.iter().filter(|b| has_entry_point(&b.code)).count();
    assert!(
        min_blocks <= all.len() && min_programs <= programs,
        "the script's floors ({min_blocks} blocks, {min_programs} programs) exceed the tree \
         ({} blocks, {programs} programs), so the executing job cannot pass",
        all.len()
    );
    assert!(
        min_programs * 2 >= programs,
        "the program floor {min_programs} is less than half the {programs} the tree holds, \
         so half the book could vanish without failing it"
    );
}

/// **THE VERIFICATION RUNS WHERE THE WORK HAPPENS.**
///
/// It was previously invoked only from the workflow that triggers on the default branch.
/// Measured 2026-09-26: that workflow had last run on 2026-07-24 and had never run on the
/// version branch, while the book had been edited on that branch repeatedly. A freshness
/// guarantee attached to a branch the work does not pass through is not one.
#[test]
fn the_book_examples_are_verified_by_the_branch_workflow() {
    let ci = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml");
    let src = std::fs::read_to_string(&ci).expect("the continuous-integration workflow");
    // Strip comments so a mention inside one cannot satisfy this. PRESENCE assertion, so
    // an over-eager strip fails loudly; a `#` inside a quoted string would truncate a
    // line early, which cannot turn a missing invocation into a present one.
    let code: String = src
        .lines()
        .map(|l| match l.find('#') {
            Some(at) => &l[..at],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        code.contains("verify_examples.py"),
        "the branch workflow does not invoke book/ci/verify_examples.py, so the book's \
         examples are verified only when something reaches the default branch"
    );
}
