//! **HOW DEEP HAS A STREAM ACTUALLY BEEN DRIVEN? SIX TICKS.**
//!
//! # Why that is too shallow for this line's central claim
//!
//! **The arena IS the coroutine instance**, and `Op::Reset` rewinds it between
//! runs. A defect that accumulates across ticks — a pointer that creeps, state
//! surviving a rewind, a resume path that drifts — is invisible at shallow depth.
//!
//! Measured across this package before this file: hand-written stream witnesses
//! drive **2 to 6 replies**, the longest being `&[11, 20, 31, 40, 55, 60]`.
//!
//! The self-hosted stage differential does far better — 180 to 300 comparisons
//! per stage — but those are stage sources with particular shapes, seeded from
//! the shared segment. **A general stream carrying a local across a yield, driven
//! for hundreds of ticks, is a different subject**, and it is the shape that
//! already carried a defect this line fixed: a local live across `yield`, wiped
//! by the entry preamble on every resume.
//!
//! # What this found
//!
//! **No divergence.** Evidence raised from 6 ticks to 200. That is the result:
//! not a defect, but a floor lifted on the claim everything else rests on.
//!
//! # ⚠ A SINGLE-YIELD STREAM IS DEGENERATE, AND THE GENERAL DRIVER REFUSES IT
//!
//! `loop main(t: Word) -> Word { yield t }` panics
//! `common::general_native_sequence`, and that is **correct**. Measured
//! signatures: a one-yield stream lowers with **1 parameter**, a two-yield stream
//! with **4** — `declared + 3` trailing pointers. The general driver asserts
//! those three precisely because it names the signature by hand and cannot detect
//! a change to it.
//!
//! **Fourth time this session a probe implicated itself** rather than the
//! backend: the others were a float argument passed as `i64::MIN`, a float return
//! read as an integer, and a hand-named signature that took a SIGBUS.
//!
//! # What this does NOT claim
//!
//! **Nothing about memory.** Whether the arena grows across ticks is not measured
//! here; only that the two implementations produce identical sequences. A bounded
//! arena is this line's claim elsewhere and is not evidenced by this file.
//!
//! **That gap is now filled elsewhere, and this note is kept so the reader is
//! sent there rather than concluding nothing measures it.** `arena_high_water.rs`
//! measures the arena's touched extent over the same depth and finds it identical
//! at 2, 20 and 200 ticks. It remains true that THIS file evidences none of it.

mod common;

/// Shapes worth driving deep. **A branching form and a local carried across a
/// yield are the two that could drift**; the plain form is the control.
const SHAPES: &[(&str, &str)] = &[
    (
        "plain two-yield",
        "loop main(t: Word) -> Word { (yield t) + (yield t + 1) }",
    ),
    (
        "a local carried across a yield",
        "loop main(t: Word) -> Word { let keep = t + 100; let r = yield 1; yield r + keep }",
    ),
    (
        "branching on the seed",
        "loop main(t: Word) -> Word { if t > 0 { let a = yield t; yield a } else { yield 0 } }",
    ),
    // **Carried here to give the PRIVATE-region canary reach.** The other three
    // declare no private data, so nothing writes there and that canary can never
    // fire for them — a guard with no reach, which is the thing this package
    // keeps finding. A shared-slot stream would give the third canary reach too,
    // but `general_vm_sequence` calls without supplying a shared segment, so the
    // reference refuses that shape and it is not drivable by this helper.
    (
        "writing a private data slot each tick",
        "private data d { n: Word }\nloop main(t: Word) -> Word { d.n = t; (yield d.n) + (yield d.n + 1) }",
    ),
];

/// Ticks to drive. The previous maximum in this package was **6**.
const DEPTH: usize = 200;

/// Fewer compared values than this means the run was short, not that the streams
/// agreed. A shortened run otherwise passes every assertion below.
const COMPARED_FLOOR: usize = 150;

/// Replies that VARY. A constant reply would mask a resume path that ignores its
/// input, which is the kind of drift this file looks for.
///
/// **⚠ THIS PROPERTY IS NOT MUTATION-TESTED, AND CANNOT BE.** Replacing the
/// varying replies with a constant leaves every assertion here passing — because
/// with no defect present the two implementations agree either way. Varying
/// replies raise DETECTION POWER against a defect that does not currently exist;
/// they are a design choice, not a guarded invariant, and the distinction is
/// recorded rather than implied. Mutating them tests nothing, the same way
/// changing a bounds-check witness's index tested nothing.
fn replies(n: usize) -> Vec<i64> {
    (0..n).map(|i| (i as i64 % 7) - 3).collect()
}

#[test]
fn general_streams_agree_over_two_hundred_ticks() {
    let r = replies(DEPTH);
    for (label, src) in SHAPES {
        let vm = common::general_vm_sequence(src, 1, &r);
        let native = common::general_native_sequence(src, 1, &r);
        assert!(
            vm.len() >= COMPARED_FLOOR,
            "`{label}` produced only {} values, below the floor of \
             {COMPARED_FLOOR}. A short run agrees trivially; this is a broken \
             probe rather than a result.",
            vm.len()
        );
        assert_eq!(
            vm, native,
            "`{label}` DIVERGES from the reference over {DEPTH} ticks. The \
             previous deepest witness in this package was SIX, so a drift that \
             needs tens of ticks to appear would have been invisible."
        );
    }
}

/// **The rewind path, which suspend-and-resume alone does not exercise.**
///
/// A two-yield stream completes on its second resume and is rewound by the next
/// call, so driving it past its own length cycles it repeatedly.
#[test]
fn a_completing_stream_agrees_across_repeated_rewinds() {
    const SRC: &str = "loop main(t: Word) -> Word { (yield t) + (yield t + 1) }";
    let r = replies(40);
    let vm = common::general_vm_sequence(SRC, 1, &r);
    let native = common::general_native_sequence(SRC, 1, &r);
    assert!(
        vm.len() >= 30,
        "only {} values; the stream did not cycle enough to exercise the rewind",
        vm.len()
    );
    assert_eq!(
        vm, native,
        "a completing stream DIVERGES across repeated rewinds. `Op::Reset` is \
         what rewinds the arena between runs, and the arena IS the instance."
    );
}

/// **Why a single-yield stream is not in `SHAPES`.**
///
/// It lowers DEGENERATE — no trailing pointers — so the general driver refuses
/// it, correctly. Asserted by signature so the distinction is measured rather
/// than remembered, and so a change to either lowering fails here.
#[test]
fn a_single_yield_stream_lowers_degenerate_and_a_two_yield_one_does_not() {
    use inkwell::context::Context;
    let counts = |src: &str| -> (u32, u32) {
        let m = common::build(src);
        let entry = m.entry_point.expect("entry");
        let ctx = Context::create();
        let lm = ctx.create_module("k");
        keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default())
            .expect("lower");
        let f = lm
            .get_function(&format!("kel_chunk_{entry}"))
            .expect("entry function");
        (f.count_params(), u32::from(m.chunks[entry].param_count))
    };

    let (degen, declared_d) = counts("loop main(t: Word) -> Word { yield t }");
    assert_eq!(
        (degen, declared_d),
        (1, 1),
        "a single-yield stream no longer lowers degenerate. If it became \
         resumable, it belongs in `SHAPES`; if its parameter count changed for \
         another reason, the general driver's hand-named signature needs re-reading."
    );

    let (general, declared_g) = counts(SHAPES[0].1);
    assert_eq!(
        (general, declared_g + 3),
        (4, 4),
        "a two-yield stream no longer carries the three trailing pointers, which \
         is the signature every general-stream driver here names by hand."
    );
}
