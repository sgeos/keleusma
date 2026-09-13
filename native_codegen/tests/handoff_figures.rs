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
//! # ⚠ THE FIRST VERSION OF THIS GUARD HAD THE SAME DISEASE
//!
//! It checked three rows and left three alone — and **both unchecked figures were
//! stale**, the corpus by five and the unabsorbed count by twenty. The rows it
//! checked were exactly the ones just corrected by hand.
//!
//! > **The population was the analyst's attention, not the table.** A census keyed
//! > to what you already noticed finds nothing you had not already noticed.
//!
//! So the rows are now ENUMERATED FROM THE TABLE and each must carry a
//! disposition — `checked`, `measured at <commit>`, or `no figure`. A row with
//! none fails, which makes a row added later fail closed instead of joining
//! silently. **That rule is the deliverable; the individual corrections are
//! secondary.**
//!
//! # What this does NOT close
//!
//! The figures in one table. Not prose drift generally. The next stale sentence
//! will be somewhere none of these instruments look, and the handoff still says
//! so.

use std::path::PathBuf;

mod common;

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

/// Every row of the table, as `(label, disposition cell)`.
///
/// **Derived from the table, not named in advance.** Naming the rows is what let
/// two stale figures sit outside the guard's population.
fn rows(table: &str) -> Vec<(String, String)> {
    table
        .lines()
        .filter(|l| l.starts_with("| ") && l.ends_with('|'))
        .filter_map(|l| {
            let cells: Vec<&str> = l.trim_matches('|').split('|').map(str::trim).collect();
            // The header separator and the empty spacer row carry no label.
            if cells.len() < 3 || cells[0].is_empty() || cells[0].starts_with("---") {
                return None;
            }
            // **The LAST cell, not the third.** A cell containing an escaped
            // pipe splits into more than three, and reading a fixed index then
            // silently mistakes prose for a disposition.
            Some((cells[0].to_string(), (*cells.last()?).to_string()))
        })
        .collect()
}

/// Modules the differential harness builds, and how many the backend refuses.
///
/// **The harness's own enumeration and source composition**, not a file count:
/// five rtos scripts were once recorded as compiler failures when the missing
/// prelude was the cause.
fn corpus_figures() -> (usize, usize) {
    let mut built = 0;
    let mut refused = 0;
    for p in common::corpus_sources() {
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let is_rtos = p.components().any(|c| c.as_os_str() == "rtos");
        let is_prelude = p.file_name().is_some_and(|n| n == "prelude.kel");
        let src = if is_rtos && !is_prelude {
            match std::fs::read_to_string("../examples/rtos/scripts/prelude.kel") {
                Ok(pr) => format!("{pr}\n{src}"),
                Err(_) => src,
            }
        } else {
            src
        };
        if let Some(m) = common::try_build(&src) {
            built += 1;
            if !keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default())
                .is_empty()
            {
                refused += 1;
            }
        }
    }
    (built, refused)
}

/// **The rule that makes the population the table.**
#[test]
fn every_row_carries_a_disposition() {
    let table = state_table();
    let rows = rows(&table);
    assert!(
        rows.len() >= 6,
        "parsed only {} labelled rows from the state table; a BROKEN PROBE rather \
         than a short table",
        rows.len()
    );
    let undisposed: Vec<&str> = rows
        .iter()
        .filter(|(_, d)| !(d == "checked" || d.starts_with("measured at `") || d == "no figure"))
        .map(|(l, _)| l.as_str())
        .collect();
    assert!(
        undisposed.is_empty(),
        "{} row(s) of the state table carry no disposition: {undisposed:?}. Each \
         row must be `checked` against the tree, `measured at <commit>` when no \
         guard can re-derive it, or `no figure`. An undisposed row is how two \
         stale figures sat outside this guard's population while it reported \
         everything in order.",
        undisposed.len()
    );
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

    let (built, refused) = corpus_figures();
    let corpus = row_figure(&table, "corpus").expect("a `corpus` row with a bold figure");
    if corpus as usize != built {
        wrong.push(format!(
            "`corpus` says {corpus} modules, the harness builds {built}"
        ));
    }
    // The refusal count is the SECOND bold figure on that row.
    let corpus_line = table
        .lines()
        .find(|l| l.starts_with("| corpus "))
        .expect("a corpus row");
    let refused_said: usize = corpus_line
        .split("**")
        .nth(3)
        .and_then(|c| {
            c.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .ok()
        })
        .expect("a second bold figure on the corpus row");
    if refused_said != refused {
        wrong.push(format!(
            "`corpus` says {refused_said} refused, the backend refuses {refused}"
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
