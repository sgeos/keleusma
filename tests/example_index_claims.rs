//! **THE SHIPPED-EXAMPLE INDEX MADE CLAIMS ITS FILES CONTRADICTED, AND NOTHING CHECKED THEM.**
//!
//! `examples/scripts/README.md` is the table a reader consults to find the example demonstrating a
//! language feature. It is prose, so it drifted, and two of its fifteen rows were wrong when this
//! guard was written:
//!
//! | row | claimed | the file contains |
//! |---|---|---|
//! | `10_multbyte.kel` | "Byte-typed arithmetic", multiplication on `Byte` operands | **no `Byte` at all**; its own header says multi-WORD digits |
//! | `01_arithmetic.kel` | `Word`, **`Float`**, **`bool`**, arithmetic, comparison, casts | sixteen lines using only `Word` |
//!
//! # How it was found, which is the transferable part
//!
//! Not by reading the index. `tests/op_tag_tables.rs` measured that the unchecked arithmetic
//! opcodes are exercised by no corpus, and a reader consulting this index would have found that
//! surprising, because the index said byte multiplication was demonstrated. **A coverage
//! measurement disagreeing with the documentation is a signal about one of them**, and here it was
//! the documentation. The census was measuring the tree; the index was describing something else.
//!
//! # What is checkable, and why the line is drawn there
//!
//! A backticked token in a row is checked when it names language surface — a TYPE or a KEYWORD
//! construct. Prose, filenames, commands and numbers are not, because a guard demanding every
//! backticked token appear verbatim would fail on correct rows and be abandoned.
//!
//! Extending from types alone to keywords took the check count from four to twelve and found no
//! further violations, so the other nine claims — `for`, `match`, `signed`, `private data`,
//! `loop main`, `newtype` — are honest. **Both directions were checked**, not only the one the
//! author cared about.
//!
//! # COMMENTS ARE STRIPPED BEFORE MATCHING, AND THAT IS DELIBERATE
//!
//! A claim satisfied only by a comment is not demonstrated. This project has already had an
//! instrument match a commented-out construct and report a finding that was not there, so the file
//! is reduced to its code before any keyword is looked for. Verified to matter: `10_multbyte.kel`
//! mentions `overflow` in its header prose AND uses it in twelve match arms, and the guard should
//! be passing because of the second.
//!
//! # What this does NOT do
//!
//! It does not check that an example is a GOOD demonstration of what it claims, only that the
//! surface it names is present. Whether `01_arithmetic.kel` should be enriched to cover the
//! primitives its title suggests, and whether a `Byte` example should exist at all, are design
//! questions about a curated progression and are recorded for the operator rather than decided
//! here.

#![cfg(feature = "compile")]

use std::collections::BTreeSet;

const INDEX: &str = include_str!("../examples/scripts/README.md");

/// Language surface a row may name and this guard can verify by presence.
///
/// Types and keyword constructs only. A token outside this set is not checked rather than being
/// treated as a violation, so the guard stays silent about prose instead of failing on it.
const CHECKABLE: &[&str] = &[
    "Word",
    "Byte",
    "Float",
    "bool",
    "Text",
    "Fixed",
    "for",
    "match",
    "signed",
    "loop main",
    "private data",
    "newtype",
    "overflow",
    "underflow",
    "let",
];

/// `(file, every backticked token in that row's topic and feature cells)`.
fn index_rows() -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for line in INDEX.lines() {
        let t = line.trim_start();
        if !t.starts_with("| [`") {
            continue;
        }
        let cells: Vec<&str> = t.split('|').collect();
        if cells.len() < 4 {
            continue;
        }
        let Some(name) = cells[1].split('`').nth(1) else {
            continue;
        };
        if !name.ends_with(".kel") {
            continue;
        }
        let mut tokens = Vec::new();
        for cell in &cells[2..4] {
            let mut parts = cell.split('`');
            let _ = parts.next();
            while let Some(tok) = parts.next() {
                tokens.push(tok.to_string());
                let _ = parts.next();
            }
        }
        out.push((name.to_string(), tokens));
    }
    out
}

/// A source file reduced to its CODE: `//` comment tails removed.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The checkable surface a token names, if any.
fn checkable(token: &str) -> Option<&'static str> {
    let head_two = token
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");
    let head_one = token.split_whitespace().next().unwrap_or(token).to_string();
    for candidate in [token.to_string(), head_two, head_one] {
        if let Some(hit) = CHECKABLE.iter().find(|c| **c == candidate) {
            return Some(hit);
        }
    }
    None
}

/// **EVERY LANGUAGE SURFACE THE INDEX NAMES IS PRESENT IN THE FILE IT NAMES.**
///
/// Derived over ALL rows rather than over the two that were wrong, so a future row with the same
/// defect is caught without anyone having thought of it.
#[test]
fn the_example_index_claims_only_what_its_files_contain() {
    let rows = index_rows();

    // NON-VACUITY ON THE ROWS. A parse that matched nothing would satisfy everything below.
    assert!(
        rows.len() >= 15,
        "the index parse found {} example rows, so it has broken rather than the index having \
         shrunk: {rows:?}",
        rows.len()
    );

    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/scripts"));
    let mut checked = 0usize;
    let mut violations: Vec<String> = Vec::new();

    for (file, tokens) in &rows {
        let path = dir.join(file);
        // A row naming a file that is not there is a defect in its own right, and loudly so.
        let Ok(src) = std::fs::read_to_string(&path) else {
            violations.push(format!(
                "{file}: the index names a file that does not exist"
            ));
            continue;
        };
        let code = code_only(&src);
        for token in tokens {
            let Some(surface) = checkable(token) else {
                continue;
            };
            checked += 1;
            if !code.contains(surface) {
                violations.push(format!(
                    "{file}: the index claims `{surface}` (from the entry `{token}`) and the \
                     file's CODE does not contain it"
                ));
            }
        }
    }

    // NON-VACUITY ON THE CLAIMS. Without this, narrowing `CHECKABLE` to nothing would make the
    // guard pass while checking nothing at all -- the failure this repository records most often.
    assert!(
        checked >= 10,
        "only {checked} checkable claims were found across {} rows, so this guard is close to \
         vacuous; widen CHECKABLE or check why the rows stopped naming language surface",
        rows.len()
    );

    assert!(
        violations.is_empty(),
        "the shipped-example index claims language surface its files do not contain:\n  {}",
        violations.join("\n  ")
    );
}

/// **THE INDEX NAMES EVERY EXAMPLE, AND NAMES NOTHING THAT IS ABSENT.**
///
/// The companion to the row check: a file present but unlisted is invisible to a reader, and a row
/// naming an absent file sends them nowhere. Both are caught here rather than by inspection.
#[test]
fn the_index_and_the_directory_agree_on_which_examples_exist() {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/scripts"));
    let on_disk: BTreeSet<String> = std::fs::read_dir(dir)
        .expect("the shipped example directory")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    let listed: BTreeSet<String> = index_rows().into_iter().map(|(f, _)| f).collect();

    assert!(
        on_disk.len() >= 15 && listed.len() >= 15,
        "non-vacuity: {} on disk, {} listed",
        on_disk.len(),
        listed.len()
    );
    assert_eq!(
        listed, on_disk,
        "the index and the directory disagree. A file on disk and not in the index is invisible \
         to a reader; a row naming a file that is not there sends them nowhere."
    );
}
