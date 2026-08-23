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

/// The eleven scripts this claim is about, NAMED.
///
/// # Why named rather than "whatever is in the directory"
///
/// **`examples/scripts/` is a directory the `v0.3.0` line GROWS and this line's
/// tests assert over.** They add opcode-witness scripts there — that route took
/// their instruction-set census from 64 to 66 witnessed opcodes — and every one
/// of them broke a size-pinned assertion here the moment they absorbed. The
/// failure appeared only on their tree, where their corpus meets this test, and
/// was invisible from here.
///
/// Pinning the COUNT coupled a claim about ELEVEN SPECIFIC SCRIPTS to another
/// line's unrelated work. The claim was never about the directory's size; it was
/// about which shipped examples the self-hosted front end refuses, correcting a
/// figure that had been quoted as four when it was two. Naming them says that,
/// and additions are simply outside the set rather than a breakage.
///
/// **This does not weaken the test.** Every name below must still be PRESENT and
/// must still be classified, so a removal or rename fails here rather than
/// silently shrinking what is checked.
const CORPUS: &[&str] = &[
    "01_arithmetic.kel",
    "02_struct_field.kel",
    "03_enum_match.kel",
    "04_for_in.kel",
    "05_pipeline.kel",
    "06_multiheaded.kel",
    "07_refinement.kel",
    "08_method_dispatch.kel",
    "09_big_numbers.kel",
    "10_multbyte.kel",
    "11_signed.kel",
];

fn scripts() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/scripts");
    CORPUS
        .iter()
        .map(|name| {
            let p = dir.join(name);
            assert!(
                p.is_file(),
                "{name} is named in this test's corpus and is not present. If it \
                 was renamed or removed, update CORPUS deliberately -- silently \
                 checking ten scripts while claiming eleven is the failure this \
                 assertion exists to prevent."
            );
            p
        })
        .collect()
}

#[test]
fn every_shipped_example_is_parsed_or_refused_by_name() {
    let paths = scripts();
    assert_eq!(
        paths.len(),
        CORPUS.len(),
        "the named corpus and the resolved paths disagree"
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
