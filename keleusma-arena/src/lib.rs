#![doc = include_str!("../README.md")]
//!
//! # API Reference
//!
//! ## Construction
//!
//! - [`Arena::with_capacity`]. Heap-backed. Requires the `alloc` feature.
//! - [`Arena::from_static_buffer`]. Borrows a `&'static mut [u8]`. Safe.
//! - [`Arena::from_buffer_unchecked`]. Raw pointer and length. Unsafe.
//!
//! ## Allocation
//!
//! [`BottomHandle`] and [`TopHandle`] borrow the arena and implement
//! `allocator_api2::alloc::Allocator`. Pass them to `Vec::new_in` and
//! similar constructors for arena-backed collections. The bottom end
//! starts at offset zero and grows upward. The top end starts at the
//! buffer's high address and grows downward. The arena imposes no
//! semantic distinction between the two ends.
//!
//! Code that prefers a CPU-memory mental model may use the method
//! aliases [`Arena::stack_handle`] and [`Arena::heap_handle`], which
//! return the same `BottomHandle` and `TopHandle` types under
//! conventional names.
//!
//! Aligned allocations go through the `Allocator` trait with a
//! `Layout` that carries the desired alignment. Alignment is computed
//! against the actual buffer base address, so any base alignment is
//! supported. Unaligned byte allocations have direct convenience
//! methods [`Arena::alloc_bottom_bytes`] and [`Arena::alloc_top_bytes`]
//! that allocate `n` bytes without padding for alignment. Use the
//! aligned form for typed values and pointers. Use the byte form for
//! packed byte buffers.
//!
//! ## Reset, Rewind, and Marks
//!
//! [`Arena::reset`] takes `&mut self` and clears both ends safely. Each
//! end also exposes a LIFO mark and rewind discipline. The mark
//! accessors [`Arena::bottom_mark`] and [`Arena::top_mark`] are safe.
//! The rewind and per-end reset operations [`Arena::rewind_bottom`],
//! [`Arena::rewind_top`], [`Arena::reset_bottom`], and
//! [`Arena::reset_top`] are unsafe because they invalidate the rewound
//! region while raw pointers obtained through the `Allocator` trait may
//! still be held by the caller.
//!
//! ## Observability and Budget
//!
//! [`Arena::bottom_peak`] and [`Arena::top_peak`] track high watermarks
//! since arena creation or the most recent [`Arena::clear_peaks`].
//! [`Arena::bottom_used`], [`Arena::top_used`], [`Arena::free`], and
//! [`Arena::capacity`] report current state.
//!
//! [`Budget`] is a generic memory budget structure. Producers compute a
//! budget through any analysis they choose. [`Arena::fits_budget`]
//! checks whether the budget is admissible against the arena's capacity.
//!
//! ## Thread Safety
//!
//! Not thread-safe. Interior mutability uses `Cell<usize>` rather than
//! atomic primitives. The arena is designed for scoped per-thread use
//! through the `Allocator` trait. Setting it as the program's
//! `#[global_allocator]` requires a thread-safe wrapper that this crate
//! does not provide.

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
/// The arena holds the raw pointer and capacity directly in the
/// `buffer` and `capacity` fields. The variant tracks ownership for
/// the explicit `Drop` impl on `Arena`. Owned arenas reconstruct the
/// `Box` via `Box::from_raw` and let it drop, releasing the buffer.
/// External arenas leave the buffer untouched; the caller owns the
/// storage.
///
/// Using a raw pointer rather than holding the `Box` directly gives
/// the buffer "raw" provenance from the perspective of the borrow
/// checker and miri's aliasing models. This is necessary because
/// allocations through `BottomHandle` and `TopHandle` derive write
/// pointers into the buffer through a shared `&Arena`; deriving
/// through a unique-reference ancestor would make subsequent
/// derivations from the same source aliasing-unsound under both
/// stacked borrows and tree borrows.
#[derive(Clone, Copy)]
enum Storage {
    /// Externally owned buffer. The caller is responsible for keeping
    /// the buffer alive for the arena's lifetime.
    External,
    /// Owned buffer allocated through the global allocator. The arena
    /// reconstructs the `Box` and drops it on its own `Drop`.
    #[cfg(feature = "alloc")]
    Owned,
}

/// A dual-end bump-allocated arena.
///
/// Owns or borrows a fixed-size buffer of bytes. Two bump pointers track
/// allocation positions at each end. The bottom end grows from low
/// addresses upward. The top end grows from high addresses downward.
/// Allocation fails when the two pointers would meet.
///
/// The arena is not the program's `#[global_allocator]` and is not
/// intended to be one. It is designed for scoped per-region or
/// per-thread use through `BottomHandle` and `TopHandle`, which the
/// host passes to allocator-aware collection constructors. The standard
/// global allocator continues to handle every allocation that does not
/// route through an arena handle. Hosts that want every allocation in
/// the program to be arena-backed must wrap the arena in a thread-safe
/// allocator and install it via `#[global_allocator]`; this crate does
/// not provide such a wrapper because doing so well requires choices
/// that depend on the host's threading and synchronization model.
///
/// ## Generations and stale-pointer detection
///
/// The arena carries an `epoch` counter that increments on [`Arena::reset`].
/// The `ArenaHandle` family of safe wrappers captures the epoch at
/// construction and validates it on access, returning [`Stale`] if the
/// arena has been reset since the handle was issued. The counter is
/// `u64` and uses checked arithmetic. A saturated counter halts the
/// arena's reset path with [`EpochSaturated`]. Saturation requires
/// roughly five hundred eighty four thousand years at one reset per
/// microsecond and is documentation rather than a real failure mode in
/// expected use.
///
/// In-process recovery from saturation is possible through
/// [`Arena::force_reset_epoch`], which is unsafe and requires the
/// caller to certify that no `ArenaHandle` from any prior epoch is
/// reachable. Cross-process recovery for very long-lived deployments
/// uses checkpoint and restart against host-owned non-volatile storage.
/// `ArenaHandle` is intentionally not serializable because its pointer
/// is not stable across processes.
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
    /// Generation counter. Incremented on [`Arena::reset`]. Captured by
    /// [`ArenaHandle`] values and validated on access for stale-pointer
    /// detection. Saturates at `u64::MAX`, at which point further
    /// resets fail with [`EpochSaturated`] until the caller invokes
    /// [`Arena::force_reset_epoch`].
    epoch: Cell<u64>,
    /// Storage discriminator. The field is read implicitly via `Drop`.
    #[allow(dead_code)]
    storage: Storage,
}

/// Hard halt error returned by [`Arena::reset`] when the epoch counter
/// would saturate.
///
/// Saturation requires roughly five hundred eighty four thousand years
/// at one reset per microsecond, but explicit refusal at saturation is
/// the correct posture for safety-critical use. Recovery is via
/// [`Arena::force_reset_epoch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpochSaturated;

/// Error returned by [`ArenaHandle::get`] when the arena has been
/// reset since the handle was issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stale;

// SAFETY: The arena uses `Cell` for interior mutability of the bump
// pointers and peaks. `Cell` is `Send` but not `Sync`. The arena itself
// is not `Sync` for the same reason.

impl Arena {
    /// Create an arena backed by a freshly allocated heap buffer of the
    /// given byte capacity.
    ///
    /// Available only with the `alloc` feature. The buffer is zeroed at
    /// construction and is allocated with 16-byte alignment, which
    /// covers the alignment requirements of `i64`, `f64`, `u128`, and
    /// most platform-native pointers and primitives.
    ///
    /// Panics on allocation failure via the standard `handle_alloc_error`
    /// path. A capacity of zero produces an arena that satisfies
    /// allocation requests for zero-size layouts only; non-zero
    /// allocations return `AllocError`.
    ///
    /// # Examples
    ///
    /// ```
    /// use allocator_api2::vec::Vec as ArenaVec;
    /// use keleusma_arena::Arena;
    ///
    /// let arena = Arena::with_capacity(1024);
    /// let mut v: ArenaVec<i64, _> = ArenaVec::new_in(arena.stack_handle());
    /// v.push(1);
    /// v.push(2);
    /// v.push(3);
    /// assert_eq!(v.len(), 3);
    /// assert!(arena.bottom_used() >= 24);
    /// ```
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn with_capacity(capacity: usize) -> Self {
        use alloc::alloc::{Layout as AllocLayout, alloc_zeroed, handle_alloc_error};

        let buffer = if capacity == 0 {
            NonNull::<u8>::dangling()
        } else {
            // Allocate a 16-byte-aligned buffer. The alignment covers
            // every standard primitive type and gives the arena
            // predictable behavior across allocators that may otherwise
            // return only minimally-aligned memory for byte allocations.
            let layout = AllocLayout::from_size_align(capacity, 16).expect("invalid arena layout");
            // SAFETY: `layout` has non-zero size because `capacity > 0`.
            let raw = unsafe { alloc_zeroed(layout) };
            if raw.is_null() {
                handle_alloc_error(layout);
            }
            // SAFETY: `alloc_zeroed` returned non-null on success.
            unsafe { NonNull::new_unchecked(raw) }
        };
        Self {
            buffer,
            capacity,
            bottom_top: Cell::new(0),
            top_top: Cell::new(capacity),
            bottom_peak: Cell::new(0),
            top_peak_low: Cell::new(capacity),
            epoch: Cell::new(0),
            storage: Storage::Owned,
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
            epoch: Cell::new(0),
            storage: Storage::External,
        }
    }

    /// Create an arena from a raw pointer and length.
    ///
    /// The buffer's base alignment does not need to match the alignment
    /// of any particular allocation type. The arena computes alignment
    /// against the actual buffer base address and pads as needed for
    /// each aligned allocation.
    ///
    /// # Safety
    ///
    /// The caller must uphold the following.
    ///
    /// - `ptr` is non-null.
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
            epoch: Cell::new(0),
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
    /// Advances the epoch counter, invalidating every outstanding
    /// [`ArenaHandle`]. Returns [`EpochSaturated`] if the counter is
    /// already at `u64::MAX`. See [`Arena::force_reset_epoch`] for
    /// recovery.
    ///
    /// Takes `&mut self` so the borrow checker prevents calling reset
    /// while any handle borrows the arena. This guarantees no live
    /// allocations through `Allocator` trait users at the moment of
    /// reset.
    pub fn reset(&mut self) -> Result<(), EpochSaturated> {
        let next = self.epoch.get().checked_add(1).ok_or(EpochSaturated)?;
        self.bottom_top.set(0);
        self.top_top.set(self.capacity);
        self.epoch.set(next);
        Ok(())
    }

    /// Reset both ends and advance the epoch through a shared reference.
    ///
    /// Companion to [`Arena::reset`] for callers that hold the arena
    /// through a shared reference and cannot temporarily acquire
    /// exclusive access. The interior-mutable bump pointers and epoch
    /// counter make the implementation race-free for single-threaded
    /// use.
    ///
    /// # Safety
    ///
    /// The caller must certify that no allocator-bound collection
    /// holds storage in the arena at the moment of reset. Concretely,
    /// no `allocator_api2::vec::Vec<T, BottomHandle>` or
    /// `allocator_api2::vec::Vec<T, TopHandle>` value may have non-zero
    /// capacity when this is called. Outstanding [`ArenaHandle`] values
    /// are correctly invalidated by the epoch advance and remain safe.
    ///
    /// Returns [`EpochSaturated`] when the epoch counter is at
    /// `u64::MAX`. Recovery is via [`Arena::force_reset_epoch`].
    pub unsafe fn reset_unchecked(&self) -> Result<(), EpochSaturated> {
        let next = self.epoch.get().checked_add(1).ok_or(EpochSaturated)?;
        self.bottom_top.set(0);
        self.top_top.set(self.capacity);
        self.epoch.set(next);
        Ok(())
    }

    /// Reset the top end and advance the epoch through a shared
    /// reference, leaving the bottom end untouched.
    ///
    /// Intended for hosts that use the bottom end for long-lived
    /// allocator-bound collections (such as an operand stack) while
    /// using the top end for short-lived scratch (such as dynamic
    /// strings). The epoch advance invalidates every outstanding
    /// [`ArenaHandle`] regardless of which end produced it. This is
    /// the desired discipline because handles do not record which end
    /// they came from and any handle that survives a reset is by
    /// definition stale.
    ///
    /// # Safety
    ///
    /// The caller must certify that no allocator-bound collection
    /// holds storage in the top end at the moment of reset. Bottom-end
    /// allocator-bound collections are unaffected by this call and
    /// retain their storage. Outstanding [`ArenaHandle`] values are
    /// correctly invalidated by the epoch advance and remain safe.
    ///
    /// Returns [`EpochSaturated`] when the epoch counter is at
    /// `u64::MAX`. Recovery is via [`Arena::force_reset_epoch`].
    pub unsafe fn reset_top_unchecked(&self) -> Result<(), EpochSaturated> {
        let next = self.epoch.get().checked_add(1).ok_or(EpochSaturated)?;
        self.top_top.set(self.capacity);
        self.epoch.set(next);
        Ok(())
    }

    /// Current epoch counter value.
    ///
    /// Captured by [`ArenaHandle`] at construction and compared on
    /// access. Hosts performing long-running missions may consult this
    /// alongside [`Arena::epoch_remaining`] to schedule a graceful
    /// restart well before saturation.
    pub fn epoch(&self) -> u64 {
        self.epoch.get()
    }

    /// Number of resets remaining before the epoch counter saturates.
    pub fn epoch_remaining(&self) -> u64 {
        u64::MAX - self.epoch.get()
    }

    /// Reset the epoch counter to zero.
    ///
    /// Recovery path for [`EpochSaturated`]. Resets bump pointers as
    /// well so the arena is in the same observable state as a freshly
    /// constructed arena, except for retained capacity.
    ///
    /// # Safety
    ///
    /// The caller must certify that no [`ArenaHandle`] produced under
    /// any prior epoch is reachable. Calling this while such handles
    /// exist invalidates the stale-detection guarantee and may permit
    /// use after invalidation that the type system would otherwise
    /// catch through epoch comparison.
    ///
    /// The intended use is recovery after a [`Arena::reset`] call has
    /// returned [`EpochSaturated`]. The host halts every consumer of
    /// the arena, drains every cache that holds an [`ArenaHandle`],
    /// and only then invokes this method.
    pub unsafe fn force_reset_epoch(&mut self) {
        self.bottom_top.set(0);
        self.top_top.set(self.capacity);
        self.epoch.set(0);
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

    /// Alias for [`Arena::bottom_handle`]. Suitable for code that
    /// treats the bottom end as a stack-like region.
    pub fn stack_handle(&self) -> BottomHandle<'_> {
        self.bottom_handle()
    }

    /// Alias for [`Arena::top_handle`]. Suitable for code that treats
    /// the top end as a heap-like region whose allocations are reset
    /// together rather than freed individually.
    pub fn heap_handle(&self) -> TopHandle<'_> {
        self.top_handle()
    }

    /// Allocate `n` bytes from the bottom end with no alignment
    /// requirement. Convenience wrapper for byte buffers and similar
    /// allocations where the caller does not care about alignment.
    ///
    /// Equivalent to allocating with a `Layout::from_size_align(n, 1)`
    /// through the `BottomHandle` Allocator implementation.
    pub fn alloc_bottom_bytes(&self, n: usize) -> Result<NonNull<[u8]>, AllocError> {
        let layout = Layout::from_size_align(n, 1).map_err(|_| AllocError)?;
        self.alloc_bottom(layout)
    }

    /// Allocate `n` bytes from the top end with no alignment requirement.
    pub fn alloc_top_bytes(&self, n: usize) -> Result<NonNull<[u8]>, AllocError> {
        let layout = Layout::from_size_align(n, 1).map_err(|_| AllocError)?;
        self.alloc_top(layout)
    }

    /// Allocate from the bottom end.
    ///
    /// Alignment is computed against the actual buffer base address, not
    /// the offset within the buffer. This makes the arena correct for
    /// buffers with any base alignment, including buffers obtained from
    /// allocators that only guarantee one-byte alignment and static
    /// arrays declared without explicit alignment annotations.
    fn alloc_bottom(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let cur = self.bottom_top.get();
        let base_addr = self.buffer.as_ptr() as usize;
        let cur_addr = base_addr.checked_add(cur).ok_or(AllocError)?;
        let align_mask = layout.align().saturating_sub(1);
        let aligned_addr = cur_addr.checked_add(align_mask).ok_or(AllocError)? & !align_mask;
        // `aligned_addr >= cur_addr >= base_addr`, so the subtraction
        // does not underflow.
        let aligned_offset = aligned_addr - base_addr;
        let new_top = aligned_offset
            .checked_add(layout.size())
            .ok_or(AllocError)?;
        if new_top > self.top_top.get() {
            return Err(AllocError);
        }
        self.bottom_top.set(new_top);
        if new_top > self.bottom_peak.get() {
            self.bottom_peak.set(new_top);
        }
        // SAFETY: `aligned_offset` is within `[0, top_top)` which is a
        // subset of `[0, capacity)`. The reserved range
        // `[aligned_offset, new_top)` is exclusive to this allocation
        // until the next reset or rewind.
        let ptr = unsafe { self.buffer.as_ptr().add(aligned_offset) };
        let slice = core::ptr::slice_from_raw_parts_mut(ptr, layout.size());
        NonNull::new(slice).ok_or(AllocError)
    }

    /// Allocate from the top end.
    ///
    /// Alignment is computed against the actual buffer base address.
    fn alloc_top(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let cur = self.top_top.get();
        let new_end_offset = cur.checked_sub(layout.size()).ok_or(AllocError)?;
        let base_addr = self.buffer.as_ptr() as usize;
        let new_end_addr = base_addr.checked_add(new_end_offset).ok_or(AllocError)?;
        let align_mask = layout.align().saturating_sub(1);
        // Round down to alignment. The result may be less than
        // `base_addr` if the buffer base is itself misaligned and the
        // allocation is near the bottom of the buffer; that case fails.
        let aligned_addr = new_end_addr & !align_mask;
        if aligned_addr < base_addr {
            return Err(AllocError);
        }
        let aligned_offset = aligned_addr - base_addr;
        if aligned_offset < self.bottom_top.get() {
            return Err(AllocError);
        }
        self.top_top.set(aligned_offset);
        if aligned_offset < self.top_peak_low.get() {
            self.top_peak_low.set(aligned_offset);
        }
        // SAFETY: `aligned_offset` is within `[bottom_top, capacity)`
        // and the reserved range `[aligned_offset, aligned_offset + size)`
        // is exclusive to this allocation until the next reset or
        // rewind.
        let ptr = unsafe { self.buffer.as_ptr().add(aligned_offset) };
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

// Soundness audit for the explicit `Drop` impl below.
//
// The arena holds a raw `NonNull<u8>` pointer to the backing storage.
// The `storage` field tracks ownership.
//
// - `Storage::External`: the caller owns the buffer. The `Drop` impl
//   does not free it. The caller's safety contracts on
//   `Arena::from_static_buffer` and `Arena::from_buffer_unchecked`
//   require the buffer to outlive the arena.
// - `Storage::Owned`: the arena owns the heap allocation that backs
//   the buffer. The `Drop` impl reconstitutes a `Box<[u8]>` from the
//   raw pointer and drops it, releasing the buffer.
//
// The buffer pointer has raw provenance (derived from `Box::into_raw`)
// so that handle allocations through a shared `&Arena` reference do
// not run afoul of stacked-borrows or tree-borrows aliasing rules.
impl Drop for Arena {
    fn drop(&mut self) {
        #[cfg(feature = "alloc")]
        if matches!(self.storage, Storage::Owned) && self.capacity > 0 {
            use alloc::alloc::{Layout as AllocLayout, dealloc};
            // SAFETY: When `storage` is `Owned` with non-zero capacity,
            // the buffer was obtained from `alloc_zeroed` with this
            // exact layout. The same layout is used for `dealloc`. The
            // arena is being dropped, so no further access to the
            // buffer occurs after this point.
            let layout = unsafe { AllocLayout::from_size_align_unchecked(self.capacity, 16) };
            unsafe { dealloc(self.buffer.as_ptr(), layout) };
        }
    }
}

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

/// Lifetime-free safe handle to a value stored in an arena.
///
/// Stores a raw pointer to a value of type `T` together with the epoch
/// at which the value was allocated. Access goes through [`ArenaHandle::get`],
/// which takes a borrow of the arena and validates the epoch. A mismatch
/// returns [`Stale`].
///
/// `ArenaHandle` does not borrow the arena directly. This makes it safe
/// to embed inside types whose lifetime is unrelated to the arena, such
/// as a runtime value enum that flows through caches and channels in
/// the host. The trade-off is that every dereference requires explicit
/// arena context. The wrapper does not implement `Deref` for that
/// reason.
///
/// `T: ?Sized` is supported. `T = str` is the canonical use through the
/// [`KString`] alias, where `NonNull<str>` is a wide pointer that
/// carries the byte length alongside the data pointer.
///
/// # Safety contract
///
/// The pointer must reference a region of the same arena that produced
/// the handle. The region must remain unmodified across resets while
/// the epoch is unchanged. The constructors in this crate uphold this
/// contract. Hand-rolled construction through public fields is not
/// possible because the fields are private.
///
/// # Serialization
///
/// `ArenaHandle` is intentionally not serializable. Its pointer is not
/// stable across processes. Long-lived deployments must convert handles
/// to owned bytes before checkpointing.
pub struct ArenaHandle<T: ?Sized> {
    ptr: NonNull<T>,
    epoch: u64,
}

// SAFETY: `ArenaHandle` is `Copy` for any `T: ?Sized` because both
// fields are `Copy`. `NonNull<T>` is `Copy` for unsized `T`.
impl<T: ?Sized> Copy for ArenaHandle<T> {}

impl<T: ?Sized> Clone for ArenaHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> core::fmt::Debug for ArenaHandle<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ArenaHandle")
            .field("ptr", &self.ptr.as_ptr())
            .field("epoch", &self.epoch)
            .finish()
    }
}

// `ArenaHandle` is intentionally not `Send` or `Sync` because the
// arena it references is single-threaded. The pointer is `*mut` under
// `NonNull`, which inherits the conservative auto-trait posture.

impl<T: ?Sized> ArenaHandle<T> {
    /// Resolve the handle against the arena that produced it.
    ///
    /// Returns [`Stale`] if the arena has been reset since the handle
    /// was issued. The borrow of `arena` ties the returned reference's
    /// lifetime to the arena, preventing the reference from outliving
    /// the next reset.
    ///
    /// # Safety
    ///
    /// The arena must be the same arena that produced the handle. Mixing
    /// handles between arenas is a logic error. The arena allocations
    /// are uniquely owned by the arena, so passing the wrong arena will
    /// dereference memory that is not the original allocation. This
    /// would be unsound if the wrong arena's epoch happened to match.
    pub fn get<'a>(&self, arena: &'a Arena) -> Result<&'a T, Stale> {
        if arena.epoch() != self.epoch {
            return Err(Stale);
        }
        // SAFETY: The handle was issued under the current epoch. The
        // arena guarantees that allocated regions remain intact until
        // the next reset, which advances the epoch.
        Ok(unsafe { self.ptr.as_ref() })
    }

    /// Epoch captured when the handle was issued.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
}

/// Lifetime-free arena-backed string handle.
///
/// Specialization of [`ArenaHandle`] for `T = str`. Stores a wide
/// pointer that already carries the string's byte length, plus the
/// arena epoch at allocation time.
pub type KString = ArenaHandle<str>;

impl KString {
    /// Allocate a copy of `s` in the arena's top region and return a
    /// handle to it.
    ///
    /// The bytes are copied; the source slice is not retained. The
    /// resulting handle is valid until the next [`Arena::reset`].
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn alloc(arena: &Arena, s: &str) -> Result<Self, AllocError> {
        let bytes = s.as_bytes();
        let buffer = arena.alloc_top_bytes(bytes.len())?;
        let dst = buffer.as_ptr() as *mut u8;
        // SAFETY: `buffer` is unique storage of `bytes.len()` bytes
        // freshly allocated from the arena. The source is a valid byte
        // slice. The regions do not overlap because the allocator
        // returns previously unused memory.
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len()) };
        // Construct a `*mut str` from the freshly-written bytes. The
        // layout of `*mut str` matches `*mut [u8]`.
        let raw_slice: *mut [u8] = core::ptr::slice_from_raw_parts_mut(dst, bytes.len());
        let raw_str: *mut str = raw_slice as *mut str;
        // SAFETY: `raw_str` is non-null because `dst` came from a
        // successful arena allocation.
        let nn = unsafe { NonNull::new_unchecked(raw_str) };
        Ok(ArenaHandle {
            ptr: nn,
            epoch: arena.epoch(),
        })
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

    // Skipped under miri because the test deliberately leaks a Vec to
    // synthesize a `'static mut [u8]`. Real embedded use of
    // `from_static_buffer` is a `static mut` array, which has no leak.
    #[cfg_attr(miri, ignore)]
    #[test]
    fn arena_from_static_buffer() {
        // Use a leaked Box for a 'static-like buffer in tests. In real
        // embedded use, this would be a `static mut [u8; N]`.
        let leaked: &'static mut [u8] = test_alloc::vec![0u8; 256].leak();
        let arena = Arena::from_static_buffer(leaked);
        assert_eq!(arena.capacity(), 256);
        let layout = Layout::new::<u64>();
        let p = arena.bottom_handle().allocate(layout).unwrap();
        // The leaked Vec<u8> has alignment-of-u8 (one byte) per Rust's
        // contract. The arena pads as needed to satisfy the requested
        // u64 alignment, so usage is at least size and at most
        // size + alignment.
        assert!(arena.bottom_used() >= 8);
        assert!(arena.bottom_used() <= 8 + 8);
        let addr = p.as_ptr() as *const u8 as usize;
        assert_eq!(addr % 8, 0);
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
        // The buffer base may be any alignment for from_buffer_unchecked.
        // The arena pads to satisfy the requested alignment, so usage
        // is at least the layout size and at most size + alignment.
        assert!(arena.bottom_used() >= 4);
        assert!(arena.bottom_used() <= 4 + 4);
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
        arena.reset().unwrap();
        assert_eq!(arena.bottom_used(), 0);
        assert_eq!(arena.top_used(), 0);
        assert_eq!(arena.epoch(), 1);
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
        arena.reset().unwrap();
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
    fn epoch_advances_on_reset() {
        let mut arena = Arena::with_capacity(64);
        assert_eq!(arena.epoch(), 0);
        arena.reset().unwrap();
        assert_eq!(arena.epoch(), 1);
        arena.reset().unwrap();
        assert_eq!(arena.epoch(), 2);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn epoch_saturates() {
        let mut arena = Arena::with_capacity(16);
        // Force the epoch to one below saturation.
        arena.epoch.set(u64::MAX - 1);
        // First reset advances to u64::MAX.
        arena.reset().unwrap();
        assert_eq!(arena.epoch(), u64::MAX);
        assert_eq!(arena.epoch_remaining(), 0);
        // Second reset must refuse.
        let result = arena.reset();
        assert!(matches!(result, Err(EpochSaturated)));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn force_reset_epoch_recovers() {
        let mut arena = Arena::with_capacity(16);
        arena.epoch.set(u64::MAX);
        assert!(matches!(arena.reset(), Err(EpochSaturated)));
        // SAFETY: No `ArenaHandle` exists in this test scope.
        unsafe {
            arena.force_reset_epoch();
        }
        assert_eq!(arena.epoch(), 0);
        arena.reset().unwrap();
        assert_eq!(arena.epoch(), 1);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn kstring_roundtrip() {
        let arena = Arena::with_capacity(256);
        let handle = KString::alloc(&arena, "hello").unwrap();
        let s = handle.get(&arena).unwrap();
        assert_eq!(s, "hello");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn kstring_stale_after_reset() {
        let mut arena = Arena::with_capacity(256);
        let handle = KString::alloc(&arena, "ephemeral").unwrap();
        assert_eq!(handle.get(&arena).unwrap(), "ephemeral");
        arena.reset().unwrap();
        assert!(matches!(handle.get(&arena), Err(Stale)));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn kstring_handle_is_copy() {
        let arena = Arena::with_capacity(256);
        let handle = KString::alloc(&arena, "shared").unwrap();
        let copy = handle;
        assert_eq!(handle.get(&arena).unwrap(), "shared");
        assert_eq!(copy.get(&arena).unwrap(), "shared");
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

    #[test]
    fn arena_misaligned_base_produces_aligned_allocation() {
        // Construct an arena over a buffer whose base address is
        // deliberately offset by one byte from the underlying storage.
        // The base is therefore at most byte-aligned. The arena must
        // still produce u64-aligned pointers for u64 allocations.
        let mut backing = test_alloc::vec![0u8; 256];
        let raw_ptr = backing.as_mut_ptr();
        // SAFETY: The backing vector lives until the end of the test.
        // We deliberately offset by one to create a misaligned base.
        let arena = unsafe { Arena::from_buffer_unchecked(raw_ptr.add(1), 200) };

        // Allocate a u64. The pointer must be 8-byte aligned regardless
        // of the misaligned base.
        let p_u64 = arena
            .bottom_handle()
            .allocate(Layout::new::<u64>())
            .unwrap();
        let addr = p_u64.as_ptr() as *const u8 as usize;
        assert_eq!(addr % 8, 0, "allocation must be 8-byte aligned");

        // Allocate a u128. The pointer must be 16-byte aligned.
        let p_u128 = arena
            .bottom_handle()
            .allocate(Layout::new::<u128>())
            .unwrap();
        let addr = p_u128.as_ptr() as *const u8 as usize;
        assert_eq!(addr % 16, 0, "allocation must be 16-byte aligned");

        // Top-end allocation also aligned.
        let p_top = arena.top_handle().allocate(Layout::new::<u64>()).unwrap();
        let addr = p_top.as_ptr() as *const u8 as usize;
        assert_eq!(addr % 8, 0, "top allocation must be 8-byte aligned");

        // Keep `backing` alive until here.
        drop(backing);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_byte_allocation_packs_tightly() {
        // alloc_bottom_bytes does not enforce alignment. Three u8
        // allocations of one byte each consume exactly three bytes.
        let arena = Arena::with_capacity(64);
        let _a = arena.alloc_bottom_bytes(1).unwrap();
        let _b = arena.alloc_bottom_bytes(1).unwrap();
        let _c = arena.alloc_bottom_bytes(1).unwrap();
        assert_eq!(arena.bottom_used(), 3);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_aligned_allocation_pads() {
        // After one byte, an aligned u64 allocation pads to align 8.
        // Total used should be 8 + 8 = 16 bytes.
        let arena = Arena::with_capacity(64);
        let _a = arena.alloc_bottom_bytes(1).unwrap();
        assert_eq!(arena.bottom_used(), 1);
        let _b = arena
            .bottom_handle()
            .allocate(Layout::new::<u64>())
            .unwrap();
        assert_eq!(arena.bottom_used(), 16);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_top_byte_allocation() {
        let arena = Arena::with_capacity(64);
        let _a = arena.alloc_top_bytes(3).unwrap();
        assert_eq!(arena.top_used(), 3);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn arena_byte_allocation_zero_size() {
        let arena = Arena::with_capacity(64);
        // Zero-size byte allocation is admissible and consumes nothing.
        let _a = arena.alloc_bottom_bytes(0).unwrap();
        assert_eq!(arena.bottom_used(), 0);
    }
}
