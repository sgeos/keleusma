//! **THE HANDOFF'S STATE TABLE DRIFTED FOR FIVE INCREMENTS WHILE ITS STAMP WAS
//! REFRESHED EACH TIME.**
//!
//! # What happened
//!
//! `docs/process/handoffs/v0.3.0.md` carries a table headed *"State, every figure
//! re-derived at the stamp"*, inside `► RESUME HERE` — the one section that
//! declares itself authoritative. The commit stamp above it was updated on five
//! consecutive increments. **The table was not.**
//!
//! One quantity held three values in one file: the banner said 561 tests, the
//! table said 562, and the tree ran 576. The `test files` row said 120 against an
//! actual 124.
//!
//! The file warns about precisely this. It records four cases of records
//! outliving their subjects and states plainly: *"Six instruments now fire on
//! drift in code. None fires on drift in prose."* A fifth case then appeared in
//! the section headed **every figure re-derived**.
//!
//! # Why a general prose census was right to be rejected, and this is not that
//!
//! A whole-document drift matcher was attempted and rejected ON MEASUREMENT: two
//! candidates returned 169 and 32 lines, both dominated by narrative, where a
//! sentence ABOUT a corrected claim is indistinguishable from the claim. A census
//! whose population is ten times its signal reads as coverage while being none.
//! **That reasoning stands and is not being overturned here.**
//!
//! This table is not general prose. It is a fixed set of labelled rows holding
//! numbers, at one anchor, most of them derivable from the tree. The population
//! IS the table, and the signal is the whole population.
//!
//! # ⚠ ONE ROW CANNOT BE DERIVED, AND THAT IS THE INTERESTING PART
//!
//! *"576 tests, 0 failed, both float configurations, every half FROZEN"* is a
//! **run result**. No guard can re-derive it by reading files, because it is the
//! outcome of executing them. Inventing an approximate derivation that passes
//! would be worse than no check.
//!
//! So that row carries **the commit it was measured at** instead, and this file
//! asserts the attribution is present. A figure that is neither derivable nor
//! attributed is exactly the shape that drifted.
//!
//! # What this does NOT close
//!
//! The figures in one table. Not prose drift generally. The next stale sentence
//! will be somewhere none of these instruments look, and the handoff still says
//! so.

use std::path::PathBuf;

/// The handoff this line writes to.
fn handoff() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("native_codegen has a parent")
        .join("docs/process/handoffs/v0.3.0.md");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// The state table in the resume section, and nothing else.
///
/// **Scoping is the whole correctness of this file.** The document is thousands
/// of lines and most of it is deliberate history under `◄ RECORD`, carrying older
/// figures that are correct AS HISTORY. A guard reading the whole file would
/// report those as drift and be wrong.
fn state_table() -> String {
    let s = handoff();
    let start = s
        .find("### State, every figure re-derived at the stamp")
        .expect("the resume section still carries its state table");
    let rest = &s[start..];
    // The table ends at the next third-level heading.
    let end = rest[1..]
        .find("\n### ")
        .map(|i| i + 1)
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

/// The integer in the row whose label is `label`, taken from the table only.
fn row_figure(table: &str, label: &str) -> Option<u64> {
    let line = table
        .lines()
        .find(|l| l.starts_with(&format!("| {label} ")))?;
    // The first bold number on the row is the figure; the rest of the cell is
    // prose that may contain other numerals.
    let bold = line.split("**").nth(1)?;
    let digits: String = bold.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Integration test files, the figure the `test files` row states.
fn test_file_count() -> usize {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    std::fs::read_dir(dir)
        .expect("the tests directory is readable")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
        .count()
}

/// The population guard's own recorded function count, which is itself checked
/// against the tree by that guard. Read rather than recomputed, so the two
/// cannot disagree.
fn recorded_test_functions() -> usize {
    let src = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/test_population_guard.rs"),
    )
    .expect("the population guard is readable");
    let at = src
        .find("const RECORDED_TEST_FUNCTIONS: usize = ")
        .expect("the population guard still records a function count");
    src[at..]
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .expect("a number")
}

/// How many opcodes the denominator records as lowered, and how many exist.
fn isa_classification() -> (usize, usize) {
    let src = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/opcode_denominator.rs"),
    )
    .expect("the opcode denominator is readable");
    let at = src
        .find("const RECORDED: &[(&str, Class)]")
        .expect("its record");
    let end = src[at..].find("\n];").expect("the record terminates") + at;
    let body = &src[at..end];
    let total = body.matches("Class::").count();
    let lowered = body.matches("Class::Lowered").count();
    (lowered, total)
}

#[test]
fn every_derivable_figure_in_the_state_table_matches_the_tree() {
    let table = state_table();
    // **Non-vacuity.** A parse that silently matches nothing passes every check
    // below. This session has already seen a probe-name extraction return zero
    // rows and a case-sensitive matcher see two thirds of its subject.
    let rows = table.lines().filter(|l| l.starts_with("| ")).count();
    assert!(
        rows >= 7,
        "parsed only {rows} table rows from the resume section's state table. \
         This is a BROKEN PROBE, not an empty table."
    );

    let mut wrong = Vec::new();

    let files = row_figure(&table, "test files").expect("a `test files` row with a bold figure");
    if files as usize != test_file_count() {
        wrong.push(format!(
            "`test files` says {files}, the tree has {}",
            test_file_count()
        ));
    }

    let fns =
        row_figure(&table, "test functions").expect("a `test functions` row with a bold figure");
    if fns as usize != recorded_test_functions() {
        wrong.push(format!(
            "`test functions` says {fns}, the population guard records {}",
            recorded_test_functions()
        ));
    }

    let (lowered, total) = isa_classification();
    let isa = row_figure(&table, "ISA").expect("an `ISA` row with a bold figure");
    if isa as usize != lowered {
        wrong.push(format!(
            "`ISA` says the backend lowers {isa}, the denominator records {lowered} \
             of {total}"
        ));
    }

    assert!(
        wrong.is_empty(),
        "{} figure(s) in the handoff's state table disagree with the tree: \
         {wrong:?}. This table is the one the resume protocol reads first. It \
         drifted for five increments while the stamp above it was refreshed each \
         time; that is what this guard exists to stop.",
        wrong.len()
    );
}

#[test]
fn the_underivable_figure_names_the_commit_it_was_measured_at() {
    let table = state_table();
    let suite = table
        .lines()
        .find(|l| l.starts_with("| backend suite "))
        .expect("a `backend suite` row");
    // A run result cannot be re-derived by reading files, so the only honest
    // alternative to a check is PROVENANCE.
    assert!(
        suite.contains("measured at `"),
        "the `backend suite` row states a figure no guard can re-derive and does \
         not say which commit it was measured at: {suite}. Either attribute it or \
         stop stating it; an unattributed underivable figure is what drifted."
    );
}

/// The banner disagreed with the table it sits above, for five increments.
#[test]
fn the_banner_does_not_restate_a_figure_the_table_owns() {
    let s = handoff();
    let banner_end = s.find("\n---\n").expect("the banner is delimited");
    let banner = &s[..banner_end];
    assert!(
        !banner.contains("561 tests") && !banner.contains("562 tests"),
        "the opening banner carries a stale suite figure again. One quantity held \
         three values in this file once — 561 in the banner, 562 in the table, 576 \
         in the tree. The table owns this figure."
    );
}
