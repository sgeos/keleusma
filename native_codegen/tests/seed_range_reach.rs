//! **HOW MANY CORPUS MODULES DOES THE DIFFERENTIAL'S ARGUMENT WIDENING ACTUALLY
//! REACH?**
//!
//! # Why this exists
//!
//! `corpus_differential.rs` drives `SEEDS` argument vectors per module, and its
//! own header records that widening the COUNT from 24 to 64 bought nothing
//! measurable. **The RANGE was never audited.** Until 2026-09-08 every seed
//! produced a small non-negative value — seeds 4 through 23 were the constants
//! 0 to 19 — so a differential running hundreds of comparisons had never driven
//! a negative number or a boundary.
//!
//! Boundaries were added. **This file answers the question that makes that
//! change meaningful or vacuous: how many modules receive more than one
//! argument vector at all?**
//!
//! # Why the answer is not "all of them"
//!
//! The harness drives one vector for a module whose entry takes no parameters,
//! and one for a stream, whose arguments are supplied by the resume protocol
//! rather than by the seed. **Only a module with at least one scalar parameter
//! can see a second seed**, so the widening's reach is bounded by that
//! population and by nothing in the seed table.
//!
//! # What this file does NOT claim
//!
//! It does not claim the boundaries found anything — they did not; the
//! differential stayed green. It states the SIZE OF THE SURFACE the widening
//! applies to, so "we now test negatives" cannot be read as a stronger claim
//! than the corpus supports.

use keleusma::bytecode::{Module, WireShape};

mod common;

/// Does this module's entry take at least one scalar parameter?
fn entry_takes_a_scalar(m: &Module) -> bool {
    let Some(entry) = m.entry_point else {
        return false;
    };
    match m.signatures.get(entry) {
        Some(sig) => {
            !sig.params.is_empty()
                && sig
                    .params
                    .iter()
                    .all(|p| matches!(p, WireShape::Scalar { .. }))
        }
        None => false,
    }
}

/// Is the entry a stream, whose arguments come from the resume protocol?
fn entry_is_a_stream(m: &Module) -> bool {
    m.entry_point
        .and_then(|e| m.chunks.get(e))
        .map(|c| c.block_type == keleusma::bytecode::BlockType::Stream)
        .unwrap_or(false)
}

#[test]
fn how_far_the_argument_widening_reaches() {
    let corpus = common::corpus();
    assert!(!corpus.is_empty(), "the corpus loaded nothing");

    let mut widened: Vec<String> = Vec::new();
    let mut single: Vec<String> = Vec::new();
    for (name, m) in &corpus {
        if entry_takes_a_scalar(m) && !entry_is_a_stream(m) {
            widened.push(name.clone());
        } else {
            single.push(name.clone());
        }
    }

    println!("\n================ REACH OF THE ARGUMENT WIDENING");
    println!("  corpus modules                       : {}", corpus.len());
    println!(
        "  reached by more than one seed        : {} {widened:?}",
        widened.len()
    );
    println!("  driven at ONE vector regardless      : {}", single.len());
    println!(
        "\n  A module with no scalar entry parameter, or a stream entry, sees ONE\n  \
         argument vector whatever the seed table says. Boundary seeds therefore\n  \
         apply to the first group only, and \"the differential now drives\n  \
         negatives and boundaries\" is a claim about THAT population.\n================\n"
    );

    // **NON-VACUITY IN BOTH DIRECTIONS.** If nothing is reached, the boundary
    // seeds are decoration; if everything is, this file is measuring nothing.
    assert!(
        !widened.is_empty(),
        "no corpus module takes a scalar entry parameter, so the seed table --
         boundaries included -- drives exactly one vector everywhere and the
         widening is decoration"
    );
    assert!(
        !single.is_empty(),
        "every corpus module is reached by the widening, which would make this \
         file's distinction empty. Verify that before deleting it: the harness \
         drives one vector for a parameterless or stream entry."
    );
}
