//! **WHAT IS THE PUBLISHED ARENA FIGURE ACTUALLY MADE OF?**
//!
//! # The question, and why it is not the one already answered
//!
//! `host_arena_supplement_bytes` is the figure this backend tells a host to add
//! to its arena. `arena_high_water.rs` measured how much of it three hand-written
//! streams TOUCH — 16, 24 and 48 bytes of a 520, 536 and 552 byte plan — and
//! found the dominant term to be `stream_spill_bytes`, a flat `MAX_STACK * 8`
//! reserved for every stream chunk.
//!
//! **Three subjects is not a corpus.** This file asks the same question of every
//! corpus module, statically: what fraction of the published figure is that flat
//! reservation?
//!
//! `arena_gap_explanation.rs` answers a DIFFERENT question — why the backend's
//! composite-site sum exceeds the verifier's peak-liveness figure. That gap is
//! about `plan_chunk_region`; this is about the terms beside it.
//!
//! # ⚠ THIS FILE RECOMPUTES THE PLANNER'S SUM, WHICH IS A DRIFT RISK
//!
//! A second computation of a quantity is free to drift from the first. **So the
//! decomposition is checked against the published figure rather than trusted**:
//! the named terms may not exceed it, and the unexplained remainder must be small
//! enough to be alignment. If that check fails, the published figure contains a
//! term this analysis has not named — which is a more interesting result than any
//! share, and is what the assertion says.
//!
//! # What this is NOT
//!
//! **Not a defect report.** The reservation is fixed, static, documented and
//! safe, and its stated rationale — that a predicate disagreeing with the
//! lowering's own would be worse than unused bytes — stands. Over-provisioning
//! errs in the safe direction. **The finding is the SIZE of the looseness in a
//! bound this line sells as definitive**, and what to do about it is an
//! arena-accounting question for the operator, exactly as
//! `host_arena_supplement_bytes` says of itself.

mod common;

use keleusma::bytecode::{BlockType, Module, Op};
use keleusma_native::region::{
    host_arena_supplement_bytes, plan_chunk_region, stream_locals_bytes, stream_spill_bytes,
};

/// The planner rounds at two levels, so an exact sum is not expected. A
/// remainder above this is a term nobody named rather than padding.
///
/// **Eight bytes per chunk visited** — one alignment quantum each — which is the
/// most `align_up` can add per level.
const ALIGN_QUANTUM: u32 = 8;

#[derive(Default, Debug, Clone, Copy)]
struct Terms {
    composite_sites: u32,
    stream_locals: u32,
    stream_spill: u32,
    chunks_visited: u32,
}

impl Terms {
    fn named(&self) -> u32 {
        self.composite_sites + self.stream_locals + self.stream_spill
    }
}

/// Walk the same shape `region_total_bytes` walks, accumulating each term
/// separately. The depth guard matches the planner's.
fn decompose(m: &Module, chunk_index: usize, depth: usize, out: &mut Terms) {
    if depth > 64 {
        return;
    }
    let Some(chunk) = m.chunks.get(chunk_index) else {
        return;
    };
    out.chunks_visited += 1;
    out.composite_sites += plan_chunk_region(chunk).bytes;
    out.stream_locals += stream_locals_bytes(chunk);
    out.stream_spill += stream_spill_bytes(chunk);
    for op in &chunk.ops {
        if let Op::Call(idx, _) = op {
            decompose(m, usize::from(*idx), depth + 1, out);
        }
    }
}

/// Every corpus module with an entry point, decomposed.
fn census() -> Vec<(String, Terms, u32, bool)> {
    common::corpus()
        .into_iter()
        .filter_map(|(name, m)| {
            let entry = m.entry_point?;
            let mut t = Terms::default();
            decompose(&m, entry, 0, &mut t);
            let published = host_arena_supplement_bytes(&m);
            let has_stream = m.chunks.iter().any(|c| c.block_type == BlockType::Stream);
            Some((name, t, published, has_stream))
        })
        .collect()
}

/// **THE DECOMPOSITION IS CHECKED, NOT ASSUMED.**
#[test]
fn the_named_terms_account_for_the_published_figure() {
    let rows = census();
    assert!(
        rows.len() >= 20,
        "the census enumerated only {} modules; that is a broken probe rather \
         than a small corpus",
        rows.len()
    );
    for (name, t, published, _) in &rows {
        assert!(
            t.named() <= *published,
            "`{name}`: the named terms sum to {} but the backend publishes \
             {published}. A decomposition exceeding the total it decomposes is \
             this file misreading the planner, not a finding about the planner.",
            t.named()
        );
        let remainder = published - t.named();
        let slack = t.chunks_visited * ALIGN_QUANTUM;
        assert!(
            remainder <= slack,
            "`{name}`: {remainder} byte(s) of the published {published} are \
             explained by no named term, above the {slack} bytes alignment can \
             add over {} chunk(s). **The published figure contains a term this \
             analysis has not named**, which is a more interesting result than \
             any share and must be identified before the figures below mean \
             anything.",
            t.chunks_visited
        );
    }
}

/// **THE RESULT, AND IT IS NOT THE ONE THIS FILE FIRST CLAIMED.**
///
/// The first version of this test asserted the reservation *"dominates every
/// streaming module"* at 50% or more. **The corpus refuted that on the first
/// run**: `piano_roll_0` and `piano_roll_1` sit at 27% of an 1840-byte figure,
/// and `piano_roll_3` at 47%. The claim came from three hand-written streams
/// whose composite regions were tiny; the corpus contains modules whose are not.
///
/// **What IS true, measured across the corpus:**
///
/// - every streaming module reserves the SAME flat block, independent of what it
///   does — that is the property, and it is asserted below;
/// - it is the majority of the published figure for most of them;
/// - **for the twelve self-hosted compiler stages it is 98%** of a figure a host
///   is told to provision, those modules planning barely anything else.
///
/// The last of those is the one worth carrying: the compiler this line exists to
/// support has a published arena demand that is almost entirely a reservation
/// whose deepest observed use is one slot.
#[test]
fn every_streaming_module_reserves_the_same_flat_block() {
    let rows = census();
    let streaming: Vec<_> = rows.iter().filter(|(_, _, _, s)| *s).collect();
    let plain: Vec<_> = rows.iter().filter(|(_, _, _, s)| !*s).collect();

    assert!(
        !streaming.is_empty(),
        "no corpus module has a stream chunk, so this census measures nothing"
    );
    assert!(
        !plain.is_empty(),
        "every corpus module has a stream chunk, so the contrast below is vacuous"
    );

    for (name, t, published, _) in &plain {
        assert_eq!(
            t.stream_spill, 0,
            "`{name}` has no stream chunk yet reserves {} spill byte(s) of \
             {published}. The reservation is documented as per-stream-chunk; if \
             it became unconditional it widens the figure for every module.",
            t.stream_spill
        );
    }

    let sizes: std::collections::BTreeSet<u32> = streaming
        .iter()
        .map(|(_, t, _, _)| t.stream_spill)
        .collect();
    assert_eq!(
        sizes.len(),
        1,
        "streaming modules reserve DIFFERENT amounts: {sizes:?}. The figure is \
         documented as fixed and static, which is what makes it admissible for \
         this project at all; if it became input-dependent, the worst-case bound \
         stops moving by a known constant."
    );
}

/// **The distribution, pinned. A share is a measured property, not a target.**
#[test]
fn the_reservation_share_across_the_corpus_is_pinned() {
    /// Modules with a stream chunk on the entry path.
    const PINNED_STREAMING: usize = 28;
    /// Of those, how many where the reservation is at least half the figure.
    const PINNED_MAJORITY: usize = 25;
    /// The lowest share any streaming module shows, as a percentage.
    const PINNED_MIN_SHARE: u32 = 27;

    let rows = census();
    let mut streaming: Vec<(&str, u32, u32, u32)> = rows
        .iter()
        .filter(|(_, _, _, s)| *s)
        .map(|(n, t, p, _)| {
            (
                n.as_str(),
                t.stream_spill,
                *p,
                t.stream_spill * 100 / (*p).max(1),
            )
        })
        .collect();
    streaming.sort_by_key(|(_, _, _, share)| *share);

    let majority = streaming.iter().filter(|(_, _, _, sh)| *sh >= 50).count();
    let min_share = streaming.first().map(|(_, _, _, sh)| *sh).unwrap_or(0);

    assert_eq!(
        (streaming.len(), majority, min_share),
        (PINNED_STREAMING, PINNED_MAJORITY, PINNED_MIN_SHARE),
        "the reservation-share distribution moved. Measured: {streaming:?}.\n\n\
         Name what changed -- a module entering or leaving the corpus, a planner \
         term moving, or a composite region growing -- and whether the new figure \
         is what you expect. Do not re-pin without saying which."
    );
}
