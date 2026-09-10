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
const RECORDED_TEST_FUNCTIONS: usize = 475;
// 474 when the figure was derived from git at the previous head, plus THIS
// FILE'S OWN test. The guard fired on its own arrival, which is the first
// evidence that it fires at all — and the update is accountable in the way the
// failure message demands: the one added name is named here.

/// Backend test binaries at the stamp.
const RECORDED_TEST_FILES: usize = 104;

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
