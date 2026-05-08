//! Simple and boring memory allocator for exciting applications.
//!
//! `keleusma-arena` is a dual-end bump-allocated arena for embedded Rust.
//! A single contiguous buffer holds two bump pointers that grow toward each
//! other from opposite ends. Allocation is constant-time. Allocation fails
//! when the two pointers would meet. The arena's discipline supports both
//! ad-hoc embedded use and static analysis of memory bounds.
//!
//! # Design
//!
//! The arena offers two ends, named `Bottom` and `Top`. The bottom end
//! starts at offset zero and grows upward toward higher addresses. The top
//! end starts at the buffer's high address and grows downward toward lower
//! addresses. The two ends share a single contiguous buffer with no fixed
//! boundary. Either may consume any portion that the other has not.
//!
//! These names are conventional. The arena imposes no semantic
//! distinction between the two ends. Users may map their own concepts to
//! the two ends as they see fit. A common mapping uses the bottom end for
//! a stack-like region whose allocations follow LIFO order and the top
//! end for a heap-like region whose allocations are reset together rather
//! than freed individually. The Keleusma scripting runtime, the original
//! consumer of this crate, follows this mapping. Independent users are
//! free to do otherwise.
//!
//! # Storage
//!
//! The arena's backing storage is supplied at construction. Three
//! constructors exist.
//!
//! - [`Arena::with_capacity`]. Allocates a `Box<[u8]>` from the global
//!   allocator. Available only with the `alloc` feature, which is on by
//!   default.
//! - [`Arena::from_static_buffer`]. Borrows a `&'static mut [u8]`. Safe.
//!   Suitable for embedded targets with link-time-allocated buffers, such
//!   as static arrays placed in BSS or DATA, or fixed memory regions on
//!   targets like the Game Boy Advance with IWRAM, EWRAM, and VRAM.
//! - [`Arena::from_buffer_unchecked`]. Accepts a raw pointer and length.
//!   Unsafe. The caller asserts that the buffer remains valid for the
//!   arena's lifetime and that no aliasing access occurs.
//!
//! The arena is `core`-compatible without `alloc` if only the static or
//! unsafe constructors are used.
//!
//! # Allocator Trait
//!
//! [`BottomHandle`] and [`TopHandle`] are zero-cost handles that borrow
//! an `Arena` and implement `allocator_api2::alloc::Allocator`. They can
//! be passed to `allocator_api2::vec::Vec::new_in` and similar
//! constructors to obtain arena-backed collections without changing the
//! collection's type beyond its allocator parameter.
//!
//! The arena is not thread-safe. Interior mutability uses `Cell<usize>`
//! for the bump pointers and peak watermarks rather than atomic
//! primitives.
//!
//! The arena is intended for scoped use through the `Allocator` trait.
//! Setting it as the program's `#[global_allocator]` is possible but
//! unusual and requires the user to wrap the arena in an interior-
//! mutability container with thread-safe access. The crate does not
//! provide such a wrapper.
//!
//! # Mark and Rewind
//!
//! Each end exposes a mark and rewind discipline for LIFO usage. The
//! safe [`Arena::bottom_mark`] and [`Arena::top_mark`] take a snapshot of
//! the current bump pointer. The unsafe [`Arena::rewind_bottom`] and
//! [`Arena::rewind_top`] restore an end to a previous mark. The unsafe
//! [`Arena::reset_bottom`] and [`Arena::reset_top`] clear one end while
//! leaving the other intact. The safe [`Arena::reset`] takes `&mut self`
//! and clears both ends; the borrow checker ensures no outstanding
//! handles exist at that point.
//!
//! Rewind and per-end reset are unsafe because they invalidate the
//! contents of the rewound region. Subsequent allocations may overwrite
//! data that the caller still references through raw pointers obtained
//! via the `Allocator` trait. Higher-level collections such as
//! `allocator_api2::vec::Vec` do not protect against this; the caller
//! must drop or otherwise abandon any such collection before calling
//! rewind.
//!
//! # Observability
//!
//! Each end tracks a peak watermark in addition to its current usage.
//! [`Arena::bottom_peak`] and [`Arena::top_peak`] return the highest
//! observed usage in bytes since the arena was created or since
//! [`Arena::clear_peaks`] was last called. [`Arena::bottom_used`],
//! [`Arena::top_used`], [`Arena::free`], and [`Arena::capacity`] report
//! current state. The peak watermarks are useful for sizing analysis,
//! namely determining the smallest arena that admits the program's
//! observed behavior across a representative workload.
//!
//! # Budget Contract
//!
//! [`Budget`] is a generic memory budget structure independent of any
//! specific bytecode or analysis. Producers compute a budget from
//! whatever analysis they choose. The arena's [`Arena::fits_budget`]
//! method checks whether a budget is admissible against the arena's
//! capacity. The Keleusma scripting runtime computes its budget through
//! a static analysis of bytecode and uses `fits_budget` to enforce the
//! bounded-memory guarantee at module load time. Independent users may
//! compute budgets through profiling, manual analysis, or any other
//! mechanism.

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::alloc::Layout;
use core::cell::Cell;
use core::ptr::NonNull;

use allocator_api2::alloc::{AllocError, Allocator};

/// A worst-case memory usage budget.
///
/// A producer-agnostic structure describing a worst-case stack and heap
/// memory bound. The arena's [`Arena::fits_budget`] method checks whether
/// the budget is admissible against the arena's capacity. The two bounds
/// must be non-overlapping in any single state of the arena, but they
/// represent peak usage of the two ends and so must sum within the
/// arena's capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Budget {
    /// Maximum bytes consumed at the bottom end.
    pub bottom_bytes: usize,
    /// Maximum bytes consumed at the top end.
    pub top_bytes: usize,
}

impl Budget {
    /// Construct a budget with the given bottom and top bounds.
    pub const fn new(bottom_bytes: usize, top_bytes: usize) -> Self {
        Self {
            bottom_bytes,
            top_bytes,
        }
    }

    /// Total bytes required by this budget. Saturates at `usize::MAX` on
    /// overflow so that an oversized budget does not silently wrap.
    pub const fn total(&self) -> usize {
        self.bottom_bytes.saturating_add(self.top_bytes)
    }
}

/// A mark for the bottom end of an arena.
///
/// Returned by [`Arena::bottom_mark`]. Pass back to
/// [`Arena::rewind_bottom`] to restore the bottom pointer to this
/// position. Marks are tied to the arena that produced them; passing a
/// mark to a different arena is a logic error and produces undefined
/// behavior under the unsafe rewind contract.
#[derive(Debug, Clone, Copy)]
pub struct BottomMark(usize);

/// A mark for the top end of an arena.
///
/// Returned by [`Arena::top_mark`]. Pass back to [`Arena::rewind_top`]
/// to restore the top pointer to this position.
#[derive(Debug, Clone, Copy)]
pub struct TopMark(usize);

/// Storage backing variants for an arena.
///
/// The variant is read implicitly through `Drop`. Owned variants release
/// their backing allocation when the arena drops; External variants do
/// nothing at drop time.
#[allow(dead_code)]
enum Storage {
    /// Externally owned buffer. The arena holds a raw pointer and length.
    /// The caller is responsible for keeping the buffer alive for the
    /// arena's lifetime.
    External,
    /// Owned buffer allocated through the global allocator. Dropped when
    /// the arena drops.
    #[cfg(feature = "alloc")]
    Owned(alloc::boxed::Box<[u8]>),
}

/// A dual-end bump-allocated arena.
///
/// Owns or borrows a fixed-size buffer of bytes. Two bump pointers track
/// allocation positions at each end. The bottom end grows from low
/// addresses upward. The top end grows from high addresses downward.
/// Allocation fails when the two pointers would meet.
///
/// See the crate-level documentation for the design overview.
pub struct Arena {
    /// Pointer to the start of the backing buffer. Stable for the
    /// arena's lifetime.
    buffer: NonNull<u8>,
    /// Total capacity of the buffer in bytes.
    capacity: usize,
    /// Current bottom pointer. Allocations from the bottom end consume
    /// the range `[0, bottom_top)`.
    bottom_top: Cell<usize>,
    /// Current top pointer. Allocations from the top end consume the
    /// range `[top_top, capacity)`.
    top_top: Cell<usize>,
    /// Peak observed value of `bottom_top`. Watermark for sizing
    /// analysis.
    bottom_peak: Cell<usize>,
    /// Lowest observed value of `top_top`. Combined with `capacity`
    /// gives the peak top usage.
    top_peak_low: Cell<usize>,
    /// Storage discriminator. The field is read implicitly via `Drop`.
    #[allow(dead_code)]
    storage: Storage,
}

// SAFETY: The arena uses `Cell` for interior mutability of the bump
// pointers and peaks. `Cell` is `Send` but not `Sync`. The arena itself
// is not `Sync` for the same reason.

impl Arena {
    /// Create an arena backed by a freshly allocated heap buffer of the
    /// given byte capacity.
    ///
    /// Available only with the `alloc` feature. The buffer is zeroed at
    /// construction.
    #[cfg(feature = "alloc")]
    pub fn with_capacity(capacity: usize) -> Self {
        let mut backing: alloc::boxed::Box<[u8]> = alloc::vec![0u8; capacity].into_boxed_slice();
        let ptr = backing.as_mut_ptr();
        // SAFETY: `ptr` is non-null because `backing` is a valid
        // allocation. The `Box` is held in `Storage::Owned` to keep the
        // buffer alive for the arena's lifetime.
        let buffer = unsafe { NonNull::new_unchecked(ptr) };
        Self {
            buffer,
            capacity,
            bottom_top: Cell::new(0),
            top_top: Cell::new(capacity),
            bottom_peak: Cell::new(0),
            top_peak_low: Cell::new(capacity),
            storage: Storage::Owned(backing),
        }
    }

    /// Create an arena backed by a static buffer.
    ///
    /// The buffer must outlive the arena. The `'static mut` requirement
    /// satisfies this for typical embedded patterns where the buffer is
    /// a static array placed in BSS or DATA. For shorter-lived buffers,
    /// see [`Arena::from_buffer_unchecked`].
    pub fn from_static_buffer(buffer: &'static mut [u8]) -> Self {
        let capacity = buffer.len();
        // SAFETY: `&'static mut [u8]` is non-null and lives for the
        // duration of the program.
        let ptr = unsafe { NonNull::new_unchecked(buffer.as_mut_ptr()) };
        Self {
            buffer: ptr,
            capacity,
            bottom_top: Cell::new(0),
            top_top: Cell::new(capacity),
            bottom_peak: Cell::new(0),
            top_peak_low: Cell::new(capacity),
            storage: Storage::External,
        }
    }

    /// Create an arena from a raw pointer and length.
    ///
    /// # Safety
    ///
    /// The caller must uphold the following.
    ///
    /// - `ptr` is non-null and aligned to at least byte alignment.
    /// - `ptr` is valid for reads and writes of `capacity` bytes for the
    ///   entire lifetime of the returned arena.
    /// - No other code accesses the buffer through any path that would
    ///   alias with the arena's allocations during the arena's lifetime.
    ///
    /// This constructor is the only path that admits buffers with
    /// non-`'static` lifetimes. It exists for embedded contexts where
    /// the lifetime is known statically through other means but the
    /// type system cannot express it. Most callers should prefer
    /// [`Arena::from_static_buffer`].
    pub unsafe fn from_buffer_unchecked(ptr: *mut u8, capacity: usize) -> Self {
        // SAFETY: Caller asserts non-null and validity. `NonNull::new_unchecked`
        // is sound under the caller's assertion.
        let buffer = unsafe { NonNull::new_unchecked(ptr) };
        Self {
            buffer,
            capacity,
            bottom_top: Cell::new(0),
            top_top: Cell::new(capacity),
            bottom_peak: Cell::new(0),
            top_peak_low: Cell::new(capacity),
            storage: Storage::External,
        }
    }

    /// Total capacity of the arena in bytes.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Bytes currently allocated from the bottom end.
    pub fn bottom_used(&self) -> usize {
        self.bottom_top.get()
    }

    /// Bytes currently allocated from the top end.
    pub fn top_used(&self) -> usize {
        self.capacity - self.top_top.get()
    }

    /// Bytes available for either end to consume.
    pub fn free(&self) -> usize {
        self.top_top.get().saturating_sub(self.bottom_top.get())
    }

    /// Highest observed bottom usage in bytes since arena creation or
    /// the most recent [`Arena::clear_peaks`] call.
    pub fn bottom_peak(&self) -> usize {
        self.bottom_peak.get()
    }

    /// Highest observed top usage in bytes since arena creation or the
    /// most recent [`Arena::clear_peaks`] call.
    pub fn top_peak(&self) -> usize {
        self.capacity - self.top_peak_low.get()
    }

    /// Return a snapshot of the bottom-end bump pointer for later use
    /// with [`Arena::rewind_bottom`].
    pub fn bottom_mark(&self) -> BottomMark {
        BottomMark(self.bottom_top.get())
    }

    /// Return a snapshot of the top-end bump pointer for later use with
    /// [`Arena::rewind_top`].
    pub fn top_mark(&self) -> TopMark {
        TopMark(self.top_top.get())
    }

    /// Reset both ends, reclaiming all allocations.
    ///
    /// Constant-time. Does not zero the buffer contents because
    /// subsequent allocations will overwrite as needed. Does not clear
    /// peak watermarks; use [`Arena::clear_peaks`] for that.
    ///
    /// Takes `&mut self` so the borrow checker prevents calling reset
    /// while any handle borrows the arena. This guarantees no live
    /// allocations through `Allocator` trait users at the moment of
    /// reset.
    pub fn reset(&mut self) {
        self.bottom_top.set(0);
        self.top_top.set(self.capacity);
    }

    /// Clear the peak watermarks for both ends.
    ///
    /// Sets each peak to the current pointer value. After this call,
    /// peak readings reflect only allocations made after the call.
    pub fn clear_peaks(&mut self) {
        self.bottom_peak.set(self.bottom_top.get());
        self.top_peak_low.set(self.top_top.get());
    }

    /// Rewind the bottom end to a previously recorded mark.
    ///
    /// # Safety
    ///
    /// The caller must ensure that no live values reference memory in
    /// the range `[mark.0, current_bottom_top)`. References obtained
    /// through the `Allocator` trait, including those held by
    /// `allocator_api2::vec::Vec` and similar collections, must be
    /// dropped or otherwise abandoned before this call. Subsequent
    /// allocations may overwrite the rewound region, which would alias
    /// with any retained reference and produce undefined behavior.
    ///
    /// Marks from a different arena are a logic error.
    pub unsafe fn rewind_bottom(&self, mark: BottomMark) {
        let target = mark.0.min(self.bottom_top.get());
        self.bottom_top.set(target);
    }

    /// Rewind the top end to a previously recorded mark.
    ///
    /// # Safety
    ///
    /// Same contract as [`Arena::rewind_bottom`].
    pub unsafe fn rewind_top(&self, mark: TopMark) {
        let target = mark.0.max(self.top_top.get());
        self.top_top.set(target);
    }

    /// Clear the bottom end without checking for live references.
    ///
    /// # Safety
    ///
    /// The caller must ensure no live references into the bottom region
    /// exist. Equivalent to [`Arena::rewind_bottom`] with a mark of
    /// zero, with the same safety contract.
    pub unsafe fn reset_bottom(&self) {
        self.bottom_top.set(0);
    }

    /// Clear the top end without checking for live references.
    ///
    /// # Safety
    ///
    /// The caller must ensure no live references into the top region
    /// exist. Equivalent to [`Arena::rewind_top`] with a mark of
    /// `capacity`, with the same safety contract.
    pub unsafe fn reset_top(&self) {
        self.top_top.set(self.capacity);
    }

    /// Returns true if the given budget fits within the arena's
    /// capacity. The check is `budget.bottom_bytes + budget.top_bytes
    /// <= capacity`.
    ///
    /// This is the generic budget contract referenced in the crate
    /// documentation. Producers compute a budget through whatever
    /// analysis they choose and use this method to verify admissibility
    /// before relying on the arena.
    pub fn fits_budget(&self, budget: &Budget) -> bool {
        budget.total() <= self.capacity
    }

    /// Obtain a bottom-end allocation handle.
    pub fn bottom_handle(&self) -> BottomHandle<'_> {
        BottomHandle(self)
    }

    /// Obtain a top-end allocation handle.
    pub fn top_handle(&self) -> TopHandle<'_> {
        TopHandle(self)
    }

    /// Allocate from the bottom end.
    fn alloc_bottom(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let cur = self.bottom_top.get();
        let align_mask = layout.align().saturating_sub(1);
        let aligned = cur.checked_add(align_mask).ok_or(AllocError)? & !align_mask;
        let new_top = aligned.checked_add(layout.size()).ok_or(AllocError)?;
        if new_top > self.top_top.get() {
            return Err(AllocError);
        }
        self.bottom_top.set(new_top);
        if new_top > self.bottom_peak.get() {
            self.bottom_peak.set(new_top);
        }
        // SAFETY: `aligned` is within `[0, capacity)` because it is at
        // most `top_top` which is at most `capacity`. The reserved range
        // `[aligned, new_top)` is exclusive to this allocation until
        // the next reset or rewind.
        let ptr = unsafe { self.buffer.as_ptr().add(aligned) };
        let slice = core::ptr::slice_from_raw_parts_mut(ptr, layout.size());
        NonNull::new(slice).ok_or(AllocError)
    }

    /// Allocate from the top end.
    fn alloc_top(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let cur = self.top_top.get();
        let new_end = cur.checked_sub(layout.size()).ok_or(AllocError)?;
        let align_mask = layout.align().saturating_sub(1);
        let aligned_start = new_end & !align_mask;
        if aligned_start < self.bottom_top.get() {
            return Err(AllocError);
        }
        self.top_top.set(aligned_start);
        if aligned_start < self.top_peak_low.get() {
            self.top_peak_low.set(aligned_start);
        }
        // SAFETY: `aligned_start` is within `[bottom_top, capacity)` and
        // the reserved range `[aligned_start, aligned_start + size)` is
        // exclusive to this allocation until the next reset or rewind.
        let ptr = unsafe { self.buffer.as_ptr().add(aligned_start) };
        let slice = core::ptr::slice_from_raw_parts_mut(ptr, layout.size());
        NonNull::new(slice).ok_or(AllocError)
    }
}

impl core::fmt::Debug for Arena {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Arena")
            .field("capacity", &self.capacity)
            .field("bottom_used", &self.bottom_used())
            .field("top_used", &self.top_used())
            .field("free", &self.free())
            .field("bottom_peak", &self.bottom_peak())
            .field("top_peak", &self.top_peak())
            .finish()
    }
}

// The `storage` field handles drop. `External` does nothing. `Owned`
// drops the held `Box`, which deallocates the buffer. No additional
// `Drop` impl is needed.

/// Allocation handle for the bottom end of an arena.
///
/// Implements `allocator_api2::alloc::Allocator`. Use with constructors
/// such as `allocator_api2::vec::Vec::new_in(arena.bottom_handle())`.
#[derive(Clone, Copy, Debug)]
pub struct BottomHandle<'a>(&'a Arena);

/// Allocation handle for the top end of an arena.
///
/// Implements `allocator_api2::alloc::Allocator`. Use with constructors
/// such as `allocator_api2::vec::Vec::new_in(arena.top_handle())`.
#[derive(Clone, Copy, Debug)]
pub struct TopHandle<'a>(&'a Arena);

// SAFETY: The arena's allocation methods uphold the `Allocator`
// contract. Returned pointers are valid for the requested layout,
// unique to the caller, and remain valid until the next reset or
// rewind. Deallocation is a no-op because the bump allocator reclaims
// memory in bulk.
unsafe impl Allocator for BottomHandle<'_> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.0.alloc_bottom(layout)
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        // No-op. Bump allocator reclaims at reset.
    }
}

// SAFETY: Same reasoning as `BottomHandle`.
unsafe impl Allocator for TopHandle<'_> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.0.alloc_top(layout)
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        // No-op. Bump allocator reclaims at reset.
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc as test_alloc;

    use super::*;
    use allocator_api2::vec::Vec as ArenaVec;

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_with_capacity() {
        let arena = Arena::with_capacity(1024);
        assert_eq!(arena.capacity(), 1024);
        assert_eq!(arena.bottom_used(), 0);
        assert_eq!(arena.top_used(), 0);
        assert_eq!(arena.free(), 1024);
        assert_eq!(arena.bottom_peak(), 0);
        assert_eq!(arena.top_peak(), 0);
    }

    #[test]
    fn arena_from_static_buffer() {
        // Use a leaked Box for a 'static-like buffer in tests. In real
        // embedded use, this would be a `static mut [u8; N]`.
        let leaked: &'static mut [u8] = test_alloc::vec![0u8; 256].leak();
        let arena = Arena::from_static_buffer(leaked);
        assert_eq!(arena.capacity(), 256);
        let layout = Layout::new::<u64>();
        let _p = arena.bottom_handle().allocate(layout).unwrap();
        assert_eq!(arena.bottom_used(), 8);
    }

    #[test]
    fn arena_from_buffer_unchecked() {
        let mut buffer = test_alloc::vec![0u8; 128];
        let ptr = buffer.as_mut_ptr();
        let len = buffer.len();
        // SAFETY: `buffer` outlives the arena because we hold it until
        // the test ends, and we do not access it through `buffer` while
        // the arena is in use.
        let arena = unsafe { Arena::from_buffer_unchecked(ptr, len) };
        assert_eq!(arena.capacity(), 128);
        let layout = Layout::new::<u32>();
        let _p = arena.bottom_handle().allocate(layout).unwrap();
        assert_eq!(arena.bottom_used(), 4);
        drop(arena);
        // `buffer` is still alive here.
        assert_eq!(buffer.len(), 128);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_dual_end() {
        let arena = Arena::with_capacity(64);
        let layout = Layout::new::<u64>();
        let _b = arena.bottom_handle().allocate(layout).unwrap();
        let _t = arena.top_handle().allocate(layout).unwrap();
        assert_eq!(arena.bottom_used(), 8);
        assert_eq!(arena.top_used(), 8);
        assert_eq!(arena.free(), 48);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_alignment() {
        let arena = Arena::with_capacity(64);
        let _byte = arena.bottom_handle().allocate(Layout::new::<u8>()).unwrap();
        let p_u64 = arena
            .bottom_handle()
            .allocate(Layout::new::<u64>())
            .unwrap();
        let addr = p_u64.as_ptr() as *const u8 as usize;
        assert_eq!(addr % 8, 0);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_exhaustion() {
        let arena = Arena::with_capacity(16);
        let layout = Layout::new::<u64>();
        let _a = arena.bottom_handle().allocate(layout).unwrap();
        let _b = arena.bottom_handle().allocate(layout).unwrap();
        assert!(arena.bottom_handle().allocate(layout).is_err());
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_reset() {
        let mut arena = Arena::with_capacity(64);
        let layout = Layout::new::<u64>();
        {
            let _b = arena.bottom_handle().allocate(layout).unwrap();
            let _t = arena.top_handle().allocate(layout).unwrap();
        }
        assert_eq!(arena.bottom_used(), 8);
        assert_eq!(arena.top_used(), 8);
        arena.reset();
        assert_eq!(arena.bottom_used(), 0);
        assert_eq!(arena.top_used(), 0);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_peak_tracking() {
        let arena = Arena::with_capacity(128);
        let layout = Layout::new::<u64>();
        let mark = arena.bottom_mark();
        let _a = arena.bottom_handle().allocate(layout).unwrap();
        let _b = arena.bottom_handle().allocate(layout).unwrap();
        assert_eq!(arena.bottom_peak(), 16);
        // Rewind reduces current usage but not the peak.
        // SAFETY: Drops happen at scope end, and we are about to
        // re-allocate. The peak observation is from before any rewind.
        unsafe {
            arena.rewind_bottom(mark);
        }
        assert_eq!(arena.bottom_used(), 0);
        assert_eq!(arena.bottom_peak(), 16);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_clear_peaks() {
        let mut arena = Arena::with_capacity(64);
        let layout = Layout::new::<u64>();
        let _a = arena.bottom_handle().allocate(layout).unwrap();
        assert_eq!(arena.bottom_peak(), 8);
        arena.reset();
        assert_eq!(arena.bottom_used(), 0);
        // Peak persists after reset.
        assert_eq!(arena.bottom_peak(), 8);
        arena.clear_peaks();
        assert_eq!(arena.bottom_peak(), 0);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_mark_rewind() {
        let arena = Arena::with_capacity(128);
        let layout = Layout::new::<u32>();
        let mark = arena.bottom_mark();
        let _a = arena.bottom_handle().allocate(layout).unwrap();
        let _b = arena.bottom_handle().allocate(layout).unwrap();
        assert_eq!(arena.bottom_used(), 8);
        // SAFETY: We have not retained any references to the
        // allocations beyond this scope. The handles' allocations are
        // raw pointers that we are not using past this point.
        unsafe {
            arena.rewind_bottom(mark);
        }
        assert_eq!(arena.bottom_used(), 0);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_per_end_reset() {
        let arena = Arena::with_capacity(64);
        let layout = Layout::new::<u64>();
        let _b = arena.bottom_handle().allocate(layout).unwrap();
        let _t = arena.top_handle().allocate(layout).unwrap();
        // SAFETY: No retained allocations.
        unsafe {
            arena.reset_bottom();
        }
        assert_eq!(arena.bottom_used(), 0);
        assert_eq!(arena.top_used(), 8);
        // SAFETY: No retained allocations.
        unsafe {
            arena.reset_top();
        }
        assert_eq!(arena.top_used(), 0);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_vec_integration() {
        let arena = Arena::with_capacity(2048);
        let mut v: ArenaVec<i64, _> = ArenaVec::new_in(arena.bottom_handle());
        for i in 0..10 {
            v.push(i);
        }
        assert_eq!(v.iter().sum::<i64>(), 45);
        assert!(arena.bottom_used() > 0);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_dual_vec_integration() {
        let arena = Arena::with_capacity(4096);
        let mut bot: ArenaVec<i64, _> = ArenaVec::new_in(arena.bottom_handle());
        let mut top: ArenaVec<i64, _> = ArenaVec::new_in(arena.top_handle());
        for i in 0..5 {
            bot.push(i);
            top.push(i * 100);
        }
        assert_eq!(bot.len(), 5);
        assert_eq!(top.len(), 5);
        assert!(arena.bottom_used() > 0);
        assert!(arena.top_used() > 0);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn budget_fits() {
        let arena = Arena::with_capacity(1024);
        assert!(arena.fits_budget(&Budget::new(512, 256)));
        assert!(arena.fits_budget(&Budget::new(0, 0)));
        assert!(arena.fits_budget(&Budget::new(1024, 0)));
        assert!(!arena.fits_budget(&Budget::new(513, 512)));
        assert!(!arena.fits_budget(&Budget::new(usize::MAX, 1)));
    }

    #[test]
    fn budget_total_saturates() {
        let b = Budget::new(usize::MAX, 1);
        assert_eq!(b.total(), usize::MAX);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_zero_capacity() {
        let arena = Arena::with_capacity(0);
        assert!(arena.bottom_handle().allocate(Layout::new::<u8>()).is_err());
        assert!(arena.fits_budget(&Budget::new(0, 0)));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_zero_size_layout() {
        let arena = Arena::with_capacity(64);
        let layout = Layout::new::<()>();
        assert!(arena.bottom_handle().allocate(layout).is_ok());
        assert_eq!(arena.bottom_used(), 0);
    }
}
