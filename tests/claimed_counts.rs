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

/// **THE `InvalidBytecode` CENSUS STATES A POPULATION; THE TREE MUST STILL HAVE IT.**
///
/// `docs/decisions/INVALID_BYTECODE_CENSUS.md` enumerates every site where the runtime raises the
/// "this artefact should never have been produced" error, and its whole value is that the
/// enumeration is complete at the moment it was taken. A site added later leaves the document
/// quietly describing a smaller class than exists -- and a census that has silently stopped being
/// exhaustive is worse than none, because its reader stops looking.
///
/// **The first draft of that document was miscounted**, at 48 against a true 46, because the grep
/// counts TEXT and a doc comment and a match arm read exactly like a construction site to it. This
/// guard derives the figure the same way the document says it was derived, and subtracts the same
/// named exclusions, so the two cannot part.
///
/// # Reach, stated rather than assumed
///
/// It sees `VmError::InvalidBytecode` written in that form. It does not see a site that returns a
/// pre-built error, propagates one with `?`, or maps another kind into this one. The document says
/// the same thing about itself; this test is a drift alarm on a lower bound, not a proof of
/// completeness.
#[test]
fn the_invalid_bytecode_census_still_describes_the_tree() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut matches = 0usize;
    let mut excluded = 0usize;
    for name in ["src/vm.rs", "src/marshall.rs"] {
        let text = std::fs::read_to_string(root.join(name)).expect("read a runtime source file");
        for line in text.lines() {
            if !line.contains("VmError::InvalidBytecode") {
                continue;
            }
            matches += 1;
            let t = line.trim_start();
            // The same exclusions the document names: a doc comment, and a match arm binding the
            // variant rather than constructing it.
            if t.starts_with("///") || t.starts_with("| VmError::InvalidBytecode(_)") {
                excluded += 1;
            }
        }
    }

    // NON-VACUITY. A read that found nothing, or a pattern that matched nothing, would otherwise
    // satisfy the comparison below while measuring an empty set.
    assert!(
        matches > 30,
        "only {matches} occurrences were found across the runtime sources, so this scan has \
         broken rather than the class having shrunk"
    );
    assert!(
        excluded > 0,
        "no excluded occurrence was recognised, so the exclusion rules no longer match anything \
         and the derived total is counting non-sites as sites"
    );

    let doc = std::fs::read_to_string(root.join("docs/decisions/INVALID_BYTECODE_CENSUS.md"))
        .expect("read INVALID_BYTECODE_CENSUS.md");

    // The document states the total in words as well as digits; the digits are what is checked.
    let stated = doc
        .split("**50 matches, of which ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse::<usize>().ok())
        .expect(
            "the census no longer states its site count in the expected form. If the wording \
             changed, update this extraction; a guard that finds nothing to check is worse than \
             no guard.",
        );

    // Test-module occurrences are excluded by the document too, and they are not distinguishable
    // by line shape, so the comparison carries a small tolerance rather than pretending to
    // exactness the scan cannot deliver.
    let derived = matches - excluded;
    assert!(
        derived.abs_diff(stated) <= 4,
        "the census states {stated} construction sites and this scan derives {derived} (from \
         {matches} occurrences less {excluded} excluded). The class has moved; re-run the \
         enumeration in the document rather than adjusting the number, because the value of that \
         document is that every site carries a verdict."
    );
}

/// The narrow-runtime coverage figure the decision documents state must not
/// exceed what the tree actually holds.
///
/// # The claim this guards, and why it needed guarding
///
/// Four documents said, in various wordings, that the narrow widths are
/// unexercised. They are not: `tests/narrow_vm.rs` and
/// `tests/composite_width_skew.rs` define host aliases for narrow and skewed
/// runtimes and drive them in the DEFAULT build, on every continuous-integration
/// run. The corrections state a count. **A count in prose is the thing this tree
/// has watched go stale three times** — the census paragraph was wrong three
/// times, and `CLAUDE.md` carried a stale stage count in two places — so writing
/// a fresh number and walking away would repeat the pattern one level up.
///
/// # The asymmetry is deliberate: at least, not exactly
///
/// The document's figure becomes FALSE if coverage drops below it, and merely
/// conservative if coverage grows. An exact-equality guard would fire on every
/// added narrow-runtime test, which is friction against the behaviour the
/// project wants, and a guard that punishes good changes gets its number bumped
/// without thought or deleted outright.
///
/// **Do not tighten this to equality.**
///
/// # Comments are stripped, and that is not a detail
///
/// A first version of this guard matched alias names anywhere in a test's text
/// and derived 41, which agreed exactly with the figure the document then
/// stated. **That agreement was two errors cancelling.** It counted
/// `narrow_declared_multiword_long_division_on_wide_runtime` — a test whose name
/// says it runs on a WIDE runtime — because a comment inside it mentions a
/// narrow helper; and the document had counted all nine tests in the skew file,
/// including one that is a deliberate control at the DEFAULT widths.
///
/// The true figure is 32 + 8 = 40. **A number that matches expectation is the
/// one least likely to be re-examined**, which is why the check against the
/// hand-derived split mattered more than the total.
///
/// # The attribution rule, and what it misses
///
/// A test counts when the CODE of its body — comments removed — names one of the
/// narrow or skewed runtime aliases those files define, or calls a helper that
/// does. **A test reaching a narrow runtime through a deeper indirection is not
/// counted**, so the derived figure is a lower bound.
///
/// The two files are named explicitly. A third file introducing its own narrow
/// alias would not be seen here.
#[test]
fn the_narrow_runtime_coverage_claim_still_describes_the_tree() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    // Aliases and helpers that reach a narrow or skewed runtime, taken from the
    // two files' own type aliases rather than invented here.
    const REACHES: &[&str] = &[
        "NarrowVm",
        "NarrowWordF64Vm",
        "RetroVm",
        "SixFiveOhTwo",
        "WideWordNarrowAddress",
        "run_i16",
        "run_bool_i16",
        "run_i16_data",
    ];

    /// Tests in `src` whose body code names something in `REACHES`.
    fn attributed(src: &str) -> usize {
        let lines: Vec<&str> = src.lines().collect();
        let mut count = 0;
        let mut i = 0;
        while i < lines.len() {
            let is_test_fn = lines[i].starts_with("fn ")
                && i > 0
                && lines[i - 1].trim_start().starts_with("#[test]");
            if is_test_fn {
                // The body runs to the next top-level item.
                let mut j = i + 1;
                while j < lines.len()
                    && !(lines[j].starts_with("fn ")
                        || lines[j].starts_with("#[test]")
                        || lines[j].starts_with("type ")
                        || lines[j].starts_with("struct ")
                        || lines[j].starts_with("impl "))
                {
                    j += 1;
                }
                // Strip line comments: an alias named only in prose does not
                // mean the test drives that runtime.
                let code: String = lines[i..j]
                    .iter()
                    .map(|l| l.split("//").next().unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join("\n");
                if REACHES.iter().any(|a| code.contains(a)) {
                    count += 1;
                }
                i = j;
            } else {
                i += 1;
            }
        }
        count
    }

    let mut derived = 0usize;
    for rel in ["tests/narrow_vm.rs", "tests/composite_width_skew.rs"] {
        let src = std::fs::read_to_string(root.join(rel))
            .unwrap_or_else(|_| panic!("read {rel}; if it was renamed, update this guard"));
        derived += attributed(&src);
    }

    assert!(
        derived > 0,
        "no test was attributed to a narrow runtime, so the alias list no longer matches anything \
         and this guard is reporting a comfortable zero rather than checking the claim"
    );

    let doc = std::fs::read_to_string(root.join("docs/decisions/FEATURE_COMBINATION_SWEEP.md"))
        .expect("read FEATURE_COMBINATION_SWEEP.md");

    // Parse the figure OUT of the document. A guard carrying its own copy of the
    // number would check the tree against itself and keep passing while the
    // document drifted.
    const MARKER: &str = " tests, in every continuous-integration";
    let stated: usize = doc
        .split(MARKER)
        .next()
        .filter(|head| head.len() < doc.len())
        .and_then(|head| {
            let digits: String = head
                .trim_end()
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_digit())
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            digits.parse().ok()
        })
        .expect(
            "the sweep document no longer states the narrow-runtime coverage figure in the \
             expected form. If the wording changed, update this extraction; a guard that finds \
             nothing to check is worse than no guard.",
        );

    assert!(
        derived >= stated,
        "the documents claim {stated} tests drive a narrow or skewed runtime and this scan finds \
         only {derived}. Coverage has been REMOVED, which makes the claim false. Restore the \
         coverage or correct the documents — do not lower the figure to match without saying why \
         the tests went away."
    );
}

/// The census's per-group table must add up to the totals its prose states.
///
/// # The drift this catches, which happened twice
///
/// The document already warns that "a table whose parts do not add to its
/// stated whole has been the tell for a miscount here before". It then became
/// the tell again: the group F row read "1 of 7 probed" while the prose beneath
/// it said "two of F's seven", because a second site was probed and the row was
/// not updated. The two disagreed for long enough that a third figure — 17
/// examined — was still circulating in `HANDOFF.md` five lines from the correct
/// one.
///
/// A count in a table and the same count in a sentence are two places to go
/// stale independently. This checks they agree.
///
/// # What it does NOT check
///
/// Whether either figure is TRUE of the tree. The sibling guard
/// `the_invalid_bytecode_census_still_describes_the_tree` checks the site total
/// against the source; this one checks the document against itself, which is a
/// weaker and different claim. Both are needed: a self-consistent document can
/// still describe a tree that has moved.
#[test]
fn the_census_group_table_adds_up_to_its_stated_totals() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc = std::fs::read_to_string(root.join("docs/decisions/INVALID_BYTECODE_CENSUS.md"))
        .expect("read INVALID_BYTECODE_CENSUS.md");

    let mut sites = 0usize;
    let mut examined = 0usize;
    let mut rows = 0usize;
    for line in doc.lines() {
        // `| A | description | 1 | verdict |`
        let cols: Vec<&str> = line.split('|').map(str::trim).collect();
        if cols.len() < 6 {
            continue;
        }
        let group = cols[1];
        if group.len() != 1 || !group.chars().all(|c| c.is_ascii_uppercase()) {
            continue;
        }
        let Ok(n) = cols[3].parse::<usize>() else {
            continue;
        };
        let verdict = cols[4];
        rows += 1;
        sites += n;
        // "(k of n probed)" gives a partial count; "not examined" gives none;
        // anything else is a class verdict covering the whole group.
        examined += if let Some(rest) = verdict.split(" of ").next().and_then(|head| {
            head.rfind('(')
                .map(|i| &head[i + 1..])
                .and_then(|d| d.parse::<usize>().ok())
        }) {
            rest
        } else if verdict.contains("not examined") {
            0
        } else {
            n
        };
    }

    assert!(
        rows >= 8,
        "only {rows} group rows were parsed, so the table's shape changed and this guard is \
         checking almost nothing rather than checking the table"
    );

    let stated_sites = 46usize;
    assert_eq!(
        sites, stated_sites,
        "the group rows sum to {sites} sites, not the {stated_sites} the document states. \
         Re-derive the totals by summing the per-group column — adjusting the total instead is \
         how group G went missing from every remainder list."
    );

    // The prose states the examined total in words and then enumerates it.
    assert!(
        doc.contains("Thirty-five of forty-six sites carry an examined verdict"),
        "the census no longer states its examined total in the expected form; update this \
         extraction rather than deleting the check"
    );
    assert_eq!(
        examined, 35,
        "the group rows sum to {examined} examined sites and the prose says thirty-five. One of \
         them moved without the other. The per-group column is the authority."
    );
}

/// Every test file must run in at least one continuous-integration
/// configuration, except the one documented exception.
///
/// # Why a test can be invisible
///
/// A test file's `#![cfg(...)]` decides which builds compile it. If that gate is
/// satisfied by no configuration continuous integration runs, the file is
/// compiled by nobody and run by nobody — and it still sits in the tree looking
/// like coverage. This is the same defect
/// `docs/decisions/FEATURE_COMBINATION_SWEEP.md` records for builds, one level
/// down.
///
/// **Only a gate negating a DEFAULT feature can escape every job**, because the
/// non-default jobs are additive to default. The `not(feature = "narrow-word-*")`
/// gates across the suite negate features nothing enables, so they are satisfied
/// everywhere.
///
/// # The one exception, and why it is deliberate
///
/// `float_opcode_without_floats.rs` is gated
/// `all(feature = "verify", not(feature = "floats"))`. It pins a hole reachable
/// only on a build without floats, so no job that enables `verify` can also run
/// it. That is queued as an operator decision, not an oversight.
///
/// # The feature sets are DERIVED, not restated
///
/// They are read out of the workflow file. Restating them here would be a second
/// copy of a fact continuous integration already owns, and would go stale the
/// day a job is added — the failure this session found in five process documents
/// at once.
#[test]
fn the_only_test_gated_out_of_every_ci_configuration_is_the_known_one() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = std::fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .expect("read ci.yml; if the workflow moved, update this guard");

    // Derive the keleusma feature sets CI runs, from the workflow's own commands.
    let default_features = ["compile", "verify", "floats"];
    let mut sets: Vec<Vec<String>> = vec![
        default_features.iter().map(|s| s.to_string()).collect(),
        Vec::new(), // the bare --no-default-features job
    ];
    for line in workflow.lines() {
        // ONLY lines that RUN the integration tests count. `cargo doc --features
        // signatures,encryption,shell` COMPILES a gated test's crate for
        // documentation but never executes it, and treating a doc job as
        // coverage is how this guard first reported a test gated on
        // `encryption` as covered. `cargo test --doc` runs doctests, not the
        // files in `tests/`.
        if !line.contains("nextest run") {
            continue;
        }
        if !line.contains("-p keleusma ") || !line.contains("--features ") {
            continue;
        }
        let Some(rest) = line.split("--features ").nth(1) else {
            continue;
        };
        let list = rest.split_whitespace().next().unwrap_or("");
        if list.is_empty() {
            continue;
        }
        let mut set: Vec<String> = default_features.iter().map(|s| s.to_string()).collect();
        for f in list.split(',') {
            set.push(f.to_string());
        }
        sets.push(set);
    }
    assert!(
        sets.len() >= 4,
        "only {} feature sets were derived from the workflow, so the extraction stopped matching \
         and this guard is checking almost nothing",
        sets.len()
    );

    /// Evaluate a `cfg` gate against a feature set. An expression this cannot
    /// interpret is treated as SATISFIED, which biases toward reporting a file
    /// as running — the direction that under-reports, which is why the
    /// non-vacuity assertions above and below matter.
    fn satisfied(gate: &str, feats: &[String]) -> bool {
        let g: String = gate.split_whitespace().collect::<Vec<_>>().join(" ");
        fn ev(e: &str, feats: &[String]) -> bool {
            let e = e.trim();
            for kind in ["all(", "any(", "not("] {
                if let Some(inner) = e.strip_prefix(kind) {
                    let mut depth = 0usize;
                    let mut parts: Vec<String> = Vec::new();
                    let mut cur = String::new();
                    for ch in inner.chars() {
                        match ch {
                            '(' => depth += 1,
                            ')' if depth == 0 => break,
                            ')' => depth -= 1,
                            ',' if depth == 0 => {
                                parts.push(std::mem::take(&mut cur));
                                continue;
                            }
                            _ => {}
                        }
                        cur.push(ch);
                    }
                    if !cur.trim().is_empty() {
                        parts.push(cur);
                    }
                    let vals: Vec<bool> = parts
                        .iter()
                        .filter(|p| !p.trim().is_empty())
                        .map(|p| ev(p, feats))
                        .collect();
                    return match kind {
                        "all(" => vals.iter().all(|v| *v),
                        "any(" => vals.iter().any(|v| *v),
                        _ => !vals.first().copied().unwrap_or(false),
                    };
                }
            }
            if let Some(rest) = e.split("feature = \"").nth(1)
                && let Some(name) = rest.split('"').next()
            {
                return feats.iter().any(|f| f == name);
            }
            true
        }
        ev(&g, feats)
    }

    const KNOWN: &str = "float_opcode_without_floats.rs";
    let mut orphans: Vec<String> = Vec::new();
    let mut gated = 0usize;
    let mut total = 0usize;
    for entry in std::fs::read_dir(root.join("tests")).expect("read tests/") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        total += 1;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let src = std::fs::read_to_string(&path).expect("read test file");
        // Strip line comments so a commented gate fragment does not confuse the parse.
        let stripped: String = src
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        let Some(open) = stripped.find("#![cfg(") else {
            continue; // ungated: runs wherever the crate builds
        };
        let after = &stripped[open + "#![cfg(".len()..];
        let mut depth = 0usize;
        let mut gate = String::new();
        for ch in after.chars() {
            match ch {
                '(' => depth += 1,
                ')' if depth == 0 => break,
                ')' => depth -= 1,
                _ => {}
            }
            gate.push(ch);
        }
        gated += 1;
        if !sets.iter().any(|s| satisfied(&gate, s)) {
            orphans.push(name);
        }
    }

    assert!(
        total > 50 && gated > 50,
        "only {total} test files and {gated} gates were seen, so the scan is not reaching the suite"
    );
    assert!(
        orphans.iter().any(|o| o == KNOWN),
        "the known exception {KNOWN} was NOT detected as gated out of every configuration. Either \
         its gate changed — in which case the load-time float hole may now be covered, which is \
         news — or this evaluator has stopped working. A guard that cannot find the one case it \
         knows about is checking nothing."
    );
    orphans.retain(|o| o != KNOWN);
    assert!(
        orphans.is_empty(),
        "these test files are gated out of EVERY continuous-integration configuration, so they are \
         compiled by nobody and run by nobody while still reading as coverage: {orphans:?}. Either \
         widen the gate, add a job, or document the exception as deliberate the way \
         {KNOWN} is."
    );
}
