//! **Two live composite returns from the same callee alias each other.**
//!
//! Found by `corpus_differential.rs` as `10_multbyte.kel` returning 1 on the
//! virtual machine and 0 natively, then narrowed to three lines.
//!
//! ```text
//! fn mk(x: Word, y: Word) -> [Word; 2] { [x, y] }
//! fn main(a: Word, b: Word) -> Word { let p = mk(a, b); let r = mk(b, a); p[0] + r[0] }
//! ```
//!
//! With `a = 3, b = 4` the answer is `3 + 4 = 7`. Natively it is **8**, because
//! `p[0]` reads `4`: the second call's body overwrote the first.
//!
//! # The mechanism
//!
//! `plan_chunk_region` gives every flat site in a chunk a distinct offset, and
//! (**Within-chunk non-reuse is enforced by `region_nonreuse.rs`.** The
//! cross-chunk collision this file documents is NOT covered by that guard and
//! remains open — the two are orthogonal.)
//! offsets are planned **per chunk from zero**. `mk` therefore writes its result
//! at the same region offset on every call, while the caller holds two of those
//! results live at once. One buffer, one offset, two live values.
//!
//! A single composite return is fine — `a_single_composite_return_is_correct` below passes —
//! which is why the corpus looked clean until a caller kept two alive.
//!
//! # This is the case `sret` exists for, and I said the corpus did not have one
//!
//! `docs/decisions/NATIVE_COMPOSITE_RETURN_ABI.md` records the operator-
//! authorised caller-allocated return slot and says no multi-chunk composite
//! return exists in the corpus, so the convention was not yet forced. **That was
//! wrong**, and `10_multbyte.kel` is the counterexample. It had lowered
//! "successfully" since composites landed and was never executed, so nothing
//! contradicted the claim.
//!
//! Under `sret` the defect cannot arise: the caller reserves a distinct slot per
//! CALL SITE, so two calls to one callee write to two places.
//!
//! # Reported, not repaired here
//!
//! Implementing `sret` is a lowering change with a region-cost measurement owed
//! first. These tests pin the defect and its boundary so the repair has a target
//! and cannot silently half-land.

mod common;

/// `(vm, native)` for a two-argument entry, driving the trailing pointers when
/// the module builds composites.
fn both(src: &str, a: i64, b: i64) -> (i64, i64) {
    // **ONE HARNESS, NOT A TWIN.** The body of this function moved to
    // `common::vm_and_native_two_arg` when a second file needed it. A copy is
    // how two tests come to answer the same question differently without either
    // saying so, which is a failure mode this package has already recorded.
    common::vm_and_native_two_arg(src, a, b)
}

const MK: &str = "fn mk(x: Word, y: Word) -> [Word; 2] { [x, y] }\n";

/// **The defect.** Two live results from one callee alias.
///
/// **REPAIRED 2026-08-14** and un-ignored. It was a pinned failing case; it is
/// now a regression guard. The repair gives each CALL SITE a disjoint block of
/// the caller's region, so two calls to one callee no longer share offsets.
#[test]
fn two_live_composite_returns_must_not_alias() {
    let src = format!(
        "{MK}fn main(a: Word, b: Word) -> Word {{ let p = mk(a, b); let r = mk(b, a); p[0] + r[0] }}"
    );
    let (vm, nat) = both(&src, 3, 4);
    assert_eq!(
        vm, nat,
        "two live composite returns alias: p[0] should be 3 and r[0] 4, summing \
         to 7, but natively p[0] reads r[0]'s value. plan_chunk_region gives `mk` \
         one offset and the caller holds two of its results at once."
    );
}

/// The boundary: **one** live composite return is correct.
///
/// This is why the corpus looked clean. Nothing was wrong until a caller kept
/// two alive, and no test kept two alive.
#[test]
fn a_single_composite_return_is_correct() {
    let src = format!("{MK}fn main(a: Word, b: Word) -> Word {{ let p = mk(a, b); p[0] + p[1] }}");
    let (vm, nat) = both(&src, 3, 4);
    assert_eq!(vm, 7, "the VM itself must compute 3 + 4");
    assert_eq!(vm, nat, "a single composite return must agree");
}

/// A caller building its OWN composite alongside one callee result is also
/// correct: the two live in different chunks' plans and do not collide.
#[test]
fn a_caller_composite_beside_one_callee_result_is_correct() {
    let src = format!(
        "{MK}fn main(a: Word, b: Word) -> Word {{ let q = [a, b]; let p = mk(a, b); q[0] + p[1] }}"
    );
    let (vm, nat) = both(&src, 3, 4);
    assert_eq!(
        vm, nat,
        "a caller composite beside one callee result must agree"
    );
}
