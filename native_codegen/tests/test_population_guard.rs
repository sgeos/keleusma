//! **THE CLAUSE WRITTEN SIX TIMES AND ENFORCED ZERO TIMES.**
//!
//! Every completion condition drafted for this line carried some form of *"no
//! test present before this increment has been deleted or disabled"*. Verifying
//! it for one session took a manual name-diff against that session's first
//! commit: **456 test functions then, 474 now, ten names absent** — each
//! cross-referenced by hand to a documented successor, because they were renamed
//! rather than removed.
//!
//! **The result was clean. The method was not.** Nothing in the tree would have
//! noticed a test quietly disappearing, and the clause would have gone on being
//! asserted every increment.
//!
//! That is the shape this line keeps finding: **a guard present in the prose and
//! absent from the mechanism.** The standing warning to assert a lowered
//! function's parameter count had been written into the handoff and left
//! unenforced in the two files the warning names — where it later turned out a
//! four-parameter function was being called through a one-parameter signature.
//! This is the same failure at the level of acceptance criteria rather than code.
//!
//! # What this guard establishes, and what it CANNOT
//!
//! **It detects CHANGE, not deletion.** A count cannot tell a rename from a
//! delete-plus-add, and no cheap check can. What it can do is refuse to let the
//! population move without someone looking — which is exactly what
//! `pointer_offset_census.rs` did when the spill slice added two sites, and what
//! the mutation table did when a fix left `SetLocal` unbounded.
//!
//! **Pinning the 474 names was considered and rejected**: a table that large is
//! one nobody maintains, and an unmaintained table is worse than a count because
//! it looks like coverage.

/// Backend test functions at the stamp.
///
/// **Derive this, do not transcribe it.** A count in this tree once read
/// "nineteen" while the block it described held twenty-nine.
const RECORDED_TEST_FUNCTIONS: usize = 503;
// 501 -> 503 on 2026-09-11: two added in `host_contract_completeness.rs`. The
// generated header stated the layout of ONE of the three pointers the entry
// takes; the shipped C host sized the other two by eye, and that file is what a
// host programmer copies.
//
// 495 -> 501 on 2026-09-11: six added across two new files. Four in
// `private_init_image.rs`, for a defect the read census found: a private scalar
// slot's declared initializer was never applied natively, because the runtime
// applies `private_init` at load and there is no native load step. Two in
// `memory_read_census.rs`, the third axis — what guarantees the contents of
// memory the emitter reads but did not write.
//
// 494 -> 495 on 2026-09-11: one added,
// `the_published_supplement_covers_every_byte_the_backend_writes`. The
// initialisation words for composite slots are new persistent state, and a figure
// a host must add is one the runtime's sizing does not include. The trap subject
// added in the same increment is a DATA ROW in `corpus_differential.rs`, not a
// test function, so it moves no count -- worth saying, because a reader
// reconciling this number against the increment would otherwise look for it.
//
// 492 -> 494 on 2026-09-11: two added, both in the new
// `value_movement_census.rs`. It is the deliberate instrument for VALUE movement,
// the counterpart to `pointer_offset_census.rs` for addresses: the data-slot
// defect was a value move, and no census looked at those.
//
// 491 -> 492 on 2026-09-11: one added,
// `the_private_contract_exceeds_the_slot_array_for_real_corpus_modules`. It is
// the non-vacuity check on a harness repair: the corpus differential sized its
// private buffer by slot count rather than by the contract the backend
// publishes, and the composite copy was the first lowering to reach the space
// between the two.
//
// 490 -> 491 on 2026-09-11: one added,
// `the_pool_persists_across_a_silent_cycle_and_changes_on_a_writing_one`. It
// exists because auditing the increment's own completion condition found clause
// 2 satisfiable by a pool that is merely never overwritten — a weaker property
// than surviving a reset. The subject writes on some cycles and not others, so
// survival and rewrite fail on different elements of one sequence.
//
// 488 -> 490 on 2026-09-11, net, and the net hides a REPLACEMENT that must be
// stated: `private_slot_composite.rs` went from four names to six. Two were
// REMOVED because their claim was inverted by the same-day fix — the composite
// data slot no longer refuses, so a test asserting the refusal would have been
// kept green by deleting the lowering. Four were added, asserting the copy, its
// survival across a reset, the corpus subject running, and the indexed form
// still refusing. **A count cannot tell a replacement from an addition**, which
// is why it is written here.
//
// 476 -> 488 on 2026-09-11: TWELVE added, NONE removed, and each is accounted
// for rather than absorbed. Six in `reset_region_retention.rs`, four in
// `private_slot_composite.rs`, two in `general_stream_sequence.rs`. The two new
// files exist because the increment found two defects: a local live across a
// suspension was cleared by the entry preamble on every resume, and a composite
// written to a private data slot was stored as the body's ADDRESS.
//
// 475 -> 476 on 2026-09-10: one added,
// `the_corpus_contains_no_general_stream_and_the_control_proves_the_probe_works`.
// Two scaffolding probes written in the same increment were REMOVED, both
// subsumed by that one, so the net is a single name — accounted for here in the
// way the failure message demands rather than absorbed into the figure.
//
// 474 when the figure was first derived from git at an earlier head, plus THIS
// FILE'S OWN test. The guard fired on its own arrival, which is the first
// evidence that it fires at all — and the update is accountable in the way the
// failure message demands: the one added name is named here.

/// Backend test binaries at the stamp.
///
/// 104 -> 106 on 2026-09-11. Both additions are named above, and `git status`
/// showed no deletion under `tests/` in the same increment — which is the check
/// the failure message asks for, since a count cannot tell an add from a
/// delete-plus-add. Unchanged by the persistent-composite-copy increment that
/// followed: it rewrote a file's contents without adding or removing one.
///
/// 106 -> 107 on 2026-09-11: `value_movement_census.rs` added, none removed.
///
/// 107 -> 109 later the same day: `private_init_image.rs` and
/// `memory_read_census.rs` added, none removed.
///
/// 109 -> 110: `host_contract_completeness.rs` added, none removed.
const RECORDED_TEST_FILES: usize = 110;

fn test_files() -> Vec<std::path::PathBuf> {
    let mut out: Vec<_> = std::fs::read_dir("tests")
        .expect("the tests directory is readable")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    out.sort();
    out
}

/// Every `#[test]` function name across the backend's test files.
fn test_function_names() -> Vec<String> {
    let mut names = Vec::new();
    for path in test_files() {
        let src = std::fs::read_to_string(&path).expect("a test file is readable");
        let lines: Vec<&str> = src.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            if l.trim() != "#[test]" {
                continue;
            }
            // The `fn` may be a line or two below, past attributes such as
            // `#[should_panic]` or a doc comment continuation.
            for probe in lines.iter().skip(i + 1).take(4) {
                if let Some(rest) = probe.trim().strip_prefix("fn ")
                    && let Some(name) = rest.split('(').next()
                {
                    names.push(name.trim().to_string());
                    break;
                }
            }
        }
    }
    names.sort();
    names
}

#[test]
fn the_backend_test_population_has_not_moved_unnoticed() {
    let names = test_function_names();
    let files = test_files();

    println!("\n================ BACKEND TEST POPULATION");
    println!("  test files     : {}", files.len());
    println!("  test functions : {}", names.len());
    println!(
        "\n  THIS DETECTS CHANGE, NOT DELETION. A count cannot tell a rename from\n  \
         a delete-plus-add. It refuses to let the population move without\n  \
         someone looking, which is all it claims.\n================\n"
    );

    // **NON-VACUITY.** A matcher that found nothing would pass for ever, and
    // this file's whole subject is a population.
    assert!(
        names.len() > 100,
        "only {} test functions were found across {} files, so the matcher has \
         gone stale and this guard is measuring nothing",
        names.len(),
        files.len()
    );
    // It must find ITSELF, or it is not reading the directory it claims to.
    assert!(
        names
            .iter()
            .any(|n| n == "the_backend_test_population_has_not_moved_unnoticed"),
        "the guard cannot see its own test function, so it is not reading the \
         files it reports on"
    );

    assert_eq!(
        files.len(),
        RECORDED_TEST_FILES,
        "the number of backend test FILES changed. Update the figure only after \
         establishing that no file was removed."
    );
    assert_eq!(
        names.len(),
        RECORDED_TEST_FUNCTIONS,
        "THE BACKEND TEST POPULATION CHANGED. Do not simply update the number.\n\
         \n\
         Establish that nothing was LOST: diff the test-function names against an \
         earlier commit, and for every name that disappeared, name the successor \
         that covers its subject. Ten disappeared in one session and all ten were \
         deliberate renames with the reason in their own doc comments — but that \
         was established by looking, not by the count agreeing.\n\
         \n\
         A test removed because it became inconvenient is exactly what this guard \
         exists to make visible, and it is indistinguishable from a rename here."
    );
}
