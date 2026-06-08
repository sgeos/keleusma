//! Arena-backed dynamic-string handle for the Keleusma runtime.
//!
//! [`KString`] is a thin newtype around [`keleusma_arena::ArenaHandle`]
//! specialised to `str`. The wrapper holds a wide pointer into the
//! arena's top region together with the arena epoch at allocation
//! time. [`KString::get`] returns [`keleusma_arena::Stale`] when the
//! arena has been reset since the handle was issued, surfacing
//! lifetime-free stale-pointer detection at the access site.
//!
//! The generic mechanism lives in `keleusma-arena`. The `str`
//! specialisation lives here because the `&str` copy semantics and
//! the bounded-memory accounting are concerns of the Keleusma
//! runtime, not of the allocator.

extern crate alloc;

use core::ptr::NonNull;

use allocator_api2::alloc::AllocError;
use keleusma_arena::{Arena, ArenaHandle, Stale};

/// Arena-backed dynamic-string handle.
///
/// Newtype over [`ArenaHandle<str>`]. Forwards [`KString::get`] and
/// [`KString::epoch`] to the inner handle. [`KString::alloc`] copies
/// the source `&str` into the arena's top region and wraps the result.
#[derive(Debug, Clone, Copy)]
pub struct KString(ArenaHandle<str>);

impl KString {
    /// Allocate a copy of `s` in the arena's top region and return a
    /// handle to it.
    ///
    /// The bytes are copied; the source slice is not retained. The
    /// resulting handle is valid until the next
    /// [`Arena::reset`](keleusma_arena::Arena::reset).
    pub fn alloc(arena: &Arena, s: &str) -> Result<Self, AllocError> {
        let bytes = s.as_bytes();
        let buffer = arena.alloc_top_bytes(bytes.len())?;
        let dst = buffer.as_ptr() as *mut u8;
        // SAFETY: `buffer` is unique storage of `bytes.len()` bytes
        // freshly allocated from the arena. The source is a valid
        // byte slice. The regions do not overlap because the
        // allocator returns previously unused memory.
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len()) };
        // Construct a `*mut str` from the freshly-written bytes. The
        // layout of `*mut str` matches `*mut [u8]`.
        let raw_slice: *mut [u8] = core::ptr::slice_from_raw_parts_mut(dst, bytes.len());
        let raw_str: *mut str = raw_slice as *mut str;
        // SAFETY: `raw_str` is non-null because `dst` came from a
        // successful arena allocation.
        let nn = unsafe { NonNull::new_unchecked(raw_str) };
        // SAFETY: `nn` references storage in `arena`'s top region
        // freshly allocated under the current epoch.
        let handle = unsafe { ArenaHandle::from_raw_parts(nn, arena.epoch()) };
        Ok(KString(handle))
    }

    /// Resolve the handle against the arena that produced it.
    ///
    /// Returns [`Stale`] if the arena has been reset since the handle
    /// was issued.
    pub fn get<'a>(&self, arena: &'a Arena) -> Result<&'a str, Stale> {
        self.0.get(arena)
    }

    /// Epoch captured when the handle was issued.
    pub fn epoch(&self) -> u64 {
        self.0.epoch()
    }

    /// Borrow the underlying generic handle. Useful for callers that
    /// want to compose with other `ArenaHandle<T>` machinery.
    pub fn as_handle(&self) -> &ArenaHandle<str> {
        &self.0
    }

    /// The handle's raw `(data_pointer, byte_length)`, read from the wide
    /// pointer's metadata without dereferencing it (B28 P3).
    ///
    /// A flat composite stores these two words in place of a `Text` field.
    /// The epoch is not stored: it is reattached at every extraction by
    /// [`KString::from_raw_parts`], which is the arena's epoch-carrying
    /// wrapper for a value passed back out of a flat body.
    pub fn raw_parts(&self) -> (usize, usize) {
        let raw: *const [u8] = self.0.as_non_null().as_ptr() as *const [u8];
        (raw as *const u8 as usize, raw.len())
    }

    /// Rebuild a handle from a `(data_pointer, byte_length)` pair read from
    /// a flat `Text` field and the arena epoch current at extraction (B28
    /// P3). The returned `KString` carries that epoch, so a later `get`
    /// after a `RESET` returns [`Stale`] rather than dereferencing
    /// reclaimed memory.
    ///
    /// # Safety
    ///
    /// `ptr` and `len` must describe a `str`-valid (UTF-8) region that is
    /// live under `epoch`. The flat-composite access path upholds this: the
    /// `(ptr, len)` words come from a composite body that has just resolved
    /// current, so the referenced string shares that live epoch.
    pub unsafe fn from_raw_parts(ptr: usize, len: usize, epoch: u64) -> Self {
        let raw_slice: *mut [u8] = core::ptr::slice_from_raw_parts_mut(ptr as *mut u8, len);
        let raw_str: *mut str = raw_slice as *mut str;
        // SAFETY: the caller guarantees `ptr` is the non-null data pointer
        // of a live arena allocation under `epoch`.
        let nn = unsafe { NonNull::new_unchecked(raw_str) };
        // SAFETY: forwarded from this function's safety contract.
        let handle = unsafe { ArenaHandle::from_raw_parts(nn, epoch) };
        KString(handle)
    }
}
