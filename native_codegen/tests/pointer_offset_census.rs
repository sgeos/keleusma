//! **EVERY POINTER-ARITHMETIC SITE IN THE EMITTER, AND WHERE ITS OFFSET COMES
//! FROM.**
//!
//! # Why this exists
//!
//! On 2026-09-08 `Op::GetIndex` was found computing `base + index * stride` for
//! a RUNTIME index with no bound, on the strength of a comment claiming the
//! compiler emitted a `BoundsCheck` that it does not emit. A three-element array
//! indexed at 5 returned the caller region's filler bytes where the reference
//! faults.
//!
//! That was found by accident. The premise sweep in
//! `upstream_premise_census.rs` is the deliberate version for COMMENTS; this is
//! the deliberate version for the thing a bad comment endangered — **the pointer
//! arithmetic itself.**
//!
//! # The classification, which is the whole content
//!
//! An offset is safe when it is one of two things, and dangerous otherwise:
//!
//! 1. **Compile-time constant** — a local's slot, a field's offset, a call
//!    site's region offset, the stream state word. Nothing a program supplies
//!    reaches it.
//! 2. **Runtime value, bounded before use** — an array index or a data-segment
//!    index, compared UNSIGNED against a bound and branched to the trap on
//!    failure. Unsigned matters: it catches a negative index with the same test.
//!
//! At the stamp, every site is one of those. The two that were not are the
//! `GetIndex` pair, fixed the same day.
//!
//! | site | offset source |
//! |---|---|
//! | locals in the stream frame | constant — `plan_chunk_region` bytes plus slot index |
//! | stream state word (three sites) | constant — `stream_state_off` |
//! | call-site region base | constant — per-site plan |
//! | shared data, indexed and not | runtime, guarded by the opcode's own bound |
//! | private data, indexed and not | runtime, same guard — it precedes the shared/private split |
//! | composite field write and read | constant — the layout's field offset |
//! | composite body base | constant — the site's planned offset |
//! | **flat array element** | **runtime, guarded by `guard_array_index`** |
//!
//! # What this test can and cannot do
//!
//! It counts sites. **It cannot prove a given site is guarded** — that is what
//! the differential and `partial_operation_census.rs` do by execution. What it
//! refuses is silent GROWTH: a new pointer-arithmetic site fails this, and the
//! author must classify it above. A site added without a bound is how the
//! `GetIndex` defect existed for as long as it did.

/// Pointer-arithmetic sites in the emitter, at the stamp.
///
/// **Re-derive rather than trust.** It moves whenever a site is added or
/// removed.
const RECORDED_GEP_SITES: usize = 12;

fn gep_sites() -> Vec<(usize, String)> {
    let src = std::fs::read_to_string("src/lib.rs").expect("the emitter is readable");
    src.lines()
        .enumerate()
        .filter(|(_, l)| l.contains("build_in_bounds_gep"))
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .collect()
}

#[test]
fn every_pointer_offset_is_constant_or_bounded() {
    let sites = gep_sites();
    println!("\n================ POINTER-ARITHMETIC SITES IN THE EMITTER");
    for (n, _) in &sites {
        println!("  src/lib.rs:{n}");
    }
    println!("  ------------------------------------------------");
    println!("  sites: {}", sites.len());
    println!(
        "\n  An offset is safe when it is a COMPILE-TIME CONSTANT or a RUNTIME\n  \
         value BOUNDED before use. Every site is classified in this file's\n  \
         header. The two that were neither returned foreign memory.\n================\n"
    );

    // **NON-VACUITY.** A matcher that finds nothing would pass forever.
    assert!(
        !sites.is_empty(),
        "no pointer-arithmetic site matched, so either the emitter stopped using \
         `build_in_bounds_gep` or this matcher has gone stale. A census that \
         cannot fire is indistinguishable from no census."
    );
    assert_eq!(
        sites.len(),
        RECORDED_GEP_SITES,
        "the number of pointer-arithmetic sites in the emitter has changed. Do \
         not patch the number: classify the new site in this file's header as \
         COMPILE-TIME CONSTANT or RUNTIME-AND-BOUNDED, and if it is neither, it \
         is the defect this census was written after."
    );
}

/// **The data-segment guard must precede BOTH the shared and private paths.**
///
/// The private path computes `base + index * PRIVATE_SLOT_BYTES` from the same
/// runtime index. It is safe only because the bound check sits ahead of the
/// branch that separates the two, which is a property of the ORDER of two
/// statements and would be silently lost by a refactor that moved either.
#[test]
fn the_data_index_guard_precedes_the_shared_private_split() {
    let src = std::fs::read_to_string("src/lib.rs").expect("the emitter is readable");
    let guard = src
        .find("\"dataoob\"")
        .expect("the data-segment bound comparison is named `dataoob`");
    let private = src
        .find("\"pdataptr\"")
        .expect("the private data pointer is named `pdataptr`");
    let shared = src
        .find("\"sdataptr\"")
        .expect("the shared data pointer is named `sdataptr`");

    assert!(
        guard < shared && guard < private,
        "the data-segment bound check no longer precedes both pointer \
         computations (guard at {guard}, shared at {shared}, private at \
         {private}). The private path indexes by the same runtime value, so it \
         is bounded only by sitting after this check."
    );
}
