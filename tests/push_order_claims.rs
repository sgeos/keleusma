//! The checked-arithmetic push order, guarded across the WHOLE repository
//! including the translation catalogues.
//!
//! # What this exists to prevent
//!
//! The runtime pushes the low half first, then the high half, then the flag, so
//! that the compiler's wrapping synthesis (`CheckedAdd; PopN(2)`) discards the
//! top two slots and leaves the wrapped low half as the expression's value. The
//! surface form binds the two halves in the OPPOSITE order, as `overflow(h, l)`,
//! so big-number arithmetic can chain. **Both orders are real**, which is why a
//! search and replace is the wrong repair and why the wrong claim is durable: a
//! site saying `(high, low)` about the BINDING is correct, and there are several.
//!
//! The `v0.3.0` line reported one site claiming the wrong PUSH order. A sweep on
//! 2026-08-13 found eight and corrected them. **That sweep's scope was the Rust
//! sources, `docs/`, and `book/src/`, and it therefore did not reach
//! `book/po/`** — where the extracted message catalogue still carried the
//! superseded English, and the Japanese translation keyed to it still stated the
//! wrong order in Japanese. Continuous integration builds the Japanese book from
//! that catalogue, so it was a shipped artifact.
//!
//! **A guard with a scope narrower than its class is the defect it prevents.**
//! This one walks the tree rather than a chosen list of directories, and asserts
//! that it reached the file the previous scope missed.
//!
//! # What is deliberately allowed
//!
//! `docs/process/` narrates the defect and its repair, and the task log carries
//! dated historical entries describing a published release. Rewriting those would
//! erase the record of the fix. They are allowed BY DIRECTORY, and the test
//! asserts the allowed set is non-empty so that a pattern which has stopped
//! matching anything cannot pass as cleanliness.

use std::fs;
use std::path::{Path, PathBuf};

/// The wrong claim, assembled at run time.
///
/// Written as a join rather than a literal so that this file does not match its
/// own guard. A must-fire check that fires on the comment explaining the fix it
/// guards has happened in this repository before.
fn wrong_push_order() -> String {
    format!("({}, {}, flag)", "high", "low")
}

/// The correct claim, assembled the same way and for the same reason.
fn right_push_order() -> String {
    format!("({}, {}, flag)", "low", "high")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn is_skipped_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "target" | "tmp" | "node_modules" | "book_out" | "site"
    )
}

/// Every text file under `root`, depth first. `book/book` is a build output and
/// is skipped by name along with the other generated directories.
fn walk(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !is_skipped_dir(&name) {
                walk(&path, out);
            }
        } else {
            out.push(path);
        }
    }
}

#[test]
fn no_live_document_states_the_checked_push_order_backwards() {
    let root = repo_root();
    let mut files = Vec::new();
    walk(&root, &mut files);

    let needle = wrong_push_order();
    let mut offenders = Vec::new();
    let mut allowed = Vec::new();
    let mut saw_catalogue = false;
    let mut saw_grammar = false;

    for path in &files {
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        if rel_str == "book/po/ja.po" {
            saw_catalogue = true;
        }
        if rel_str == "docs/spec/GRAMMAR.md" {
            saw_grammar = true;
        }
        for (n, line) in text.lines().enumerate() {
            if line.contains(&needle) {
                let site = format!("{rel_str}:{}", n + 1);
                if rel_str.starts_with("docs/process/") {
                    allowed.push(site);
                } else {
                    offenders.push(site);
                }
            }
        }
    }

    // Non-vacuity, in three parts. The walk must have reached the file the
    // previous sweep's scope missed, and the file the original report named;
    // and the pattern must still match the narration, or it has stopped
    // describing anything and its silence means nothing.
    assert!(
        saw_catalogue,
        "the walk never reached book/po/ja.po, which is the file the earlier \
         sweep's scope missed; this guard would then be silent for the same \
         reason that one was"
    );
    assert!(
        saw_grammar,
        "the walk never reached docs/spec/GRAMMAR.md, the site the original \
         defect report named"
    );
    assert!(
        !allowed.is_empty(),
        "no file under docs/process/ states the superseded push order, so the \
         pattern matches nothing anywhere and a clean result proves nothing"
    );

    assert!(
        offenders.is_empty(),
        "these sites state the checked-arithmetic PUSH order backwards: {offenders:?}\n\
         The runtime pushes low, then high, then the flag. A site saying the \
         reverse about the ARM BINDING is correct and is not what this matches, \
         because a binding site binds two halves and no flag."
    );
}

#[test]
fn the_authoritative_sites_still_state_the_push_order() {
    let root = repo_root();
    let right = right_push_order();

    // Bidirectional: the guard above catches a REINTRODUCED wrong claim, and
    // this one catches the claim being deleted rather than corrected. Deletion
    // would satisfy the first test perfectly.
    for rel in [
        "docs/spec/GRAMMAR.md",
        "book/src/BIG_NUMBERS.md",
        "book/src/INSTRUCTION_SET.md",
        "book/po/ja.po",
    ] {
        let text = fs::read_to_string(root.join(rel))
            .unwrap_or_else(|e| panic!("{rel} must be readable: {e}"));
        assert!(
            text.contains(&right),
            "{rel} no longer states the checked-arithmetic push order at all. \
             Correcting a wrong claim by deleting it leaves a reader with \
             nothing, and passes the backwards-claim guard."
        );
    }

    // The Japanese catalogue must carry the order inside the TRANSLATION of the
    // entry that states it, not merely somewhere in the file. Without this, a
    // stale msgstr beside a corrected msgid reads as clean.
    //
    // **THE FIRST VERSION OF THIS CLAUSE COULD NOT FAIL**, and a mutation found
    // that rather than a reading. It asked whether ANY line held both the order
    // and the Japanese for "push", which the `INSTRUCTION_SET.md` entries
    // satisfy on their own: emptying the `BIG_NUMBERS.md` translation left the
    // check green. A check satisfied by a different entry from the one it is
    // about is not a check. It is scoped to the entry by its source reference.
    let ja = fs::read_to_string(root.join("book/po/ja.po")).expect("ja.po readable");
    let entry = ja
        .split("\n\n")
        .find(|e| e.contains("#: src/BIG_NUMBERS.md:19"))
        .expect("book/po/ja.po carries an entry for the push-order paragraph");
    let msgstr = entry
        .split_once("\nmsgstr")
        .expect("the entry has a msgstr")
        .1;
    assert!(
        msgstr.contains(&right),
        "the Japanese translation of the push-order paragraph does not state the \
         order. The Japanese book is built from the translations, so a corrected \
         msgid beside a stale or empty msgstr still fails its reader.\n\
         msgstr was: {msgstr}"
    );
}
