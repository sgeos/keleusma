//! **A PER-LINE STATE FILE STAYS WITHIN THE BUDGET IT DECLARES.**
//!
//! A state file under `docs/process/handoffs/` is overwritten whole on every update and
//! holds only what is in flight, so that a session resuming after compaction can read it
//! cheaply. The kaizen line's file states that it stays "within a budget of 12 KB", and it
//! reached 11,974 bytes on 2026-11-01 with only a hand count to notice. Nothing failed when
//! a rewrite forgot to drop an item. This test makes the stated budget a checked one.
//!
//! # What it checks, and what it leaves alone
//!
//! A file opts in by stating its budget in the words `within a budget of N KB`, read as
//! N thousand bytes, the reading the kaizen line has kept since it adopted the budget. A
//! file without that phrase is not checked, because the other handoffs are append-style
//! records with no budget, and pinning one on them here would be a policy this test has
//! no authority to set. When a line converts its file to a state file, it adds the phrase
//! and the guard starts to apply.
//!
//! # Reach
//!
//! A pattern that matched nothing would pass while checking nothing, so the test also
//! requires that the kaizen line's file declares a budget and that the parser reads the
//! phrase from a known sentence. The kaizen file is named because it is the one file known
//! to opt in, not because the budget belongs to it alone.

use std::fs;
use std::path::{Path, PathBuf};

const HANDOFFS: &str = "docs/process/handoffs";
const PHRASE: &str = "within a budget of ";

/// The budget in bytes a state file declares, or `None` when it declares none.
fn declared_budget(text: &str) -> Option<usize> {
    let start = text.find(PHRASE)? + PHRASE.len();
    let rest = &text[start..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    let after = rest[digits.len()..].trim_start();
    if digits.is_empty() || !after.starts_with("KB") {
        return None;
    }
    digits.parse::<usize>().ok()?.checked_mul(1000)
}

fn handoff_files() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(HANDOFFS);
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    files.sort();
    files
}

#[test]
fn every_declared_state_file_budget_holds() {
    let mut checked = Vec::new();
    let mut over = Vec::new();
    for path in handoff_files() {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        if let Some(budget) = declared_budget(&text) {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if text.len() > budget {
                over.push(format!(
                    "{name} is {} bytes, over its budget of {budget}",
                    text.len()
                ));
            }
            checked.push(name);
        }
    }
    assert!(
        checked.iter().any(|name| name == "kaizen.md"),
        "kaizen.md no longer declares a budget, so this guard may be checking nothing. \
         Files that declare one: {checked:?}"
    );
    assert!(over.is_empty(), "state files over budget: {over:?}");
}

#[test]
fn the_budget_phrase_is_read_and_a_near_miss_is_not() {
    assert_eq!(
        declared_budget("holds only what is in flight, within a budget of 12 KB."),
        Some(12_000)
    );
    assert_eq!(declared_budget("within a budget of 8KB"), Some(8_000));
    assert_eq!(declared_budget("within a budget of twelve KB"), None);
    assert_eq!(declared_budget("within a budget of 12 MB"), None);
    assert_eq!(declared_budget("no budget is stated here"), None);
}
