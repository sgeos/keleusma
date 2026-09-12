//! **AN ABSORPTION PREDICTION MUST CARRY THE TREE IT WAS COMPUTED AGAINST.**
//!
//! # The failure this exists after
//!
//! Absorption 59's brief predicted **zero conflicting files**. `merge-tree`
//! computed **one**, in output printed by the same command that filed the brief.
//!
//! **The prediction was stale, not mistaken.** Its `merge-tree` had run an
//! iteration earlier against a FOUR-commit backlog; by the time it was filed the
//! backlog was seven and both lines had rewritten `REVERSE_PROMPT.md` in between.
//!
//! **That is this session's recurring class arriving inside the discipline meant
//! to catch it** — a figure separated from the measurement that produced it. The
//! bounds-transfer numbers, a repaired unsoundness still asserted, a "none acted
//! on" claim resting on recollection, and now a prediction.
//!
//! # What this checks, and what it deliberately does not
//!
//! **Only the NEWEST absorption brief.** What matters is the next prediction; a
//! guard over all of them would rot into a list, and this package has already
//! rejected one widening for that reason.
//!
//! **It does not demand the stamp from briefs written before the rule.** Fifty-nine
//! predate it. A guard that failed against documents authored before its rule
//! would be disabled within the day.
//!
//! **It does not claim the recorded figures are RIGHT.** It cannot know the true
//! backlog. It requires only that one was recorded, which is the whole difference
//! between a prediction that can go stale unnoticed and one that cannot.

use std::path::PathBuf;

/// The highest-numbered absorption brief, which is the one whose stamp matters.
fn newest_absorption_brief() -> Option<(usize, PathBuf, String)> {
    let dir = std::path::Path::new("../docs/decisions");
    let mut best: Option<(usize, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()? {
        let path = entry.ok()?.path();
        let name = path.file_name()?.to_string_lossy().into_owned();
        let n = name
            .strip_prefix("ABSORPTION_")
            .and_then(|r| r.split('_').next())
            .and_then(|d| d.parse::<usize>().ok());
        if let Some(n) = n
            && name.ends_with("_BRIEF.md")
            && best.as_ref().is_none_or(|(b, _)| n > *b)
        {
            best = Some((n, path));
        }
    }
    let (n, path) = best?;
    let text = std::fs::read_to_string(&path).ok()?;
    Some((n, path, text))
}

/// The absorption at which the stamping rule began.
const RULE_STARTS_AT: usize = 59;

#[test]
fn the_newest_absorption_prediction_records_its_tree() {
    let (n, path, text) = newest_absorption_brief()
        .expect("no absorption brief found under docs/decisions; this guard is reading nothing");

    // **NON-VACUITY.** A directory scan that matched nothing would pass silently,
    // which is the shape of check this line refuses.
    assert!(
        n >= 4,
        "the newest absorption brief found is {n}, which is older than the earliest \
         this line recorded. The scan is not reading the decisions directory."
    );

    if n < RULE_STARTS_AT {
        println!(
            "  the newest absorption brief is {n}, before the rule began at \
             {RULE_STARTS_AT}; nothing to require"
        );
        return;
    }

    let has_commit = text.contains("Computed against");
    let has_backlog = text.to_lowercase().contains("unabsorbed");
    assert!(
        has_commit && has_backlog,
        "{} states no tree for its prediction. A prediction INHERITS THE TREE IT WAS \
         COMPUTED AGAINST: absorption 59's predicted zero conflicts while `merge-tree` \
         computed one, because its figure came from a run against a four-commit \
         backlog and nothing recorded which tree it belonged to. Record the \
         unabsorbed count and the commit.",
        path.display()
    );
}
