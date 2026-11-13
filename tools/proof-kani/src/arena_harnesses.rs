//! Harnesses over the `keleusma-arena` allocator, against the safety
//! contract of the `Allocator` trait.
//!
//! The arena is a two-ended bump allocator over one buffer. Its
//! `BottomHandle` and `TopHandle` implement `allocator_api2::alloc::Allocator`,
//! whose contract the Rust standard library stabilised in Rust 1.100 as
//! `std::alloc::Allocator`. The parts of that contract checked here are the
//! ones a caller relies on.
//!
//! - A returned block fits its layout. Its length is the requested size and
//!   its address is a multiple of the requested alignment.
//! - A returned block is valid memory of this arena, outside the persistent
//!   region, and does not overlap any other live block.
//! - `grow` preserves the contents of the block it grows.
//! - A failed allocation leaves the arena unchanged.
//!
//! The arena's own accounting invariant is checked alongside, that the bottom
//! head never sits below the persistent boundary or above the top head, by
//! calling the public accessors that subtract those quantities. Kani checks
//! every subtraction for overflow, so an accessor called on a broken state
//! fails the harness.
//!
//! Every harness reaches the arena through its public application programming
//! interface only, because the crate belongs to the V0.2.X line.

use allocator_api2::alloc::Allocator;
use core::alloc::Layout;
use core::ptr::NonNull;
use keleusma_arena::Arena;

/// The buffer size. Small, so the model checker can enumerate every offset.
const CAP: usize = 24;

/// The largest alignment exponent chosen, so alignments run from 1 to 32.
/// The buffer is aligned to 32, so every alignment up to its own is possible
/// at offset zero, and an alignment of 32 can still fail at a later offset.
const MAX_ALIGN_LOG2: u32 = 5;

/// A buffer whose base address is a multiple of 32.
#[repr(C, align(32))]
struct Buf([u8; CAP]);

/// An arena over `buf`. The buffer outlives the arena in every harness,
/// because it is declared first and dropped last, which is the contract of
/// `from_buffer_unchecked`.
fn arena_over(buf: &mut Buf) -> Arena {
    // SAFETY: `buf` is non-null, valid for `CAP` bytes, outlives the arena,
    // and is reached by nothing else while the arena lives.
    unsafe { Arena::from_buffer_unchecked(buf.0.as_mut_ptr(), CAP) }
}

/// Any layout with a size up to one past the buffer, so that a request too
/// large to fit is among the cases, and any alignment from 1 to 32.
fn any_layout() -> Layout {
    let size: usize = kani::any();
    kani::assume(size <= CAP + 1);
    let k: u32 = kani::any();
    kani::assume(k <= MAX_ALIGN_LOG2);
    match Layout::from_size_align(size, 1usize << k) {
        Ok(layout) => layout,
        // Unreachable for these sizes and alignments. Assumed away rather
        // than asserted, since only the arena is under check here.
        Err(_) => {
            kani::assume(false);
            Layout::new::<u8>()
        }
    }
}

/// Allocates `layout` from the bottom end when `bottom`, and from the top end
/// otherwise, through the `Allocator` trait.
fn allocate(arena: &Arena, bottom: bool, layout: Layout) -> Option<NonNull<[u8]>> {
    let result = if bottom {
        arena.bottom_handle().allocate(layout)
    } else {
        arena.top_handle().allocate(layout)
    };
    result.ok()
}

/// The arena's accounting invariant through its public accessors. Each of
/// `bottom_used`, `top_used` and `dual_headed_capacity` subtracts, so a call
/// on a state with the bottom head below the persistent boundary, or the top
/// head above the capacity, fails on overflow before the assertion.
fn assert_accounting(arena: &Arena) {
    let total = arena.bottom_used() + arena.top_used() + arena.free();
    assert!(
        total == arena.dual_headed_capacity(),
        "used and free bytes must sum to the dual-headed capacity"
    );
}

/// The block fits `layout` and lies inside the arena's ephemeral region.
/// Returns its start offset from `base`.
fn assert_block(arena: &Arena, base: usize, block: NonNull<[u8]>, layout: Layout) -> usize {
    let start = block.as_ptr() as *mut u8 as usize;
    assert!(block.len() == layout.size(), "a block has the requested size");
    assert!(start % layout.align() == 0, "a block has the requested alignment");
    assert!(
        start >= base + arena.persistent_capacity(),
        "a block lies above the persistent region"
    );
    assert!(
        start + layout.size() <= base + arena.capacity(),
        "a block lies inside the buffer"
    );
    start - base
}

/// One allocation from either end, after an arbitrary earlier allocation and
/// with an arbitrary persistent region, fits its layout and lies inside the
/// ephemeral region. A failed allocation changes nothing.
#[kani::proof]
#[kani::unwind(3)]
fn arena_allocate_meets_layout() {
    let mut buf = Buf([0; CAP]);
    let base = buf.0.as_ptr() as usize;
    let mut arena = arena_over(&mut buf);
    let persistent: usize = kani::any();
    kani::assume(persistent <= CAP);
    assert!(arena.resize_persistent(persistent).is_ok());

    // An earlier allocation, from either end, so that the checked one starts
    // from an arbitrary reachable state rather than an empty arena.
    let _ = allocate(&arena, kani::any(), any_layout());
    assert_accounting(&arena);

    let bottom_before = arena.bottom_used();
    let top_before = arena.top_used();
    let layout = any_layout();
    match allocate(&arena, kani::any(), layout) {
        Some(block) => {
            let _ = assert_block(&arena, base, block, layout);
        }
        None => {
            assert!(arena.bottom_used() == bottom_before, "a failure leaves the bottom unchanged");
            assert!(arena.top_used() == top_before, "a failure leaves the top unchanged");
        }
    }
    assert_accounting(&arena);
}

/// Three allocations, each from either end with any layout, give blocks that
/// pairwise do not overlap. Blocks of size zero are excluded, because they
/// occupy no bytes and may share an address with a neighbour.
#[kani::proof]
#[kani::unwind(4)]
fn arena_allocations_are_disjoint() {
    let mut buf = Buf([0; CAP]);
    let base = buf.0.as_ptr() as usize;
    let arena = arena_over(&mut buf);
    let mut blocks: [(usize, usize); 3] = [(0, 0); 3];
    let mut i = 0;
    while i < 3 {
        let layout = any_layout();
        if let Some(block) = allocate(&arena, kani::any(), layout) {
            let start = assert_block(&arena, base, block, layout);
            blocks[i] = (start, layout.size());
        }
        assert_accounting(&arena);
        i += 1;
    }
    let mut a = 0;
    while a < 3 {
        let mut b = a + 1;
        while b < 3 {
            let (sa, la) = blocks[a];
            let (sb, lb) = blocks[b];
            if la > 0 && lb > 0 {
                assert!(sa + la <= sb || sb + lb <= sa, "live blocks never overlap");
            }
            b += 1;
        }
        a += 1;
    }
}

/// `grow` through the bottom handle keeps the contents of the block it grows.
/// The arena does not override the trait's `grow`, so this checks the
/// default method together with the arena's `allocate`.
#[kani::proof]
#[kani::unwind(26)]
fn arena_grow_preserves_contents() {
    let mut buf = Buf([0; CAP]);
    let arena = arena_over(&mut buf);
    let handle = arena.bottom_handle();
    let old = any_layout();
    let Some(block) = allocate(&arena, true, old) else {
        return;
    };
    let ptr = block.as_ptr() as *mut u8;
    let mut written = [0u8; CAP + 1];
    let mut i = 0;
    while i < old.size() {
        let byte: u8 = kani::any();
        written[i] = byte;
        // SAFETY: `i < old.size()`, and the block is valid for that many bytes.
        unsafe { ptr.add(i).write(byte) };
        i += 1;
    }
    let new_size: usize = kani::any();
    kani::assume(new_size >= old.size() && new_size <= CAP + 1);
    let Ok(new) = Layout::from_size_align(new_size, old.align()) else {
        return;
    };
    // SAFETY: `block` was allocated by this handle with layout `old`, and
    // `new.size() >= old.size()`, which is the precondition of `grow`.
    let Ok(grown) = (unsafe { handle.grow(NonNull::new_unchecked(ptr), old, new) }) else {
        return;
    };
    assert!(grown.len() >= new.size(), "a grown block has at least the new size");
    let gptr = grown.as_ptr() as *mut u8;
    let mut j = 0;
    while j < old.size() {
        // SAFETY: `j < old.size() <= grown.len()`.
        let byte = unsafe { gptr.add(j).read() };
        assert!(byte == written[j], "grow preserves the old contents");
        j += 1;
    }
    assert_accounting(&arena);
}

/// A rewind to a mark taken before an allocation restores the bottom usage,
/// and a reset empties both ends and advances the epoch by one.
#[kani::proof]
#[kani::unwind(3)]
fn arena_rewind_and_reset_restore() {
    let mut buf = Buf([0; CAP]);
    let mut arena = arena_over(&mut buf);
    let _ = allocate(&arena, true, any_layout());
    let before = arena.bottom_used();
    let mark = arena.bottom_mark();
    let _ = allocate(&arena, true, any_layout());
    // SAFETY: no reference into the bottom region above `mark` is kept.
    unsafe { arena.rewind_bottom(mark) };
    assert!(arena.bottom_used() == before, "a rewind restores the bottom usage");
    assert_accounting(&arena);

    let _ = allocate(&arena, false, any_layout());
    let epoch = arena.epoch();
    assert!(arena.reset().is_ok());
    assert!(arena.bottom_used() == 0, "a reset empties the bottom");
    assert!(arena.top_used() == 0, "a reset empties the top");
    assert!(arena.epoch() == epoch + 1, "a reset advances the epoch by one");
    assert_accounting(&arena);
}

/// Negative control for this family. Claims that two successive bottom
/// allocations of at least one byte start at the same address, which the
/// disjointness harness shows false. Kani must report the assertion as
/// failed, or the arena harnesses are not checking the code they name.
#[kani::proof]
#[kani::unwind(3)]
fn negative_control_arena_overlap() {
    let mut buf = Buf([0; CAP]);
    let arena = arena_over(&mut buf);
    let layout = Layout::from_size_align(4, 1).unwrap();
    let first = allocate(&arena, true, layout);
    let second = allocate(&arena, true, layout);
    if let (Some(a), Some(b)) = (first, second) {
        let sa = a.as_ptr() as *mut u8 as usize;
        let sb = b.as_ptr() as *mut u8 as usize;
        assert!(sa == sb, "negative control: this claim is false");
    }
}

/// The arena state of finding A1. A bottom mark is taken on an empty arena,
/// up to the whole buffer is allocated from the bottom, the persistent region
/// is grown to any size the arena accepts, and the old mark is passed to
/// `rewind_bottom`. Returns `None` where the grow is refused.
///
/// `rewind_bottom` sets the bottom head to the lesser of the mark and the
/// current head, and never to less than the persistent boundary. Its safety
/// contract asks only that no live value lie in the range rewound over, which
/// an unused persistent region satisfies, so this sequence meets that
/// contract.
fn a1_state(buf: &mut Buf) -> Option<Arena> {
    let mut arena = arena_over(buf);
    let mark = arena.bottom_mark();
    let n: usize = kani::any();
    kani::assume(n <= CAP);
    let _ = arena.alloc_bottom_bytes(n);
    let grow: usize = kani::any();
    kani::assume(grow <= CAP);
    if arena.resize_persistent_capacity(grow).is_err() {
        return None;
    }
    // SAFETY: nothing is kept that references the bottom region or the
    // persistent region, so no live value lies in the rewound range.
    unsafe { arena.rewind_bottom(mark) };
    Some(arena)
}

/// Finding A1, the accounting. After the sequence of [`a1_state`],
/// `bottom_used` subtracts the persistent boundary from a bottom head below
/// it. Expected to fail with an overflow in `Arena::bottom_used`, which wraps
/// to a huge figure in a build without overflow checks.
#[kani::proof]
#[kani::unwind(3)]
fn finding_a1_rewind_below_persistent_accounting() {
    let mut buf = Buf([0; CAP]);
    if let Some(arena) = a1_state(&mut buf) {
        assert_accounting(&arena);
    }
}

/// Finding A1, the placement. After the sequence of [`a1_state`], a one-byte
/// bottom allocation is placed below the persistent boundary, so it would
/// overwrite persistent data that the arena documents as preserved across
/// every reset. Expected to fail at the assertion in this harness, which is
/// the persistent-region check of [`assert_block`] stated alone so that the
/// required failure names exactly that check.
#[kani::proof]
#[kani::unwind(3)]
fn finding_a1_rewind_below_persistent_placement() {
    let mut buf = Buf([0; CAP]);
    let base = buf.0.as_ptr() as usize;
    if let Some(arena) = a1_state(&mut buf) {
        let layout = Layout::from_size_align(1, 1).unwrap();
        if let Some(block) = allocate(&arena, true, layout) {
            let start = block.as_ptr() as *mut u8 as usize;
            assert!(
                start >= base + arena.persistent_capacity(),
                "finding A1: a block lies below the persistent boundary"
            );
        }
    }
}
