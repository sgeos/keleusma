//! The published API, exercised as a consumer sees it.
//!
//! # Why this file exists
//!
//! `keleusma-arena` is published on crates.io, and until this file every one
//! of its tests lived inside the crate. An integration test is a genuinely
//! different check: it can reach only the **public** surface, so it catches an
//! item that is not actually exported and an API that cannot be used without a
//! private helper. A unit test cannot fail for either reason.
//!
//! The project instructions claimed this file's contents already existed --
//! "51 lib plus 8 integration" -- while `keleusma-arena/tests/` did not exist
//! at all. That is recorded in `CHANGELOG.md`; the claim is corrected and the
//! structural half of it is now guarded.
//!
//! # What is asserted
//!
//! The guarantees the crate exists to provide, not incidental details: the
//! dual-head discipline, exhaustion as a clean error rather than a panic, the
//! persistent region surviving a reset, epoch-based staleness, and the budget
//! check agreeing with the usage counters.
//!
//! Every case that means to exercise a failure path asserts it **reached**
//! that path. An allocation that unexpectedly succeeded would otherwise leave
//! an unfilled cell reported as a pass.

use keleusma_arena::{Arena, Budget};

/// The ordinary workflow is reachable using only exported items.
///
/// Deliberately unremarkable. Its value is that it compiles: every item it
/// touches must be public, and the sequence must be usable without anything
/// the crate keeps to itself.
#[test]
fn the_ordinary_workflow_uses_only_public_items() {
    let mut arena = Arena::with_capacity(4096);
    assert_eq!(arena.capacity(), 4096);
    assert_eq!(arena.bottom_used(), 0);
    assert_eq!(arena.top_used(), 0);

    let _bottom = arena
        .alloc_bottom_bytes(64)
        .expect("a 64-byte bottom allocation fits in 4096");
    let _top = arena
        .alloc_top_bytes(32)
        .expect("a 32-byte top allocation fits alongside it");

    assert!(arena.bottom_used() >= 64, "the bottom head did not advance");
    assert!(arena.top_used() >= 32, "the top head did not advance");
    assert!(
        arena.free() < 4096,
        "free space did not fall after two allocations"
    );

    let before = arena.epoch();
    arena.reset().expect("a fresh arena has epochs to spare");
    assert!(
        arena.epoch() > before,
        "reset did not advance the epoch, so a stale handle could not be detected"
    );
    assert_eq!(
        arena.bottom_used(),
        0,
        "reset did not release the bottom head"
    );
    assert_eq!(arena.top_used(), 0, "reset did not release the top head");
}

/// The two heads allocate against each other and exhaustion is an error.
///
/// The loop bound is generous, and **not reaching exhaustion fails the test**:
/// a run in which every allocation succeeded would have exercised none of the
/// behaviour this case is for.
#[test]
fn the_heads_meet_and_exhaustion_is_an_error_rather_than_a_panic() {
    let arena = Arena::with_capacity(1024);
    let mut exhausted = false;
    for _ in 0..4096 {
        if arena.alloc_bottom_bytes(64).is_err() {
            exhausted = true;
            break;
        }
        if arena.alloc_top_bytes(64).is_err() {
            exhausted = true;
            break;
        }
    }
    assert!(
        exhausted,
        "a 1 KiB arena absorbed 4096 allocations of 64 bytes from each head without \
         reporting exhaustion, so this case never reached the behaviour it tests"
    );
    // Having refused, the arena is still usable rather than poisoned: its
    // accounting still answers.
    assert!(arena.bottom_used() + arena.top_used() <= arena.capacity());
}

/// The persistent region survives a reset; the ephemeral heads do not.
///
/// This is the crate's load-bearing guarantee for a host that keeps state
/// across a coroutine boundary, and it is asserted through the public pointer
/// accessor rather than by reading a private field.
#[test]
fn the_persistent_region_survives_a_reset() {
    let mut arena = Arena::with_capacity(4096);
    arena
        .resize_persistent(128)
        .expect("128 persistent bytes fit in 4096");
    assert_eq!(arena.persistent_capacity(), 128);

    // SAFETY: the pointer is the arena's own persistent region, which the
    // accessor exposes for exactly this purpose, and 128 bytes were just
    // reserved. The borrow ends before the reset below.
    unsafe {
        let p = arena.persistent_ptr().as_ptr();
        for i in 0..128usize {
            p.add(i).write(0xAB);
        }
    }

    let _ephemeral = arena.alloc_bottom_bytes(64).expect("ephemeral allocation");
    arena.reset().expect("reset");

    assert_eq!(
        arena.bottom_used(),
        0,
        "the ephemeral head survived a reset, which it must not"
    );
    // SAFETY: as above; the region is still reserved after the reset, which is
    // the property under test.
    unsafe {
        let p = arena.persistent_ptr().as_ptr();
        for i in 0..128usize {
            assert_eq!(
                p.add(i).read(),
                0xAB,
                "persistent byte {i} did not survive the reset"
            );
        }
    }
}

/// Zeroing the persistent region clears it, so the survival above is a
/// property of reset rather than of nothing ever clearing.
///
/// Without this the previous test would pass against an arena that simply
/// never wrote to that memory again.
#[test]
fn zeroing_the_persistent_region_actually_clears_it() {
    let mut arena = Arena::with_capacity(4096);
    arena.resize_persistent(64).expect("reserve");
    // SAFETY: the arena's own region, 64 bytes reserved above.
    unsafe {
        let p = arena.persistent_ptr().as_ptr();
        for i in 0..64usize {
            p.add(i).write(0xCD);
        }
    }
    arena.zero_persistent();
    // SAFETY: as above.
    unsafe {
        let p = arena.persistent_ptr().as_ptr();
        for i in 0..64usize {
            assert_eq!(p.add(i).read(), 0, "persistent byte {i} was not zeroed");
        }
    }
}

/// The budget check is about the BUDGET's admissibility, not about current use.
///
/// Worth pinning because the name invites the opposite reading, and the first
/// version of this test asserted it: that an arena already holding more than a
/// budget allows would report the budget as not fitting. It does not. The
/// contract, stated in the method's own documentation, is
/// `budget.total() <= capacity` — a producer asks whether a budget it computed
/// is admissible for this arena at all, and what happens to be allocated right
/// now has nothing to do with the answer.
#[test]
fn the_budget_check_asks_whether_the_budget_fits_the_capacity() {
    let arena = Arena::with_capacity(4096);
    assert!(
        arena.fits_budget(&Budget::new(1024, 1024)),
        "a 2048-byte budget does not fit a 4096-byte arena"
    );
    assert!(
        !arena.fits_budget(&Budget::new(4096, 4096)),
        "an 8192-byte budget fits a 4096-byte arena"
    );
    assert_eq!(Budget::new(1024, 1024).total(), 2048);

    // Current usage does not enter the answer, which is the part a reader is
    // most likely to assume otherwise.
    let verdict_before = arena.fits_budget(&Budget::new(64, 64));
    let _ = arena.alloc_bottom_bytes(512).expect("512 bottom bytes");
    assert_eq!(
        arena.fits_budget(&Budget::new(64, 64)),
        verdict_before,
        "allocating changed the budget verdict, so the check is not purely about capacity"
    );
}

/// A fallible construction reports failure rather than aborting.
///
/// The published API offers a `try_` form precisely so a host with a bounded
/// allocator can refuse; a request that cannot be served must come back as an
/// error.
#[test]
fn an_impossible_capacity_is_refused_rather_than_aborting() {
    // Deliberately absurd: larger than any address space this runs on.
    let r = Arena::try_with_capacity(usize::MAX / 2);
    assert!(
        r.is_err(),
        "an arena of half the address space was allocated, so this case did not reach \
         the refusal path it tests"
    );
}
