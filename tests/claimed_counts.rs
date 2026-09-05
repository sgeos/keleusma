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
//!
//! # THAT CAVEAT WAS RIGHT, AND A DEFECT WAS FOUND IN EXACTLY THE REGION IT NAMED
//!
//! The paragraph above said the remaining claims were unguarded. On 2026-08-30 one of them was
//! **wrong in two places**: `CLAUDE.md` described `src/selfhost/kel/` as holding **ten** stage
//! sources, where the directory holds **twelve** and the rest of the tree — the handoff, the
//! byte-identity corpus, the `CONSTS` claim — consistently says twelve. The stage-source count is
//! now guarded too, by `the_stage_source_count_claim_matches_the_directory`.
//!
//! **THE GUARD DERIVES BOTH SIDES AND PINS NEITHER**, which is the repair for a failure this
//! repository already paid for: a test that scanned a directory while pinning the answer as a
//! constant was wrong on a branch carrying more files. Here the expected value is read from the
//! prose and the actual from the tree, so a line that adds a stage stays green as long as the
//! document is updated with it.

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
    // **A DATE IN THE RIGHT SHAPE, NOT ONE PARTICULAR DATE.**
    //
    // This assertion used to require the literal string `Measured 2026-08-28`, which made the
    // honest act of RE-MEASURING fail the guard. A check that fires on the correct behaviour
    // teaches its reader to weaken it, and a weakened guard is worse than none: the next person
    // to re-derive the figures would have deleted the assertion rather than updated a date they
    // had no reason to think was load-bearing.
    //
    // What the guard actually wants is that the figures carry SOME measurement date, so a reader
    // can judge how far they may have drifted. That is what is checked.
    let dated = INSTRUCTIONS.match_indices("Measured ").any(|(i, _)| {
        let rest = &INSTRUCTIONS[i + "Measured ".len()..];
        let d: Vec<char> = rest.chars().take(10).collect();
        d.len() == 10
            && d[..4].iter().all(char::is_ascii_digit)
            && d[4] == '-'
            && d[5..7].iter().all(char::is_ascii_digit)
            && d[7] == '-'
            && d[8..10].iter().all(char::is_ascii_digit)
    });
    assert!(
        dated,
        "the stated figures no longer carry the date they were measured in the form \
         `Measured YYYY-MM-DD`, so a reader cannot tell how far they may have drifted"
    );
}

/// **THE STAGE-SOURCE COUNT IN THE ORIENTATION DOCUMENT MATCHES THE DIRECTORY.**
///
/// `CLAUDE.md` said **ten** where the tree holds **twelve**, in two separate places, while every
/// other document — the handoff's "all twelve stages", the `CONSTS` claim, the byte-identity
/// corpus's "eleven of twelve" — said twelve. A reader calibrated on ten would have believed the
/// corpus was two stages from complete when it is one.
///
/// # BOTH SIDES ARE DERIVED. NEITHER IS PINNED.
///
/// The expected count is read out of the prose and the actual out of the directory, so this stays
/// correct on a branch that adds a stage, provided the document is updated with it. **That shape
/// is deliberate**: the earlier failure in this family was a test that scanned a directory while
/// pinning its answer as a constant, which made it wrong on another line's branch — right in the
/// direction its own message called a coverage gain.
///
/// # The claim distinguishes two counts and so does this
///
/// Twelve sources exist; **eleven** are embedded in the driver via `include_str!`, and
/// `verify_types.kel` is embedded by its tests instead. Conflating those is how the original
/// wording came to be wrong in a way nobody noticed.
///
/// **THE EMBEDDED COUNT IS OVER DISTINCT NAMES, NOT OCCURRENCES, AND THE FIRST REVISION GOT THAT
/// WRONG.** Counting `include_str!` occurrences reports twelve, because at least one stage is
/// embedded at more than one site. The test failed on that and the instrument was corrected
/// rather than the expectation — the failure was real evidence about the counter, not the tree.
#[test]
fn the_stage_source_count_claim_matches_the_directory() {
    use std::collections::BTreeSet;

    const WORDS: &[(&str, usize)] = &[
        ("nine", 9),
        ("ten", 10),
        ("eleven", 11),
        ("twelve", 12),
        ("thirteen", 13),
        ("fourteen", 14),
    ];

    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/selfhost/kel"));
    let on_disk = std::fs::read_dir(dir)
        .expect("read the stage-source directory")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "kel"))
        .count();
    assert!(
        on_disk >= 5,
        "only {on_disk} stage sources were found, so this guard is looking at the wrong directory"
    );

    let claimed: Vec<usize> = INSTRUCTIONS
        .lines()
        .filter(|l| l.contains("stage sources"))
        .filter_map(|l| {
            WORDS
                .iter()
                .find(|(w, _)| l.contains(&format!("{w} stage sources")))
                .map(|(_, n)| *n)
        })
        .collect();
    assert!(
        !claimed.is_empty(),
        "no line in the instructions states a stage-source count in words, so this guard cannot \
         fire. The claim was rephrased and the check must be rephrased with it."
    );
    for n in &claimed {
        assert_eq!(
            *n, on_disk,
            "the instructions claim {n} stage sources; the tree holds {on_disk}. A reader \
             calibrated on the wrong number mis-judges how close the byte-identity corpus is to \
             complete."
        );
    }

    // DISTINCT stage names, not `include_str!` occurrences: a stage embedded at two sites would
    // otherwise inflate the count to the directory's size and hide the very gap this checks.
    let driver = include_str!("../src/selfhost/mod.rs");
    let embedded: BTreeSet<&str> = driver
        .match_indices("include_str!(\"kel/")
        .filter_map(|(at, pat)| {
            let rest = &driver[at + pat.len()..];
            rest.find(".kel\")").map(|end| &rest[..end])
        })
        .collect();
    assert!(
        embedded.len() < on_disk,
        "the driver embeds all {on_disk} stage sources. The instructions say one is embedded by \
         its tests instead, so that sentence is stale.",
    );
    assert!(
        !embedded.contains("verify_types"),
        "`verify_types.kel` is now embedded in the driver. It is the twelfth stage, the one that \
         does not self-compile, and the instructions describe it as embedded by its tests."
    );
}

/// **EVERY WORKSPACE MEMBER IS NAMED IN THE ORIENTATION DOCUMENT.**
///
/// `CLAUDE.md` listed six members where `Cargo.toml` declares seven: `keleusma-wire-derive`
/// appeared **nowhere in the document at all**, neither in the members sentence nor in the
/// repository tree. A crate an agent has never heard of is a crate it will not think to build,
/// test, or version.
///
/// Both sides are derived — the members from `Cargo.toml`, the mentions from the document — so a
/// line that adds a crate stays green as long as the document is updated with it.
#[test]
fn every_workspace_member_is_named_in_the_instructions() {
    const MANIFEST: &str = include_str!("../Cargo.toml");

    let members: Vec<&str> = MANIFEST
        .split("members")
        .nth(1)
        .and_then(|rest| rest.split('[').nth(1))
        .and_then(|rest| rest.split(']').next())
        .expect("the workspace manifest declares members")
        .split(',')
        .map(|m| m.trim().trim_matches('"'))
        .filter(|m| !m.is_empty())
        .collect();
    assert!(
        members.len() >= 3,
        "only {} workspace member(s) were parsed, so this guard is reading the manifest wrongly \
         rather than measuring the document",
        members.len()
    );

    for m in &members {
        assert!(
            INSTRUCTIONS.contains(m),
            "workspace member `{m}` is named nowhere in the project instructions. An agent \
             orienting from that document does not know the crate exists."
        );
    }
}

/// **THE SOURCE TREE IN THE INSTRUCTIONS DOES NOT CLAIM TO BE COMPLETE UNLESS IT IS.**
///
/// The `src/` block listed sixteen modules and ended with `└──`, which reads as the whole
/// directory — while **eighteen further `src/*.rs` files existed**. The document uses an ellipsis
/// elsewhere for exactly this, under `examples/`, so its absence here was a claim rather than an
/// omission.
///
/// This does not require the listing to be exhaustive. It requires it to be **either exhaustive
/// or marked partial**, which is the distinction that was missing.
#[test]
fn the_source_listing_is_exhaustive_or_marked_partial() {
    let block = INSTRUCTIONS
        .split("├── src/")
        .nth(1)
        .and_then(|rest| rest.split("├── tests/").next())
        .expect("the instructions carry a src/ tree block");

    let marked_partial = block.contains('…') || block.contains("...");
    if marked_partial {
        return;
    }

    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
    let missing: Vec<String> = std::fs::read_dir(dir)
        .expect("read src/")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".rs"))
        .filter(|n| !block.contains(n.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "the src/ listing carries no ellipsis, so it reads as the whole directory, but {} \
         file(s) are absent: {missing:?}. Either list them or mark the block partial.",
        missing.len()
    );
}
