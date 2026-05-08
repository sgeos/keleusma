//! Dual-end bump-allocated arena for Keleusma's stack and heap regions.
//!
//! The arena is a single contiguous buffer with two pointers growing toward
//! each other from opposite ends. Stack allocations grow up from offset zero.
//! Heap allocations grow down from the buffer's high end. Allocation fails
//! when the two pointers would meet. Reset clears both pointers atomically.
//!
//! See R31, R32, R33 in `docs/decisions/RESOLVED.md` for the design decisions
//! and `docs/architecture/EXECUTION_MODEL.md` for the execution model.
//!
//! The arena exposes `StackHandle` and `HeapHandle` types that implement
//! the `allocator_api2::Allocator` trait. These handles can be passed to
//! `allocator_api2::vec::Vec::new_in` and similar constructors to obtain
//! arena-backed collections.

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec;
use core::cell::Cell;
use core::ptr::NonNull;

use allocator_api2::alloc::{AllocError, Allocator};
use core::alloc::Layout;

/// A dual-end bump-allocated arena.
///
/// The arena owns a fixed-size buffer of bytes. Two pointers track the
/// boundaries of the stack and heap regions. The stack region begins at
/// offset zero and grows upward. The heap region begins at the buffer's
/// length and grows downward. Allocation fails when the two regions would
/// overlap.
///
/// The arena is not thread-safe by design. The Keleusma VM is single-
/// threaded, and the arena's interior mutability uses `Cell` rather than
/// atomic primitives.
pub struct Arena {
    /// The owned backing storage. Read through `buffer_ptr`. The field is
    /// kept to ensure the buffer remains valid for the arena's lifetime.
    #[allow(dead_code)]
    backing: Box<[u8]>,
    /// Raw pointer to the start of `backing`. Stable for the arena's lifetime.
    buffer_ptr: *mut u8,
    /// Total capacity of the buffer in bytes.
    capacity: usize,
    /// Current top of the stack region. Grows from zero toward `heap_top`.
    stack_top: Cell<usize>,
    /// Current bottom of the heap region. Grows down from `capacity` toward
    /// `stack_top`.
    heap_top: Cell<usize>,
}

impl Arena {
    /// Create a new arena with the given byte capacity.
    ///
    /// The backing buffer is allocated from the global allocator and zeroed.
    /// The two end pointers are initialized to the buffer extents.
    pub fn with_capacity(capacity: usize) -> Self {
        let mut backing: Box<[u8]> = vec![0u8; capacity].into_boxed_slice();
        let buffer_ptr = backing.as_mut_ptr();
        Self {
            backing,
            buffer_ptr,
            capacity,
            stack_top: Cell::new(0),
            heap_top: Cell::new(capacity),
        }
    }

    /// Total capacity of the arena in bytes.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Bytes currently allocated from the stack end.
    pub fn stack_used(&self) -> usize {
        self.stack_top.get()
    }

    /// Bytes currently allocated from the heap end.
    pub fn heap_used(&self) -> usize {
        self.capacity - self.heap_top.get()
    }

    /// Bytes free for either end to consume.
    pub fn free(&self) -> usize {
        self.heap_top.get().saturating_sub(self.stack_top.get())
    }

    /// Reset both bump pointers, reclaiming all allocations.
    ///
    /// Called by the VM at `Op::Reset`. Constant-time. Does not zero the
    /// buffer contents because subsequent allocations will overwrite as
    /// needed and the script does not observe initialization.
    pub fn reset(&mut self) {
        self.stack_top.set(0);
        self.heap_top.set(self.capacity);
    }

    /// Obtain a stack-end allocation handle.
    pub fn stack_handle(&self) -> StackHandle<'_> {
        StackHandle(self)
    }

    /// Obtain a heap-end allocation handle.
    pub fn heap_handle(&self) -> HeapHandle<'_> {
        HeapHandle(self)
    }

    /// Allocate from the stack end.
    fn alloc_stack(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let cur = self.stack_top.get();
        let align_mask = layout.align().saturating_sub(1);
        let aligned = cur.checked_add(align_mask).ok_or(AllocError)? & !align_mask;
        let new_top = aligned.checked_add(layout.size()).ok_or(AllocError)?;
        if new_top > self.heap_top.get() {
            return Err(AllocError);
        }
        self.stack_top.set(new_top);
        // SAFETY: `aligned` is within `[0, capacity)` because it is at most
        // `heap_top` which is at most `capacity`. The reserved range
        // `[aligned, new_top)` is exclusive to this allocation until the
        // next reset.
        let ptr = unsafe { self.buffer_ptr.add(aligned) };
        let slice = core::ptr::slice_from_raw_parts_mut(ptr, layout.size());
        NonNull::new(slice).ok_or(AllocError)
    }

    /// Allocate from the heap end.
    fn alloc_heap(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let cur = self.heap_top.get();
        let new_end = cur.checked_sub(layout.size()).ok_or(AllocError)?;
        let align_mask = layout.align().saturating_sub(1);
        let aligned_start = new_end & !align_mask;
        if aligned_start < self.stack_top.get() {
            return Err(AllocError);
        }
        self.heap_top.set(aligned_start);
        // SAFETY: `aligned_start` is within `[stack_top, capacity)` and the
        // reserved range `[aligned_start, aligned_start + size)` is
        // exclusive to this allocation until the next reset.
        let ptr = unsafe { self.buffer_ptr.add(aligned_start) };
        let slice = core::ptr::slice_from_raw_parts_mut(ptr, layout.size());
        NonNull::new(slice).ok_or(AllocError)
    }
}

impl core::fmt::Debug for Arena {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Arena")
            .field("capacity", &self.capacity)
            .field("stack_used", &self.stack_used())
            .field("heap_used", &self.heap_used())
            .field("free", &self.free())
            .finish()
    }
}

// The `backing` field keeps the storage alive. The pointer derived from it
// is stable for the arena's lifetime. Drop happens when `Arena` is dropped,
// which deallocates the box. No additional Drop impl is needed.

/// Allocation handle for the stack end of an arena.
///
/// Implements `allocator_api2::Allocator`. Use with constructors such as
/// `allocator_api2::vec::Vec::new_in(arena.stack_handle())`.
#[derive(Clone, Copy, Debug)]
pub struct StackHandle<'a>(&'a Arena);

/// Allocation handle for the heap end of an arena.
///
/// Implements `allocator_api2::Allocator`. Use with constructors such as
/// `allocator_api2::vec::Vec::new_in(arena.heap_handle())`.
#[derive(Clone, Copy, Debug)]
pub struct HeapHandle<'a>(&'a Arena);

// SAFETY: The arena's allocation methods uphold the Allocator contract.
// Returned pointers are valid for the requested layout, unique to the
// caller, and remain valid until the next `reset()` call. Deallocation is
// a no-op because the bump allocator reclaims memory at reset.
unsafe impl Allocator for StackHandle<'_> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.0.alloc_stack(layout)
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        // No-op. Bump allocator reclaims at reset.
    }
}

// SAFETY: Same reasoning as StackHandle.
unsafe impl Allocator for HeapHandle<'_> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.0.alloc_heap(layout)
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        // No-op. Bump allocator reclaims at reset.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allocator_api2::vec::Vec as ArenaVec;

    #[test]
    fn arena_initial_state() {
        let arena = Arena::with_capacity(1024);
        assert_eq!(arena.capacity(), 1024);
        assert_eq!(arena.stack_used(), 0);
        assert_eq!(arena.heap_used(), 0);
        assert_eq!(arena.free(), 1024);
    }

    #[test]
    fn arena_stack_allocation() {
        let arena = Arena::with_capacity(64);
        let layout = Layout::new::<u32>();
        let handle = arena.stack_handle();
        let p1 = handle.allocate(layout).unwrap();
        let p2 = handle.allocate(layout).unwrap();
        // Two distinct allocations of 4 bytes each.
        assert_ne!(p1.as_ptr(), p2.as_ptr());
        assert_eq!(arena.stack_used(), 8);
        assert_eq!(arena.heap_used(), 0);
    }

    #[test]
    fn arena_heap_allocation() {
        let arena = Arena::with_capacity(64);
        let layout = Layout::new::<u64>();
        let handle = arena.heap_handle();
        let p1 = handle.allocate(layout).unwrap();
        let p2 = handle.allocate(layout).unwrap();
        assert_ne!(p1.as_ptr(), p2.as_ptr());
        assert_eq!(arena.heap_used(), 16);
        assert_eq!(arena.stack_used(), 0);
    }

    #[test]
    fn arena_alignment_respected() {
        let arena = Arena::with_capacity(64);
        let handle = arena.stack_handle();
        let _byte = handle.allocate(Layout::new::<u8>()).unwrap();
        // Now stack_top is 1. A u64 allocation needs 8-byte alignment.
        let p_u64 = handle.allocate(Layout::new::<u64>()).unwrap();
        let addr = p_u64.as_ptr() as *const u8 as usize;
        assert_eq!(addr % 8, 0, "u64 allocation must be 8-byte aligned");
    }

    #[test]
    fn arena_exhaustion() {
        let arena = Arena::with_capacity(16);
        let handle = arena.stack_handle();
        let layout = Layout::new::<u64>();
        let _a = handle.allocate(layout).unwrap();
        let _b = handle.allocate(layout).unwrap();
        let result = handle.allocate(layout);
        assert!(result.is_err());
    }

    #[test]
    fn arena_stack_heap_meet() {
        let arena = Arena::with_capacity(16);
        let stack = arena.stack_handle();
        let heap = arena.heap_handle();
        let layout = Layout::new::<u64>();
        let _a = stack.allocate(layout).unwrap();
        let _b = heap.allocate(layout).unwrap();
        // Now stack_top = 8 and heap_top = 8. Either further allocation fails.
        let stack_result = stack.allocate(layout);
        let heap_result = heap.allocate(layout);
        assert!(stack_result.is_err());
        assert!(heap_result.is_err());
    }

    #[test]
    fn arena_reset() {
        let mut arena = Arena::with_capacity(64);
        let layout = Layout::new::<u64>();
        {
            let stack = arena.stack_handle();
            let heap = arena.heap_handle();
            let _a = stack.allocate(layout).unwrap();
            let _b = heap.allocate(layout).unwrap();
        }
        assert_eq!(arena.stack_used(), 8);
        assert_eq!(arena.heap_used(), 8);
        arena.reset();
        assert_eq!(arena.stack_used(), 0);
        assert_eq!(arena.heap_used(), 0);
        assert_eq!(arena.free(), 64);
    }

    #[test]
    fn arena_vec_integration() {
        let arena = Arena::with_capacity(1024);
        let mut v: ArenaVec<i64, _> = ArenaVec::new_in(arena.stack_handle());
        for i in 0..10 {
            v.push(i);
        }
        assert_eq!(v.len(), 10);
        assert_eq!(v.iter().sum::<i64>(), 45);
        // Some bytes are now allocated in the stack region. The exact amount
        // depends on Vec's growth policy.
        assert!(arena.stack_used() > 0);
    }

    #[test]
    fn arena_dual_vec_integration() {
        let arena = Arena::with_capacity(2048);
        let mut stack_v: ArenaVec<i64, _> = ArenaVec::new_in(arena.stack_handle());
        let mut heap_v: ArenaVec<i64, _> = ArenaVec::new_in(arena.heap_handle());
        for i in 0..5 {
            stack_v.push(i);
            heap_v.push(i * 100);
        }
        assert_eq!(stack_v.len(), 5);
        assert_eq!(heap_v.len(), 5);
        assert!(arena.stack_used() > 0);
        assert!(arena.heap_used() > 0);
    }

    #[test]
    fn arena_zero_capacity() {
        let arena = Arena::with_capacity(0);
        assert_eq!(arena.capacity(), 0);
        let handle = arena.stack_handle();
        let result = handle.allocate(Layout::new::<u8>());
        assert!(result.is_err());
    }

    #[test]
    fn arena_zero_size_layout() {
        let arena = Arena::with_capacity(64);
        let handle = arena.stack_handle();
        // Layout for () has size 0, alignment 1.
        let layout = Layout::new::<()>();
        let result = handle.allocate(layout);
        assert!(result.is_ok());
        // Zero-size allocation does not consume bytes.
        assert_eq!(arena.stack_used(), 0);
    }
}
