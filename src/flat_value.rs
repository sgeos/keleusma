//! Flat-byte composite representation for Keleusma values.
//!
//! Introduced as B28 P0 infrastructure
//! (see [`docs/decisions/BACKLOG.md`](../../docs/decisions/BACKLOG.md)).
//! It defines the byte-level read and write helpers for fixed-
//! size primitive types and the [`crate::flat_value::FlatComposite`]
//! byte buffer that holds a composite value's fields packed
//! contiguously. A composite is pure bytes; the field offsets and
//! kinds are baked into the access instructions by the compiler, so
//! the body carries no layout reference.
//!
//! The helpers are little-endian throughout. Keleusma's wire
//! format is little-endian (see [`crate::wire_format`]); the
//! flat-byte composite representation follows the same
//! convention so the runtime can use the same conversion
//! routines on both sides of the wire boundary.
//!
//! The bundled runtime uses `Word = i64` and `Float = f64`, and
//! the helpers in this module target those widths directly. The
//! parametric runtime (see [`crate::vm::GenericVm`] and B16)
//! will extend the helper set to other word and float widths in
//! subsequent B28 phases. P0 establishes the foundation for the
//! bundled case so the rest of the migration can proceed
//! incrementally.
//!
//! P2 migrates the composite runtime representation onto this
//! foundation. `LayoutDescriptor` (see [`crate::value_layout`]) and the
//! layout pass remain compile-time only; they bake the offsets and
//! compute the worst-case-memory-usage bound and are never carried on a
//! value.

extern crate alloc;

use core::ptr::NonNull;

use allocator_api2::alloc::AllocError;
use keleusma_arena::{Arena, ArenaHandle, Stale};

/// Write a boolean to the byte buffer at the given offset.
///
/// Stores `1u8` for `true` and `0u8` for `false`. Panics if
/// `offset` is out of bounds.
pub fn write_bool(bytes: &mut [u8], offset: usize, value: bool) {
    bytes[offset] = u8::from(value);
}

/// Read a boolean from the byte buffer at the given offset.
///
/// Returns `true` for any non-zero byte and `false` for `0u8`.
/// Panics if `offset` is out of bounds.
pub fn read_bool(bytes: &[u8], offset: usize) -> bool {
    bytes[offset] != 0
}

/// Write a byte to the buffer at the given offset.
///
/// Panics if `offset` is out of bounds.
pub fn write_byte(bytes: &mut [u8], offset: usize, value: u8) {
    bytes[offset] = value;
}

/// Read a byte from the buffer at the given offset.
///
/// Panics if `offset` is out of bounds.
pub fn read_byte(bytes: &[u8], offset: usize) -> u8 {
    bytes[offset]
}

/// Write a 64-bit signed integer to the byte buffer as eight
/// little-endian bytes.
///
/// Used for the bundled runtime's `Word = i64` case. Panics if
/// the buffer does not have at least eight bytes available at
/// `offset`.
pub fn write_i64(bytes: &mut [u8], offset: usize, value: i64) {
    let le = value.to_le_bytes();
    bytes[offset..offset + 8].copy_from_slice(&le);
}

/// Read a 64-bit signed integer from the byte buffer as eight
/// little-endian bytes.
///
/// Used for the bundled runtime's `Word = i64` case. Panics if
/// the buffer does not have at least eight bytes available at
/// `offset`.
pub fn read_i64(bytes: &[u8], offset: usize) -> i64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[offset..offset + 8]);
    i64::from_le_bytes(buf)
}

/// Write a 64-bit floating-point value to the byte buffer as
/// eight little-endian bytes (IEEE 754 double).
///
/// Used for the bundled runtime's `Float = f64` case. Panics if
/// the buffer does not have at least eight bytes available at
/// `offset`.
#[cfg(feature = "floats")]
pub fn write_f64(bytes: &mut [u8], offset: usize, value: f64) {
    let le = value.to_le_bytes();
    bytes[offset..offset + 8].copy_from_slice(&le);
}

/// Read a 64-bit floating-point value from the byte buffer as
/// eight little-endian bytes.
///
/// Used for the bundled runtime's `Float = f64` case. Panics if
/// the buffer does not have at least eight bytes available at
/// `offset`.
#[cfg(feature = "floats")]
pub fn read_f64(bytes: &[u8], offset: usize) -> f64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[offset..offset + 8]);
    f64::from_le_bytes(buf)
}

/// The flat-byte body of a composite value.
///
/// A composite is pure bytes (B28). The field offsets and kinds are
/// baked into the access instructions by the compiler, so the body
/// carries no layout reference, no template index, and no `Arc`. It is
/// just the byte buffer holding the fields packed contiguously, read
/// and written through the scalar helpers in this module at the offsets
/// the compiler resolved.
///
/// A non-empty body is built directly in an arena region via
/// [`FlatComposite::build_in_arena`], after which it is an epoch-guarded
/// [`ArenaHandle`] (B28 P2 arena residence, mirroring
/// [`crate::kstring::KString`]). The handle may point into the arena's top
/// ephemeral head (reclaimed at `RESET`), the persistent region (survives
/// `RESET` in place, B28 item 2 step 6A), or VM-owned off-arena rodata such
/// as the const-composite pool (always live, sentinel epoch `0`). Region-aware
/// validity decides liveness by where the pointer falls. An empty body (the
/// `Unit`-only case) carries no allocation and is the zero-size sentinel handle
/// from [`FlatComposite::empty`]. The owned-bytes `Inline` form is gone (B28
/// item 2 step 6B): every
/// flat body now lives in an arena region, so a single pointer-and-length
/// handle is the only non-empty representation and `FlatComposite` is the
/// handle's size, which collapses `Value` to 32 bytes. Host marshalling and
/// constants that carry no arena handle use the boxed `GenericValue`
/// representation instead (the `*_with_widths` constructors and the boxed
/// const path).
///
/// The handle carries the arena epoch its arena-referencing fields (a flat
/// `Text` field's `(ptr, len)`) belong to (B28 P3 item 1), so a `Text` field
/// read after a `RESET` reattaches that epoch and resolves to a clean `Stale`
/// outcome rather than dereferencing reclaimed memory. The epoch is a validity
/// attribute and is not part of content equality.
///
/// Validity and equality are orthogonal. A read first resolves the handle
/// against the arena ([`FlatComposite::resolve`], which fails `Stale` if a
/// `RESET` advanced the epoch since the body was issued), then the
/// resolved bytes are read or compared. [`FlatComposite::eq_in_arena`]
/// composes the two: it requires both bodies to resolve, then compares
/// their content.
///
/// The enum has a SINGLE variant so the handle's `NonNull` niche stays exposed
/// for the surrounding body enums (`TupleBody`/`ArrayBody`/`StructBody`/
/// `EnumBody`) to reuse for their `Flat`/`Boxed` discriminant; a second
/// data-less variant (an `Empty` marker) would spend that niche on its own
/// discriminant and pin `Value` at 40 bytes rather than 32 (the empirically
/// measured layout fact, B28 P3 item 5 session 7). The empty body (the
/// `Unit`-only composite) is therefore not a separate variant but a sentinel
/// handle: a dangling, well-aligned non-null pointer of length zero under the
/// always-live sentinel epoch `0`, built by [`FlatComposite::empty`]. Resolving
/// it yields `&[]` without dereferencing storage, since a zero-length slice
/// from any aligned non-null pointer is valid.
#[derive(Debug, Clone)]
pub enum FlatComposite {
    /// A body resident in an arena region, addressed by an epoch-guarded
    /// handle. Built by [`FlatComposite::build_in_arena`] and read only through
    /// [`FlatComposite::resolve`] against the owning arena. The zero-length
    /// empty body is the sentinel handle from [`FlatComposite::empty`].
    Arena(ArenaHandle<[u8]>),
}

impl FlatComposite {
    /// The empty body sentinel (the `Unit`-only composite): a dangling,
    /// well-aligned non-null pointer of length zero under the always-live
    /// sentinel epoch `0` (B28 item 2 step 6B). Resolving it yields `&[]`
    /// without dereferencing storage. Encoded as a handle rather than a second
    /// enum variant so the handle's `NonNull` niche stays exposed and `Value`
    /// reaches 32 bytes.
    pub fn empty() -> Self {
        // SAFETY: `NonNull::<u8>::dangling()` is well-aligned and non-null; a
        // zero-length slice pointer built from it is a valid `NonNull<[u8]>`
        // that is never dereferenced (resolve returns a zero-length slice). The
        // sentinel epoch `0` marks it always-live rodata-like, the same model as
        // the const pool and a rodata `KStr`; the dangling address is outside
        // any arena range, so region-aware validity treats it as always live.
        let raw: *mut [u8] =
            core::ptr::slice_from_raw_parts_mut(NonNull::<u8>::dangling().as_ptr(), 0);
        let nn = unsafe { NonNull::new_unchecked(raw) };
        let handle = unsafe { ArenaHandle::from_raw_parts(nn, 0) };
        Self::Arena(handle)
    }

    /// The arena epoch this body's arena-referencing fields (a flat `Text`
    /// field's `(ptr, len)`) belong to (B28 P3 item 1): the handle's captured
    /// epoch (the empty sentinel reports `0`). A `Text` read reattaches this
    /// epoch so a read after a `RESET` resolves `Stale`.
    pub fn ref_epoch(&self) -> u64 {
        let Self::Arena(handle) = self;
        handle.epoch()
    }

    /// Identity, retained for the arena-migration callers (B28 item 2 step 6B).
    /// Every flat body is already an arena region handle, so there is nothing to
    /// migrate; the owned-bytes `Inline` form that this once copied into the
    /// arena is gone. Kept so
    /// [`crate::bytecode::GenericValue::into_arena_body`] can call it
    /// uniformly.
    pub fn in_arena(self, _arena: &Arena) -> Result<Self, AllocError> {
        Ok(self)
    }

    /// Build a body of `size` bytes directly in an arena region. The `fill`
    /// closure receives the freshly allocated destination slice and packs every
    /// byte: the arena returns *uninitialised* storage, so `fill` is responsible
    /// for writing the whole `size`-byte range (the packed fields and any
    /// trailing padding slack).
    ///
    /// A zero-length body needs no allocation and has no stable pointer, so it
    /// is returned as the [`FlatComposite::empty`] sentinel. A non-empty body is
    /// allocated from the arena top and returned as an `Arena` handle capturing
    /// the current epoch.
    ///
    /// `fill` returns `Err(())` only on an internal inconsistency a correct
    /// caller never produces (for example a freshly constructed child body
    /// failing to resolve, which cannot happen because no `RESET` intervenes
    /// between a child's construction and its parent's). On that error the
    /// already-reserved arena bytes are abandoned to the next `RESET` and
    /// `None` is returned so the caller can fall back.
    pub fn build_in_arena(
        arena: &Arena,
        size: usize,
        fill: impl FnOnce(&mut [u8]) -> Result<(), ()>,
    ) -> Result<Option<Self>, AllocError> {
        if size == 0 {
            return Ok(Some(Self::empty()));
        }
        let buffer = arena.alloc_top_bytes(size)?;
        let dst_ptr = buffer.as_ptr() as *mut u8;
        // SAFETY: `buffer` is a unique, freshly allocated run of `size` bytes
        // from the arena's top head; no other reference aliases this range
        // until the next reset. The bytes are uninitialised, so `fill` must
        // write all of them, which the caller's packer does.
        let dst = unsafe { core::slice::from_raw_parts_mut(dst_ptr, size) };
        if fill(dst).is_err() {
            return Ok(None);
        }
        let raw: *mut [u8] = core::ptr::slice_from_raw_parts_mut(dst_ptr, size);
        // SAFETY: `raw` is non-null because `dst_ptr` came from a successful
        // arena allocation.
        let nn = unsafe { NonNull::new_unchecked(raw) };
        // SAFETY: `nn` references storage in the arena's top region freshly
        // allocated under the current epoch.
        let handle = unsafe { ArenaHandle::from_raw_parts(nn, arena.epoch()) };
        Ok(Some(Self::Arena(handle)))
    }

    /// View a nested child composite occupying `[offset, offset + len)` of
    /// this body, without copying (B28 P3 item 5 C-residual 3b, B28 item 2 step
    /// 6B). A nested field access extracts the child body; for an `Arena`
    /// parent the child is a sub-range of the parent's single arena allocation,
    /// returned as a sub-handle pointing at `parent_ptr + offset` with the
    /// parent's epoch, sharing the parent's storage and going stale exactly
    /// when the parent does. The empty sentinel parent admits only the empty
    /// child (`offset == len == 0`), itself the empty sentinel; and a child
    /// range of length zero is the empty sentinel regardless.
    ///
    /// Returns [`Stale`] if an `Arena` parent no longer resolves, which a
    /// correct caller never observes (the parent was just on the operand
    /// stack). The compiler-baked `offset`/`len` always lie within the body.
    pub fn nested_view(&self, offset: usize, len: usize, arena: &Arena) -> Result<Self, Stale> {
        let Self::Arena(handle) = self;
        let base = handle.get(arena)?;
        // Bounds guard (audit finding B1). For compiler-produced bytecode the
        // baked `offset`/`len` always lie within the body, and the A.2.1 typed
        // operand-stack pass rejects an out-of-bounds `FlatNested` operand at
        // load whenever it can reconstruct the parent's flat shape. That pass
        // is not yet complete, though (an operand of unknown shape defers), so
        // this remains a real runtime check rather than a `debug_assert`:
        // untrusted or corrupt bytecode that slips a bad offset past the
        // load-time pass must fault here, not perform out-of-bounds pointer
        // arithmetic in a release build. An out-of-bounds range surfaces
        // through the same `Stale` channel the caller already traps on. When
        // the pass reaches completeness this guard may be lifted (A.2.1 Phase
        // 6B, the zero-copy payoff).
        if offset.checked_add(len).is_none_or(|end| end > base.len()) {
            return Err(Stale);
        }
        if len == 0 {
            return Ok(Self::empty());
        }
        // SAFETY: the bounds guard above returned early unless
        // `offset + len <= base.len()`, so `base.as_ptr().add(offset)` is in
        // bounds and the `len`-byte sub-slice lies within the parent's
        // allocation.
        let child_ptr = unsafe { base.as_ptr().add(offset) } as *mut u8;
        let raw: *mut [u8] = core::ptr::slice_from_raw_parts_mut(child_ptr, len);
        // SAFETY: `child_ptr` is derived from a non-null arena pointer.
        let nn = unsafe { NonNull::new_unchecked(raw) };
        // SAFETY: the sub-range lives in the same arena allocation under the
        // same epoch as the parent handle, so it is valid for as long as the
        // parent is.
        let child = unsafe { ArenaHandle::from_raw_parts(nn, handle.epoch()) };
        Ok(Self::Arena(child))
    }

    /// Resolve the body to its bytes against `arena` (B28 P2). The empty
    /// sentinel resolves to `&[]` and is always valid; a non-empty `Arena` body
    /// resolves its handle, returning [`Stale`] if a `RESET` advanced the epoch
    /// since the body was issued. Both borrows share the call scope so the
    /// returned slice never outlives the arena.
    pub fn resolve<'a>(&'a self, arena: &'a Arena) -> Result<&'a [u8], Stale> {
        let Self::Arena(handle) = self;
        handle.get(arena)
    }

    /// Whether the body still exists (the `if_exists` half of equality):
    /// `true` for the empty sentinel, and for a non-empty `Arena` body only
    /// while its epoch matches the arena (B28 P2).
    pub fn is_valid(&self, arena: &Arena) -> bool {
        self.resolve(arena).is_ok()
    }

    /// Content equality gated on validity (B28 P2): both bodies must
    /// resolve against `arena` (`if_exists`), then their bytes are compared
    /// (`if_equals`). A stale body equals nothing. This keeps composite
    /// equality content-based rather than keying on the handle, so two
    /// equal-content bodies in distinct allocations compare equal.
    pub fn eq_in_arena(&self, other: &Self, arena: &Arena) -> bool {
        match (self.resolve(arena), other.resolve(arena)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }

    /// Byte length of the body without the arena (B28 P3 item 5 C3), read from
    /// the handle's fat-pointer length metadata (the empty sentinel reports
    /// `0`). For an arena-less caller that needs only the size, for example a
    /// typeless display path rendering a placeholder, not the contents.
    pub fn byte_len(&self) -> usize {
        let Self::Arena(handle) = self;
        handle.len()
    }

    /// The bytes of an arena-less body, or `None` when an arena is required. The
    /// empty sentinel (length zero) returns `Some(&[])`; a non-empty `Arena`
    /// body returns `None` because it cannot be read without the arena. Lets a
    /// caller that has no arena (notably `PartialEq`) read an empty body's
    /// content and decline a non-empty arena body rather than panic, keeping
    /// validity and equality orthogonal.
    pub fn inline_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Arena(handle) if handle.is_empty() => Some(&[]),
            Self::Arena(_) => None,
        }
    }
}

/// Content equality without an arena (B28 P2, B28 item 2 step 6B).
///
/// Two empty bodies (length zero) are equal. A non-empty `Arena` body cannot be
/// read without the arena, which `PartialEq` does not have, so any pair
/// involving a non-empty body compares unequal here; the arena-aware
/// [`FlatComposite::eq_in_arena`] is the correct comparison and is what the VM
/// uses. The two halves stay orthogonal: validity is established by `resolve`,
/// content by the byte compare.
impl PartialEq for FlatComposite {
    fn eq(&self, other: &Self) -> bool {
        self.byte_len() == 0 && other.byte_len() == 0
    }
}

impl Eq for FlatComposite {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_layout::{LayoutDescriptor, ScalarKind};
    use alloc::boxed::Box;
    use alloc::string::ToString;

    const I64_BYTES: usize = 8;
    const F64_BYTES: usize = 8;

    #[test]
    fn bool_roundtrip_true() {
        let mut bytes = [0u8; 1];
        write_bool(&mut bytes, 0, true);
        assert_eq!(bytes[0], 1);
        assert!(read_bool(&bytes, 0));
    }

    #[test]
    fn bool_roundtrip_false() {
        let mut bytes = [0xFFu8; 1];
        write_bool(&mut bytes, 0, false);
        assert_eq!(bytes[0], 0);
        assert!(!read_bool(&bytes, 0));
    }

    #[test]
    fn bool_read_accepts_any_nonzero() {
        let bytes = [0x42u8; 1];
        assert!(read_bool(&bytes, 0));
    }

    #[test]
    fn byte_roundtrip() {
        let mut bytes = [0u8; 4];
        write_byte(&mut bytes, 2, 0xABu8);
        assert_eq!(bytes[2], 0xAB);
        assert_eq!(read_byte(&bytes, 2), 0xAB);
    }

    #[test]
    fn i64_roundtrip_positive() {
        let mut bytes = [0u8; 8];
        write_i64(&mut bytes, 0, 0x1234_5678_9ABC_DEF0);
        assert_eq!(read_i64(&bytes, 0), 0x1234_5678_9ABC_DEF0);
    }

    #[test]
    fn i64_roundtrip_negative() {
        let mut bytes = [0u8; 8];
        write_i64(&mut bytes, 0, -42);
        assert_eq!(read_i64(&bytes, 0), -42);
    }

    #[test]
    fn i64_roundtrip_boundary_values() {
        let mut bytes = [0u8; 8];
        write_i64(&mut bytes, 0, i64::MIN);
        assert_eq!(read_i64(&bytes, 0), i64::MIN);
        write_i64(&mut bytes, 0, i64::MAX);
        assert_eq!(read_i64(&bytes, 0), i64::MAX);
        write_i64(&mut bytes, 0, 0);
        assert_eq!(read_i64(&bytes, 0), 0);
    }

    #[test]
    fn i64_writes_little_endian() {
        let mut bytes = [0u8; 8];
        write_i64(&mut bytes, 0, 0x01);
        assert_eq!(bytes, [0x01, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn i64_roundtrip_at_offset() {
        let mut bytes = [0u8; 24];
        write_i64(&mut bytes, 8, 0x1234_5678_9ABC_DEF0);
        assert_eq!(read_i64(&bytes, 8), 0x1234_5678_9ABC_DEF0);
        assert_eq!(read_i64(&bytes, 0), 0);
        assert_eq!(read_i64(&bytes, 16), 0);
    }

    #[cfg(feature = "floats")]
    #[test]
    fn f64_roundtrip_positive() {
        let mut bytes = [0u8; 8];
        write_f64(&mut bytes, 0, core::f64::consts::PI);
        assert_eq!(read_f64(&bytes, 0), core::f64::consts::PI);
    }

    #[cfg(feature = "floats")]
    #[test]
    fn f64_roundtrip_negative_and_zero() {
        let mut bytes = [0u8; 8];
        write_f64(&mut bytes, 0, -1.5);
        assert_eq!(read_f64(&bytes, 0), -1.5);
        write_f64(&mut bytes, 0, 0.0);
        assert_eq!(read_f64(&bytes, 0), 0.0);
    }

    #[cfg(feature = "floats")]
    #[test]
    fn f64_roundtrip_boundary_values() {
        let mut bytes = [0u8; 8];
        write_f64(&mut bytes, 0, f64::MIN);
        assert_eq!(read_f64(&bytes, 0), f64::MIN);
        write_f64(&mut bytes, 0, f64::MAX);
        assert_eq!(read_f64(&bytes, 0), f64::MAX);
        write_f64(&mut bytes, 0, f64::EPSILON);
        assert_eq!(read_f64(&bytes, 0), f64::EPSILON);
    }

    #[cfg(feature = "floats")]
    #[test]
    fn f64_roundtrip_infinity_and_nan() {
        let mut bytes = [0u8; 8];
        write_f64(&mut bytes, 0, f64::INFINITY);
        assert_eq!(read_f64(&bytes, 0), f64::INFINITY);
        write_f64(&mut bytes, 0, f64::NEG_INFINITY);
        assert_eq!(read_f64(&bytes, 0), f64::NEG_INFINITY);
        write_f64(&mut bytes, 0, f64::NAN);
        assert!(read_f64(&bytes, 0).is_nan());
    }

    // -- Arena residence (B28 P2, B28 item 2 step 6B: arena-only bodies) --

    fn test_arena() -> Arena {
        Arena::with_capacity(4096)
    }

    /// Build an arena body of `bytes`, the test analogue of the VM's
    /// `pack_flat_in_arena`.
    fn arena_body(arena: &Arena, bytes: &[u8]) -> FlatComposite {
        FlatComposite::build_in_arena(arena, bytes.len(), |dst| {
            dst.copy_from_slice(bytes);
            Ok(())
        })
        .unwrap()
        .unwrap()
    }

    // The layout descriptor below stands in for the compiler computing
    // the byte size and the field offsets; the runtime body carries
    // neither, only the bytes. The tests pack at the layout-computed offsets
    // into the arena fill closure and read back through `resolve`, which is
    // what the baked access ops do.

    #[test]
    fn flat_composite_construction_size_matches_layout() {
        let arena = test_arena();
        let layout = LayoutDescriptor::Tuple(alloc::vec![
            LayoutDescriptor::Scalar(ScalarKind::Int),
            LayoutDescriptor::Scalar(ScalarKind::Bool),
        ]);
        let size = layout.size_in_bytes(I64_BYTES, F64_BYTES);
        let comp = FlatComposite::build_in_arena(&arena, size, |dst| {
            dst.fill(0);
            Ok(())
        })
        .unwrap()
        .unwrap();
        let bytes = comp.resolve(&arena).unwrap();
        assert_eq!(bytes.len(), 8 + 1);
        assert!(bytes.iter().all(|&b| b == 0));
    }

    #[test]
    fn flat_composite_struct() {
        let arena = test_arena();
        let layout = LayoutDescriptor::Struct {
            type_name: "Point".to_string(),
            fields: alloc::vec![
                ("x".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
                ("y".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
            ],
        };
        let size = layout.size_in_bytes(I64_BYTES, F64_BYTES);
        assert_eq!(size, 16);
        let x_off = layout
            .struct_field_offset("x", I64_BYTES, F64_BYTES)
            .unwrap();
        let y_off = layout
            .struct_field_offset("y", I64_BYTES, F64_BYTES)
            .unwrap();
        let comp = FlatComposite::build_in_arena(&arena, size, |dst| {
            dst.fill(0);
            write_i64(dst, x_off, 3);
            write_i64(dst, y_off, 4);
            Ok(())
        })
        .unwrap()
        .unwrap();
        let bytes = comp.resolve(&arena).unwrap();
        assert_eq!(read_i64(bytes, x_off), 3);
        assert_eq!(read_i64(bytes, y_off), 4);
    }

    #[test]
    fn flat_composite_array() {
        let arena = test_arena();
        let layout = LayoutDescriptor::Array {
            element: Box::new(LayoutDescriptor::Scalar(ScalarKind::Int)),
            count: 4,
        };
        let size = layout.size_in_bytes(I64_BYTES, F64_BYTES);
        assert_eq!(size, 32);
        let offsets: alloc::vec::Vec<usize> = (0..4)
            .map(|i| layout.field_offset(i, I64_BYTES, F64_BYTES).unwrap())
            .collect();
        let comp = FlatComposite::build_in_arena(&arena, size, |dst| {
            dst.fill(0);
            for (i, &off) in offsets.iter().enumerate() {
                write_i64(dst, off, (i as i64) * 10);
            }
            Ok(())
        })
        .unwrap()
        .unwrap();
        let bytes = comp.resolve(&arena).unwrap();
        for (i, &off) in offsets.iter().enumerate() {
            assert_eq!(read_i64(bytes, off), (i as i64) * 10);
        }
    }

    #[test]
    fn flat_composite_mixed_field_types() {
        let arena = test_arena();
        let layout = LayoutDescriptor::Tuple(alloc::vec![
            LayoutDescriptor::Scalar(ScalarKind::Bool),
            LayoutDescriptor::Scalar(ScalarKind::Int),
            LayoutDescriptor::Scalar(ScalarKind::Byte),
        ]);
        let size = layout.size_in_bytes(I64_BYTES, F64_BYTES);
        assert_eq!(size, 1 + 8 + 1);
        let off_bool = layout.field_offset(0, I64_BYTES, F64_BYTES).unwrap();
        let off_int = layout.field_offset(1, I64_BYTES, F64_BYTES).unwrap();
        let off_byte = layout.field_offset(2, I64_BYTES, F64_BYTES).unwrap();
        let comp = FlatComposite::build_in_arena(&arena, size, |dst| {
            dst.fill(0);
            write_bool(dst, off_bool, true);
            write_i64(dst, off_int, -123);
            write_byte(dst, off_byte, 0xAB);
            Ok(())
        })
        .unwrap()
        .unwrap();
        let bytes = comp.resolve(&arena).unwrap();
        assert!(read_bool(bytes, off_bool));
        assert_eq!(read_i64(bytes, off_int), -123);
        assert_eq!(read_byte(bytes, off_byte), 0xAB);
    }

    #[test]
    fn flat_composite_nested_view() {
        // A nested composite occupies a sub-range of the parent body; the
        // zero-copy `nested_view` returns a sub-handle resolving to those bytes.
        let arena = test_arena();
        let outer = arena_body(&arena, &[0, 0, 1, 2, 3, 4, 0, 0, 0, 0]);
        let inner = outer.nested_view(2, 4, &arena).unwrap();
        assert!(matches!(inner, FlatComposite::Arena(_)));
        assert_eq!(inner.resolve(&arena).unwrap(), &[1, 2, 3, 4]);
        assert_eq!(
            outer.resolve(&arena).unwrap(),
            &[0, 0, 1, 2, 3, 4, 0, 0, 0, 0]
        );
    }

    #[test]
    fn nested_view_out_of_bounds_faults_not_ub() {
        // A `FlatNested` offset/len past the parent body (untrusted or corrupt
        // bytecode that slipped past the load-time typed pass) must fault
        // rather than perform out-of-bounds pointer arithmetic (audit finding
        // B1). The guard is a real runtime check, not a `debug_assert`, so this
        // holds in release builds too.
        let arena = test_arena();
        let outer = arena_body(&arena, &[0, 1, 2, 3]); // 4-byte body
        assert!(outer.nested_view(2, 4, &arena).is_err()); // 2 + 4 > 4
        assert!(outer.nested_view(5, 0, &arena).is_err()); // offset past end
        assert!(outer.nested_view(usize::MAX, 1, &arena).is_err()); // overflow
        // An in-bounds range still succeeds.
        assert!(outer.nested_view(1, 2, &arena).is_ok());
    }

    #[test]
    fn arena_body_resolves_to_its_bytes() {
        let arena = test_arena();
        let body = arena_body(&arena, &[1, 2, 3, 4]);
        assert!(matches!(body, FlatComposite::Arena(_)));
        assert_eq!(body.resolve(&arena).unwrap(), &[1, 2, 3, 4]);
        assert!(body.is_valid(&arena));
    }

    #[test]
    fn arena_equality_is_content_not_handle() {
        // Two distinct arena allocations of the same content compare equal
        // (validity then content), unlike a handle-keyed equality.
        let arena = test_arena();
        let a = arena_body(&arena, &[7, 8, 9]);
        let b = arena_body(&arena, &[7, 8, 9]);
        let c = arena_body(&arena, &[7, 8, 0]);
        assert!(a.eq_in_arena(&b, &arena));
        assert!(!a.eq_in_arena(&c, &arena));
    }

    #[test]
    fn reset_makes_an_arena_body_stale() {
        // A RESET advances the epoch; the prior body no longer exists, so
        // it is invalid and equals nothing (validity is orthogonal to and
        // gates content equality).
        let mut arena = test_arena();
        let a = arena_body(&arena, &[1, 2, 3]);
        let b = arena_body(&arena, &[1, 2, 3]);
        assert!(a.eq_in_arena(&b, &arena));
        arena.reset().unwrap();
        assert!(!a.is_valid(&arena));
        assert!(a.resolve(&arena).is_err());
        assert!(!a.eq_in_arena(&b, &arena));
    }

    #[test]
    fn empty_body_is_the_empty_sentinel() {
        // A zero-length body needs no allocation and has no stable pointer, so
        // it is the always-valid empty sentinel handle resolving to `&[]`.
        let arena = test_arena();
        let empty = FlatComposite::build_in_arena(&arena, 0, |_| Ok(()))
            .unwrap()
            .unwrap();
        assert_eq!(empty.resolve(&arena).unwrap(), &[] as &[u8]);
        assert!(empty.is_valid(&arena));
        assert_eq!(empty.byte_len(), 0);
        assert_eq!(empty.inline_bytes(), Some(&[] as &[u8]));
        // The explicit constructor agrees with the zero-size build path.
        assert_eq!(FlatComposite::empty().byte_len(), 0);
        // Two empty bodies are equal without an arena (PartialEq).
        assert_eq!(FlatComposite::empty(), FlatComposite::empty());
    }

    #[test]
    fn value_is_thirty_two_bytes() {
        // The single-variant `FlatComposite` keeps the handle's `NonNull` niche
        // exposed so the body enums reuse it and `Value` collapses to 32 bytes
        // (the close of B28 item 2). A second data-less variant would pin it at
        // 40 (the measured layout fact, session 7).
        assert_eq!(core::mem::size_of::<crate::bytecode::Value>(), 32);
    }
}
