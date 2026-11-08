//! Arena-resident coroutine slots: a fixed pool reserved from the host arena.
//!
//! The design gives each coroutine a region of the master arena sized by its
//! static frame bound, returned to a free list on release. Retcon lowering
//! already confines a coroutine to its caller's slot, refusing any frame that
//! would reach LLVM's overflow allocator, so the arena's part is to supply
//! those slots. This pool does that and nothing else.
//!
//! Every byte is reserved once, by [`SlotPool::new`]: the slots, the free-index
//! stack and the occupancy flags. Acquire and release move indices and never
//! allocate. The pool's whole contribution to worst-case memory is therefore
//! [`SlotPool::reserved_bytes`], fixed at construction.
//!
//! The pool borrows the arena shared for its lifetime. [`keleusma::Arena::reset`]
//! takes the arena mutably, so no reset can reclaim a slot while the pool, and
//! therefore any suspended instance in it, exists. That is checked by the
//! compiler rather than by convention.
//!
//! # Host obligations
//!
//! Release an instance through its `kel_coroutine_<entry>_release` before
//! returning its slot. [`SlotPool::release`] reads the slot's continuation word
//! and refuses a slot that is still live, because a live frame may not be
//! reused. A slot is not moved while acquired, so a suspended frame's address
//! stays valid.

use core::ptr::NonNull;

use keleusma::Arena;

use super::{FRAME_ALIGN, slot_bytes};

/// Why a pool could not be built or a slot could not be returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolError {
    /// A pool of zero slots serves nothing and is refused rather than built.
    ZeroCapacity,
    /// The frame size is outside the range [`slot_bytes`] admits.
    FrameSize,
    /// The reservation does not fit in `usize`.
    Overflow,
    /// The arena cannot supply the reservation.
    ArenaExhausted,
    /// The slot was not issued by this pool.
    Foreign,
    /// The slot is already free.
    NotAcquired,
    /// The slot's continuation is still live. Release the instance first.
    Live,
}

/// An acquired slot. It is neither `Copy` nor `Clone`, so returning it to the
/// pool consumes it.
#[derive(Debug)]
pub struct Slot {
    index: u32,
    ptr: NonNull<u8>,
}

impl Slot {
    /// The slot address to pass to `kel_coroutine_<entry>_start`, `_resume`
    /// and `_release`. Aligned to [`FRAME_ALIGN`].
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    /// The slot's position in its pool.
    pub fn index(&self) -> u32 {
        self.index
    }
}

/// A fixed number of equal coroutine slots in one arena reservation.
pub struct SlotPool<'a> {
    base: NonNull<u8>,
    stride: usize,
    capacity: u32,
    /// Free indices as a stack, `free_len` of them valid.
    free: NonNull<u32>,
    free_len: u32,
    /// One byte per slot, one while acquired.
    acquired: NonNull<u8>,
    reserved: usize,
    _arena: core::marker::PhantomData<&'a Arena>,
}

fn align_up(n: usize, a: usize) -> Option<usize> {
    n.checked_add(a - 1).map(|v| v & !(a - 1))
}

impl<'a> SlotPool<'a> {
    /// Reserve `capacity` slots for coroutines lowered with `frame_bytes`.
    ///
    /// Pass the static bound from [`super::minimum_frame_bytes`], or any larger
    /// reservation the instances were lowered with. The memory comes from the
    /// arena's bottom end in one allocation.
    pub fn new(arena: &'a Arena, frame_bytes: u32, capacity: u32) -> Result<Self, PoolError> {
        if capacity == 0 {
            return Err(PoolError::ZeroCapacity);
        }
        let align = FRAME_ALIGN as usize;
        let slot = slot_bytes(frame_bytes).map_err(|_| PoolError::FrameSize)? as usize;
        let stride = align_up(slot, align).ok_or(PoolError::Overflow)?;
        let n = capacity as usize;
        let slots = stride.checked_mul(n).ok_or(PoolError::Overflow)?;
        let free_bytes = n.checked_mul(4).ok_or(PoolError::Overflow)?;
        // Layout within the reservation: slots, then the free stack, then the
        // flags. The slots are first so their alignment is the base's.
        let total = slots
            .checked_add(free_bytes)
            .and_then(|v| v.checked_add(n))
            .and_then(|v| v.checked_add(align - 1))
            .ok_or(PoolError::Overflow)?;
        let raw = arena
            .alloc_bottom_bytes(total)
            .map_err(|_| PoolError::ArenaExhausted)?;
        let start = raw.as_ptr() as *mut u8;
        let pad = start.align_offset(align);
        if pad >= align {
            return Err(PoolError::ArenaExhausted);
        }
        // SAFETY: `pad < align` bytes were over-reserved, and every offset
        // below stays within the `total` bytes the arena returned.
        let base = unsafe { start.add(pad) };
        let free = unsafe { base.add(slots) } as *mut u32;
        let acquired = unsafe { base.add(slots + free_bytes) };
        // The free stack holds every index, highest first, so the first
        // acquisition returns slot zero. Written unaligned because the stack
        // follows the slots at a multiple of eight, which is also a multiple of
        // four, but the write does not depend on that.
        for i in 0..capacity {
            // SAFETY: `i < capacity`, within the free stack.
            unsafe {
                free.add(i as usize).write_unaligned(capacity - 1 - i);
                acquired.add(i as usize).write(0);
            }
        }
        Ok(Self {
            // SAFETY: derived from a non-null arena allocation.
            base: unsafe { NonNull::new_unchecked(base) },
            stride,
            capacity,
            free: unsafe { NonNull::new_unchecked(free) },
            free_len: capacity,
            acquired: unsafe { NonNull::new_unchecked(acquired) },
            reserved: total,
            _arena: core::marker::PhantomData,
        })
    }

    /// Take a free slot, or `None` when every slot is acquired. Never allocates.
    pub fn acquire(&mut self) -> Option<Slot> {
        if self.free_len == 0 {
            return None;
        }
        self.free_len -= 1;
        // SAFETY: `free_len < capacity`, within the free stack.
        let index = unsafe {
            self.free
                .as_ptr()
                .add(self.free_len as usize)
                .read_unaligned()
        };
        debug_assert!(index < self.capacity);
        // SAFETY: `index < capacity`.
        unsafe { self.acquired.as_ptr().add(index as usize).write(1) };
        // SAFETY: `index * stride` is within the slot area.
        let ptr = unsafe { self.base.as_ptr().add(index as usize * self.stride) };
        // Arena memory is not zeroed. Clearing the continuation word makes a
        // slot that is never started read as inactive, which `release` relies
        // on. Start overwrites it.
        // SAFETY: the slot is FRAME_ALIGN aligned and at least eight bytes.
        unsafe { ptr.cast::<usize>().write(0) };
        Some(Slot {
            index,
            // SAFETY: derived from the non-null pool base.
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        })
    }

    /// Return a slot whose instance has been released or has completed.
    ///
    /// On refusal the slot is handed back unchanged with the reason.
    pub fn release(&mut self, slot: Slot) -> Result<(), (Slot, PoolError)> {
        let offset = (slot.ptr.as_ptr() as usize).wrapping_sub(self.base.as_ptr() as usize);
        if slot.index >= self.capacity || offset != slot.index as usize * self.stride {
            return Err((slot, PoolError::Foreign));
        }
        // SAFETY: `index < capacity`.
        if unsafe { self.acquired.as_ptr().add(slot.index as usize).read() } == 0 {
            return Err((slot, PoolError::NotAcquired));
        }
        // The first header word is the current continuation, null for a
        // released or completed instance and for a slot never started.
        // SAFETY: the slot is FRAME_ALIGN aligned and at least eight bytes.
        if unsafe { slot.ptr.as_ptr().cast::<usize>().read() } != 0 {
            return Err((slot, PoolError::Live));
        }
        // SAFETY: `index < capacity` and `free_len < capacity` while a slot is out.
        unsafe {
            self.acquired.as_ptr().add(slot.index as usize).write(0);
            self.free
                .as_ptr()
                .add(self.free_len as usize)
                .write_unaligned(slot.index);
        }
        self.free_len += 1;
        Ok(())
    }

    /// Bytes reserved from the arena, slots and bookkeeping together. Fixed at
    /// construction; this is the pool's whole worst-case memory contribution.
    pub fn reserved_bytes(&self) -> usize {
        self.reserved
    }

    /// Distance between consecutive slots: [`slot_bytes`] rounded to [`FRAME_ALIGN`].
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// Number of slots.
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Number of slots currently free.
    pub fn available(&self) -> u32 {
        self.free_len
    }
}
