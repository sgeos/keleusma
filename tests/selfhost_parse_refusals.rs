//! What the self-hosted front end refuses out of the SHIPPED example corpus, by
//! name, and through the entry point that returns rather than aborts.
//!
//! # Why this is pinned rather than described
//!
//! The `v0.3.0` line reported from the outside that `parse_functions` panics on
//! **four of the eleven** example scripts, naming `02_struct_field.kel`,
//! `08_method_dispatch.kel`, `09_big_numbers.kel` and `10_multbyte.kel`, and
//! offering the hypothesis that a top-level `struct` declaration was the shared
//! cause. That report was correct when it was made and carried honestly as a
//! hypothesis rather than a diagnosis.
//!
//! **Re-measured here, it is two, and the cause has changed.** The
//! struct/trait/impl skip state closed the first two. The surviving two do not
//! reach the declaration path at all — they fail inside `parse.kel` with an
//! index fault — so "a top-level `struct`" no longer explains either of them.
//!
//! A count that lives only in prose drifts silently in both directions: it was
//! quoted as four after two of the four were fixed. This test is where the
//! number lives now, so closing either remaining refusal FAILS here and forces
//! the claim to be restated.
//!
//! # What a passing run does NOT establish
//!
//! Nothing about whether the two refusals are cheap to fix, and nothing about
//! any source outside `examples/scripts/`. It also does not establish that the
//! self-hosted front end COMPILES the nine it parses — parsing is the first
//! stage of several, and this test stops there.

#![cfg(feature = "self-host")]

use keleusma::selfhost::try_parse_functions;
use std::path::PathBuf;

/// The scripts refused today, with the fault each reports.
///
/// Kept as a table rather than a count so a changed CAUSE fails as loudly as a
/// changed set. The standing report's other two entries were repaired without
/// the prose count moving, which is the failure this shape prevents.
const REFUSED: &[(&str, &str)] = &[
    ("09_big_numbers.kel", "IndexOutOfBounds(-1, 65)"),
    ("10_multbyte.kel", "IndexOutOfBounds(-1, 65)"),
];

fn scripts() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/scripts");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("examples/scripts is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn every_shipped_example_is_parsed_or_refused_by_name() {
    let paths = scripts();
    assert_eq!(
        paths.len(),
        11,
        "the example corpus changed size; this test's expectations are stated \
         against the eleven scripts that were there when it was written"
    );

    let mut refused: Vec<(String, String)> = Vec::new();
    let mut accepted: Vec<(String, usize)> = Vec::new();

    for path in &paths {
        let name = path
            .file_name()
            .expect("a file name")
            .to_string_lossy()
            .to_string();
        let src = std::fs::read_to_string(path).expect("script is readable");
        match try_parse_functions(&src) {
            Ok(program) => accepted.push((name, program.functions.len())),
            Err(e) => refused.push((name, e.message)),
        }
    }

    let refused_names: Vec<&str> = refused.iter().map(|(n, _)| n.as_str()).collect();
    let expected_names: Vec<&str> = REFUSED.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        refused_names, expected_names,
        "the set of refused example scripts moved. If a refusal was CLOSED, \
         update this table and say so where the count is published; if a new \
         one appeared, that is a regression in the self-hosted front end."
    );

    for ((name, message), (_, expected_fault)) in refused.iter().zip(REFUSED) {
        assert!(
            message.contains(expected_fault),
            "{name} is still refused but the fault changed: expected a message \
             containing {expected_fault:?}, got {message:?}. A changed cause \
             matters even when the verdict does not, because the recorded \
             diagnosis is what the next reader acts on."
        );
    }

    // Non-vacuity. A front end that accepted everything and produced nothing
    // would satisfy every assertion above, and that is close to what the
    // reported failure mode looked like from the outside.
    for (name, count) in &accepted {
        assert!(
            *count > 0,
            "{name} parsed to ZERO functions, so acceptance here means the \
             stage produced nothing rather than that it succeeded"
        );
    }
    assert_eq!(
        accepted.len(),
        paths.len() - REFUSED.len(),
        "accepted and refused do not partition the corpus"
    );
}

#[test]
fn a_refusal_is_returned_rather_than_ending_the_process() {
    // The point of the fallible entry point, stated as an executable claim: a
    // source that panics the original API comes back as a value here. If this
    // test runs to completion at all, the process was not aborted.
    let src = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/scripts/09_big_numbers.kel"),
    )
    .expect("readable");

    let err = try_parse_functions(&src).expect_err("this source is refused today");
    assert!(
        !err.message.is_empty(),
        "a refusal with an empty message is worse than a panic, because the \
         caller has a value and no reason"
    );
    assert_eq!(
        err.to_string(),
        err.message,
        "Display must show the message, since that is the whole payload"
    );
}
