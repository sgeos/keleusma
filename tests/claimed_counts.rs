//! **THE PROJECT INSTRUCTIONS STATED TEST COUNTS THAT WERE THREE TIMES WRONG.**
//!
//! `CLAUDE.md` is the document an agent reads first. In two places it said:
//!
//! > Approximately 1168 keleusma lib tests plus **368 integration tests across 30 files** ... 42
//! > keleusma-arena, and 6 keleusma-bench tests
//!
//! Measured 2026-08-28: **1263** lib tests, **1192** integration `#[test]` functions across
//! **89** files, **59** arena, 6 bench. The file count was three times wrong and the integration
//! count more than three times. "Approximately" cannot carry that.
//!
//! **THOSE TWO FIGURES WERE TAKEN BEFORE THIS FILE EXISTED, AND `CLAUDE.md` STATES 1194 ACROSS 90
//! FOR THE SAME DAY.** The difference is exactly this file: 89 + 1 = 90 and 1192 + 2 = 1194. Both
//! numbers are right for their moment and neither said which moment that was, so a reader
//! comparing them finds two measurements of "the same thing" that disagree.
//!
//! **A COUNT THAT DOES NOT NAME ITS POPULATION IS THE DEFECT THIS FILE EXISTS TO CATCH**, and the
//! file had it in its own header. Recorded rather than quietly reconciled, because the instinct on
//! finding two numbers is to pick one — and here both were correct.
//!
//! # Why it mattered operationally, which is why this is a test and not a tidy-up
//!
//! In the session that found it, a killed test sweep reported **55 binaries green while 31 never
//! ran**, and the gap was caught only by enumerating the files. **An agent calibrated on "30
//! files" would have read 55 as comfortably complete.** A stale count in the orientation document
//! is a wrong prior for every coverage judgement made against it.
//!
//! # How it was found
//!
//! By generalising the previous increment: having corrected the shipped-example index, ask whether
//! any OTHER documentation makes claims nothing checks. This was the largest instance.
//!
//! # THE TOLERANCE IS DELIBERATE
//!
//! An exact pin would fail on every increment that adds a test, become a nuisance, and be deleted.
//! **Gross drift is the defect, not movement.** The bounds here pass at today's figures and fail by
//! a wide margin on the text they replaced, which is the demonstration that they can fire.
//!
//! # What this does NOT check, said plainly
//!
//! The lib, arena and bench figures are RUN counts — what `cargo test` reports — and a test cannot
//! cheaply re-run cargo to confirm them. Only the two statically derivable figures are checked
//! here. Those are also the two that were wrong, but that is luck rather than design, and the
//! others remain unguarded.

#![cfg(feature = "compile")]

const INSTRUCTIONS: &str = include_str!("../CLAUDE.md");

/// Every integer that appears in the document immediately before a given phrase.
fn figures_before(phrase: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = INSTRUCTIONS[from..].find(phrase) {
        let at = from + rel;
        let head = &INSTRUCTIONS[..at];
        let digits: String = head
            .chars()
            .rev()
            .skip_while(|c| c.is_whitespace())
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(n) = digits.chars().rev().collect::<String>().parse::<usize>() {
            out.push(n);
        }
        from = at + phrase.len();
    }
    out
}

/// **THE STATED INTEGRATION FIGURES ARE NOT GROSSLY WRONG.**
///
/// Tolerant of the movement every increment causes, intolerant of the drift that made the document
/// mislead. Both figures are derived from the tree rather than restated.
#[test]
fn the_instructions_do_not_misstate_the_integration_test_counts() {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests"));
    let mut files = 0usize;
    let mut tests = 0usize;
    for entry in std::fs::read_dir(dir).expect("the integration test directory") {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_some_and(|x| x == "rs") {
            files += 1;
            let src = std::fs::read_to_string(&path).expect("read an integration test file");
            tests += src
                .lines()
                .filter(|l| l.trim_start().starts_with("#[test]"))
                .count();
        }
    }

    // NON-VACUITY on the measurement. A directory read that found nothing would make every
    // comparison below vacuously satisfiable.
    assert!(
        files >= 50 && tests >= 500,
        "the measurement found {files} files and {tests} tests, so it has broken rather than the \
         suite having shrunk"
    );

    let claimed_files = figures_before(" files (`ls tests/*.rs");
    let claimed_tests = figures_before(" integration `#[test]` functions across");

    // NON-VACUITY on the extraction. If the document is reworded so these phrases vanish, the
    // guard must fail rather than silently stop checking -- the exact failure it exists to prevent.
    assert!(
        !claimed_files.is_empty() && !claimed_tests.is_empty(),
        "no stated figures were found in the instructions. If the wording changed, update this \
         extraction; a guard that finds nothing to check is worse than no guard."
    );

    for c in &claimed_files {
        let diff = c.abs_diff(files);
        assert!(
            diff <= 10,
            "the instructions state {c} integration test files and the tree has {files}. Movement \
             is expected; a gap this size means the figure has stopped describing the tree, which \
             is how an agent comes to read a partial test run as complete coverage."
        );
    }
    for c in &claimed_tests {
        let allowed = tests / 5; // twenty per cent
        assert!(
            c.abs_diff(tests) <= allowed,
            "the instructions state {c} integration tests and the tree has {tests}, outside the \
             {allowed}-test tolerance."
        );
    }
}

/// **THE FIGURES CARRY THE MEANS OF RE-DERIVING THEM.**
///
/// The defect was not only that the numbers were wrong but that nothing told a reader how to check.
/// This repository's handoff already states moving numbers as dated measurements with their
/// derivation command; the instructions now do the same, and this keeps that property.
#[test]
fn the_stated_figures_say_how_they_were_measured() {
    let mentions = INSTRUCTIONS
        .matches("integration `#[test]` functions across")
        .count();
    assert!(
        mentions >= 1,
        "non-vacuity: the instructions no longer state an integration figure in the expected form"
    );
    assert!(
        INSTRUCTIONS.contains("ls tests/*.rs"),
        "the stated file count no longer carries the command that derives it, so a reader cannot \
         check it and it will drift again"
    );
    assert!(
        INSTRUCTIONS.contains("cargo test --lib --features self-host"),
        "the stated lib-test count no longer says how it was measured. It is a RUN count, and a \
         reader comparing it against a grep for `#[test]` would get a different number and think \
         the document wrong."
    );
    assert!(
        INSTRUCTIONS.contains("Measured 2026-08-28"),
        "the stated figures no longer carry the date they were measured, so a reader cannot tell \
         how far they may have drifted"
    );
}
