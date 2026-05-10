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
}
