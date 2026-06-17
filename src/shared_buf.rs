//! The host-owned shared-data buffer borrowed for one `call`/`resume`
//! (B28 item 2 shared-data re-architecture).
//!
//! Shared data is an external host-owned struct of a fixed flat layout. The
//! host lends the virtual machine a `&mut [u8]` view of it at each `call` or
//! `resume`; the script reads and writes it in place by byte offset and the
//! virtual machine retains nothing across the yield. The run loop and its op
//! handlers are too large to thread the borrow through, so the buffer is
//! captured here as a raw `(pointer, length)` for the duration of one call.
//!
//! ALL raw-pointer unsafety in the shared-data path is confined to this
//! module. The safe-signature methods below are sound provided the virtual
//! machine upholds one invariant, discharged entirely at the call boundary:
//!
//! - [`SharedBuf::set`] is called from a `&mut [u8]` entry-point argument whose
//!   borrow outlives every [`SharedBuf::bytes`] and [`SharedBuf::bytes_mut`]
//!   call, and [`SharedBuf::clear`] runs before that entry point returns, so a
//!   captured pointer is never dereferenced after its borrow ends.
//! - No two slices obtained from this buffer are held at the same time, so the
//!   `&mut` slice from [`SharedBuf::bytes_mut`] is never aliased.
//!
//! Under that invariant the captured pointer addresses `len` valid,
//! exclusively borrowed bytes at every dereference, so reconstructing the
//! slice is sound. The virtual machine sets the buffer at the top of
//! `call_with_shared`/`resume_with_shared` and clears it before returning,
//! holding the originating `&mut [u8]` borrow across the whole call.

use core::ptr::NonNull;

/// A borrowed view of the host's shared-data buffer for the current call, or
/// none (in which case shared access falls back to the slot model).
///
/// The type is not `Send`/`Sync` (a `NonNull` field), matching the
/// single-threaded virtual machine. It owns no memory; the host owns the
/// buffer and outlives the borrow.
pub struct SharedBuf {
    ptr_len: Option<(NonNull<u8>, usize)>,
}

impl Default for SharedBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedBuf {
    /// An inactive buffer. Shared access uses the slot model until [`set`] is
    /// called with a non-empty buffer.
    ///
    /// [`set`]: SharedBuf::set
    pub fn new() -> Self {
        Self { ptr_len: None }
    }

    /// Capture the host buffer for the current call.
    ///
    /// An empty slice is treated as "no buffer": the buffer goes inactive and
    /// the caller routes shared access to the slot model. This is the
    /// coexistence path for hosts that drive the virtual machine through the
    /// plain `call`/`resume` and populate shared slots through `set_data`.
    pub fn set(&mut self, buf: &mut [u8]) {
        self.ptr_len = if buf.is_empty() {
            None
        } else {
            // Capturing the pointer is itself safe; the dereference contract is
            // documented at the module level and discharged by the call
            // boundary. `as_mut_ptr` of a non-empty slice is never null.
            NonNull::new(buf.as_mut_ptr()).map(|p| (p, buf.len()))
        };
    }

    /// Release the borrowed buffer. The virtual machine calls this before the
    /// `call_with_shared`/`resume_with_shared` entry point returns, so a stale
    /// pointer is never observable.
    pub fn clear(&mut self) {
        self.ptr_len = None;
    }

    /// Whether a host buffer is currently borrowed.
    pub fn is_active(&self) -> bool {
        self.ptr_len.is_some()
    }

    /// The borrowed buffer as a shared slice, or `None` when inactive.
    pub fn bytes(&self) -> Option<&[u8]> {
        self.ptr_len.map(|(ptr, len)| {
            // SAFETY: by the module invariant, `ptr` addresses `len` bytes of
            // the host buffer whose `&mut [u8]` borrow is live for the whole
            // call, and no other slice into it is held concurrently, so an
            // immutable view is sound.
            unsafe { core::slice::from_raw_parts(ptr.as_ptr(), len) }
        })
    }

    /// The borrowed buffer as a mutable slice, or `None` when inactive.
    pub fn bytes_mut(&mut self) -> Option<&mut [u8]> {
        self.ptr_len.map(|(ptr, len)| {
            // SAFETY: as `bytes`. The `&mut self` receiver together with the
            // no-concurrent-slice invariant makes this the unique live slice
            // into the buffer, so a mutable view is sound.
            unsafe { core::slice::from_raw_parts_mut(ptr.as_ptr(), len) }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_by_default_and_after_clear() {
        let mut s = SharedBuf::new();
        assert!(!s.is_active());
        assert!(s.bytes().is_none());
        let mut buf = [1u8, 2, 3, 4];
        s.set(&mut buf);
        assert!(s.is_active());
        s.clear();
        assert!(!s.is_active());
        assert!(s.bytes().is_none());
    }

    #[test]
    fn empty_buffer_is_inactive() {
        let mut s = SharedBuf::new();
        let mut empty: [u8; 0] = [];
        s.set(&mut empty);
        assert!(!s.is_active(), "an empty buffer means the slot model");
    }

    #[test]
    fn round_trips_bytes_through_the_captured_pointer() {
        let mut buf = [10u8, 20, 30, 40];
        let mut s = SharedBuf::new();
        s.set(&mut buf);
        // Read back through the captured pointer while the borrow is live.
        assert_eq!(s.bytes().unwrap(), &[10, 20, 30, 40]);
        // Mutate through the captured pointer.
        s.bytes_mut().unwrap()[1] = 99;
        s.clear();
        // The host buffer reflects the write after the borrow is released.
        assert_eq!(buf, [10, 99, 30, 40]);
    }
}
