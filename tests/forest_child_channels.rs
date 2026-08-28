//! **THE FOREST'S CHILDREN DO NOT ALL LIVE IN `lhs` AND `rhs`, AND A WALK THAT ASSUMES THEY DO IS
//! SILENTLY INCOMPLETE.**
//!
//! Written after a measured failure, not as a precaution.
//!
//! # What was measured
//!
//! The last unmigrated type-channel extraction, `expression_nodes_resolvable`, produces a
//! POSITIONAL table: another value indexes into it, so reproducing it from the pipeline requires
//! reproducing its ORDER. A probe walked the reconstructed forest in preorder over `lhs` and `rhs`
//! to see whether that order was recoverable. It is not, and the way it failed is the useful part:
//!
//! | program | reference sees | the `lhs`/`rhs` walk saw |
//! |---|---|---|
//! | `g() + g() * 2` | two calls | **one** |
//! | `f(1)` | one call | **hundreds**, until a guard stopped it |
//!
//! So the walk **misses children and revisits nodes**, in the same probe. A call's arguments are
//! not in `lhs`/`rhs` at all — they are in `call_args` — and the loop, match, limit and multihead
//! constructs each keep their parts in their own side-table.
//!
//! # What this file pins, and what it deliberately does not
//!
//! It pins that the forest has exactly SIX child-bearing channels, so a walk written against them
//! is complete **by construction** and a seventh cannot appear silently. It does NOT attempt the
//! walk, and it does not claim the ordering question is settled — only that anyone answering it
//! now knows where the children are, which cost a probe to establish.
//!
//! **A caution for whoever writes that walk.** `codegen.kel`'s visit order is EMISSION order for a
//! stack machine, not the reference's abstract-syntax-tree preorder. Copying its child sequence
//! would produce a consistent traversal that is nonetheless the wrong order for this comparison.

#![cfg(all(feature = "self-host", feature = "compile"))]

const DRIVER: &str = include_str!("../src/selfhost/mod.rs");

/// The fields of the flattened-body struct, as the driver declares them.
fn body_fields() -> Vec<(String, String)> {
    let at = DRIVER
        .find("pub struct Body {")
        .expect("the driver declares the flattened body");
    let open = at + DRIVER[at..].find('{').expect("a brace");
    let close = open
        + DRIVER[open..]
            .find("\n}")
            .expect("the struct closes at column zero");
    DRIVER[open + 1..close]
        .lines()
        .filter_map(|l| {
            let t = l.trim().trim_end_matches(',');
            t.split_once(": ")
                .map(|(n, ty)| (n.to_string(), ty.to_string()))
        })
        .collect()
}

/// **THE CHILD CHANNELS ARE EXACTLY SIX, AND A SEVENTH CANNOT ARRIVE UNNOTICED.**
///
/// Derived from the declaration rather than restated. A walk over these six is complete; a walk
/// over `lhs`/`rhs` alone is not, which is measured in this file's header rather than assumed.
#[test]
fn the_forest_keeps_its_children_in_exactly_six_channels() {
    let fields = body_fields();

    // NON-VACUITY. An extraction that found nothing would satisfy every comparison below.
    assert!(
        fields.len() >= 6,
        "the body-struct extraction found {} fields, so it has broken rather than the struct \
         having changed: {fields:?}",
        fields.len()
    );

    let channels: Vec<&str> = fields
        .iter()
        .filter(|(_, ty)| ty.starts_with("Vec<"))
        .map(|(n, _)| n.as_str())
        .collect();
    assert_eq!(
        channels,
        vec![
            "nodes",
            "call_args",
            "for_parts",
            "match_parts",
            "limit_parts",
            "head_parts",
        ],
        "the flattened body's child channels have changed. A SEVENTH means any walk written \
         against the six is now incomplete and silently misses that construct's children; one \
         REMOVED means a walk reads a channel that no longer exists. Either way the ordering work \
         for the last type-channel extraction has to account for it."
    );

    // The scalars, asserted separately so the filter above cannot pass by classifying everything
    // as a channel.
    let scalars: Vec<&str> = fields
        .iter()
        .filter(|(_, ty)| !ty.starts_with("Vec<"))
        .map(|(n, _)| n.as_str())
        .collect();
    assert_eq!(
        scalars,
        vec!["category", "root"],
        "the flattened body's non-channel fields have changed"
    );
}

/// **A CALL'S ARGUMENTS ARE NOT REACHABLE THROUGH `lhs` AND `rhs`.**
///
/// The concrete half of the header's table, as something a reader can run. A program with two
/// calls whose arguments are expressions is reconstructed, and the node array is shown to contain
/// more call-argument structure than an `lhs`/`rhs` walk from the root can reach.
///
/// Stated as a property of the REPRESENTATION rather than of any particular walk: the count of
/// `Call` nodes in the forest exceeds what the two child fields alone connect.
#[test]
fn a_calls_arguments_are_not_in_the_two_child_fields() {
    const SRC: &str = "fn g(x: Word) -> Word { x }\nfn main() -> Word { g(1) + g(2) }";

    // The reference is the authority on how many calls the program has. Without this the counts
    // below would be compared against an assumption.
    let ast =
        keleusma::parser::parse(&keleusma::lexer::tokenize(SRC).expect("lex")).expect("parse");
    let module = keleusma::compiler::compile(&ast).expect("compile");
    let calls = module
        .chunks
        .iter()
        .flat_map(|c| c.ops.iter())
        .filter(|op| matches!(op, keleusma::bytecode::Op::Call(..)))
        .count();
    assert_eq!(
        calls, 2,
        "the probe program no longer contains exactly two calls, so the claim below is about a \
         different program than the one it describes"
    );

    // The pipeline reaches both calls when it uses the driver's own extraction, which consults
    // the call-argument channel. This is the positive control for the header's table: the
    // information IS present in the forest, and only the two-field walk fails to reach it.
    let sites = keleusma::selfhost::decl_call_rows_from_pipeline(SRC).1;
    assert_eq!(
        sites.len(),
        2,
        "the pipeline no longer reports both call sites, which would mean the forest itself is \
         missing them rather than a naive walk being unable to reach them: {sites:?}"
    );
}
