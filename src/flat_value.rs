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

use alloc::vec;
use alloc::vec::Vec;
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
/// The body is built `Inline` (an owned `Vec<u8>`) and may be migrated to
/// the arena's top ephemeral head via [`FlatComposite::in_arena`], after
/// which it is an epoch-guarded [`ArenaHandle`] (B28 P2 arena residence,
/// mirroring [`crate::kstring::KString`]). Hosts and constants keep the
/// `Inline` form; the VM migrates a freshly-built body to the arena so
/// composites carry no global-heap allocation across a `loop` iteration's
/// `RESET`.
///
/// Validity and equality are orthogonal. A read first resolves the handle
/// against the arena ([`FlatComposite::resolve`], which fails `Stale` if a
/// `RESET` advanced the epoch since the body was issued), then the
/// resolved bytes are read or compared. [`FlatComposite::eq_in_arena`]
/// composes the two: it requires both bodies to resolve, then compares
/// their content. The `Inline` form is always valid and is read directly.
#[derive(Debug, Clone)]
pub enum FlatComposite {
    /// An owned byte body on the global heap. Built by the construction
    /// choke points, host marshalling, and constant materialisation.
    Inline(Vec<u8>),
    /// A body resident in the arena's top ephemeral head, addressed by an
    /// epoch-guarded handle. Built by [`FlatComposite::in_arena`] and read
    /// only through [`FlatComposite::resolve`] against the owning arena.
    Arena(ArenaHandle<[u8]>),
}

impl FlatComposite {
    /// A zero-initialised body of `size` bytes. Zero is a valid initial
    /// value for every fixed-size scalar (`false`, `0`, `+0.0`) and
    /// selects an enum's first declared variant through its zero
    /// discriminant byte. Built `Inline`; the VM migrates it to the arena
    /// after packing (B28 P2).
    pub fn zeroed(size: usize) -> Self {
        Self::Inline(vec![0u8; size])
    }

    /// A body wrapping already-packed bytes, `Inline`.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self::Inline(bytes)
    }

    /// Migrate an `Inline` body to the arena's top ephemeral head, copying
    /// its bytes and capturing the current epoch (B28 P2). An already-arena
    /// body and an empty body are returned unchanged (an empty body needs
    /// no allocation and has no stable pointer). Mirrors
    /// [`crate::kstring::KString::alloc`].
    pub fn in_arena(self, arena: &Arena) -> Result<Self, AllocError> {
        match self {
            Self::Arena(_) => Ok(self),
            Self::Inline(v) if v.is_empty() => Ok(Self::Inline(v)),
            Self::Inline(v) => {
                let buffer = arena.alloc_top_bytes(v.len())?;
                let dst = buffer.as_ptr() as *mut u8;
                // SAFETY: `buffer` is unique storage of `v.len()` bytes
                // freshly allocated from the arena's top head; the source
                // is a valid byte slice; the regions do not overlap because
                // the allocator returns previously unused memory. Mirrors
                // `KString::alloc`.
                unsafe { core::ptr::copy_nonoverlapping(v.as_ptr(), dst, v.len()) };
                let raw: *mut [u8] = core::ptr::slice_from_raw_parts_mut(dst, v.len());
                // SAFETY: `raw` is non-null because `dst` came from a
                // successful arena allocation.
                let nn = unsafe { NonNull::new_unchecked(raw) };
                // SAFETY: `nn` references storage in the arena's top region
                // freshly allocated under the current epoch.
                let handle = unsafe { ArenaHandle::from_raw_parts(nn, arena.epoch()) };
                Ok(Self::Arena(handle))
            }
        }
    }

    /// Copy an `Arena` body's bytes back into an owned `Inline` body
    /// (B28 P2). An `Inline` body is returned unchanged. A stale `Arena`
    /// body (its epoch no longer matches) yields an empty body, which is
    /// the conservative reading of a body that no longer exists; live
    /// callers never materialise a stale body. Used to bridge an arena body
    /// across a boundary that has no arena (host marshalling) or that reads
    /// bytes without one (the shared construction packer, value equality).
    pub fn to_inline(self, arena: &Arena) -> Self {
        match self {
            Self::Inline(v) => Self::Inline(v),
            Self::Arena(handle) => {
                let bytes = handle.get(arena).map(|b| b.to_vec()).unwrap_or_default();
                Self::Inline(bytes)
            }
        }
    }

    /// Resolve the body to its bytes against `arena` (B28 P2). An `Inline`
    /// body is read directly and is always valid; an `Arena` body resolves
    /// its handle, returning [`Stale`] if a `RESET` advanced the epoch
    /// since the body was issued. Both borrows share the call scope so the
    /// returned slice never outlives the arena.
    pub fn resolve<'a>(&'a self, arena: &'a Arena) -> Result<&'a [u8], Stale> {
        match self {
            Self::Inline(v) => Ok(v.as_slice()),
            Self::Arena(handle) => handle.get(arena),
        }
    }

    /// Whether the body still exists (the `if_exists` half of equality):
    /// `true` for an `Inline` body, and for an `Arena` body only while its
    /// epoch matches the arena (B28 P2).
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

    /// Byte length of an `Inline` body.
    ///
    /// Panics on an `Arena` body, which has no length without the arena;
    /// callers with the arena use [`FlatComposite::resolve`] and read the
    /// slice length. Construction and the not-yet-migrated read sites
    /// operate on `Inline` bodies (B28 P2 arena residence migrates the
    /// arena-aware read sites to `resolve`).
    pub fn len(&self) -> usize {
        match self {
            Self::Inline(v) => v.len(),
            Self::Arena(_) => {
                panic!("FlatComposite::len on an arena body; resolve(arena) and read its length")
            }
        }
    }

    /// True when an `Inline` body has no bytes (the `Unit`-only case).
    /// Panics on an `Arena` body, like [`FlatComposite::len`].
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Inline(v) => v.is_empty(),
            Self::Arena(_) => {
                panic!("FlatComposite::is_empty on an arena body; resolve(arena) instead")
            }
        }
    }

    /// The bytes of an `Inline` body, or `None` for an `Arena` body
    /// (B28 P2). Lets a caller that has no arena (notably `PartialEq`) read
    /// an inline body's content and decline an arena body rather than
    /// panic, keeping validity and equality orthogonal.
    pub fn inline_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Inline(v) => Some(v),
            Self::Arena(_) => None,
        }
    }

    /// The bytes of an `Inline` body, for reading a field at a baked
    /// offset. Panics on an `Arena` body; arena-aware callers use
    /// [`FlatComposite::resolve`] (B28 P2).
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Inline(v) => v,
            Self::Arena(_) => {
                panic!("FlatComposite::as_bytes on an arena body; use resolve(arena)")
            }
        }
    }

    /// The bytes of an `Inline` body, mutable, for packing fields during
    /// construction. Panics on an `Arena` body, which is immutable after
    /// migration.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        match self {
            Self::Inline(v) => v,
            Self::Arena(_) => {
                panic!("FlatComposite::as_bytes_mut on an arena body; arena bodies are immutable")
            }
        }
    }

    /// Copy `src` into an `Inline` body at `offset`, for packing a scalar's
    /// little-endian bytes or a nested composite's body inline. Panics if
    /// the range is out of bounds (a correct compiler-baked offset never
    /// produces this) or if the body is an `Arena` body.
    pub fn write_at(&mut self, offset: usize, src: &[u8]) {
        self.as_bytes_mut()[offset..offset + src.len()].copy_from_slice(src);
    }

    /// Borrow `len` bytes at `offset` of an `Inline` body, for reading a
    /// nested composite's body or a field's raw bytes. Panics on an
    /// `Arena` body; arena-aware callers resolve first (B28 P2).
    pub fn slice_at(&self, offset: usize, len: usize) -> &[u8] {
        &self.as_bytes()[offset..offset + len]
    }
}

/// Content equality, valid for `Inline` bodies (B28 P2).
///
/// `Inline` bodies compare by content. An `Arena` body cannot be read
/// without the arena, which `PartialEq` does not have, so any pair
/// involving an `Arena` body compares unequal here; the arena-aware
/// [`FlatComposite::eq_in_arena`] is the correct comparison for arena
/// bodies and is what the VM uses. The two halves stay orthogonal:
/// validity is established by `resolve`, content by the byte compare.
impl PartialEq for FlatComposite {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Inline(a), Self::Inline(b)) => a == b,
            _ => false,
        }
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

    // The layout descriptor below stands in for the compiler computing
    // the byte size and the field offsets; the runtime body carries
    // neither, only the bytes. The tests pack and read at the
    // layout-computed offsets, which is what the baked access ops do.

    #[test]
    fn flat_composite_construction_size_matches_layout() {
        let layout = LayoutDescriptor::Tuple(alloc::vec![
            LayoutDescriptor::Scalar(ScalarKind::Int),
            LayoutDescriptor::Scalar(ScalarKind::Bool),
        ]);
        let comp = FlatComposite::zeroed(layout.size_in_bytes(I64_BYTES, F64_BYTES));
        assert_eq!(comp.len(), 8 + 1);
        assert!(comp.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn flat_composite_struct() {
        let layout = LayoutDescriptor::Struct {
            type_name: "Point".to_string(),
            fields: alloc::vec![
                ("x".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
                ("y".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
            ],
        };
        let mut comp = FlatComposite::zeroed(layout.size_in_bytes(I64_BYTES, F64_BYTES));
        assert_eq!(comp.len(), 16);

        let x_off = layout
            .struct_field_offset("x", I64_BYTES, F64_BYTES)
            .unwrap();
        let y_off = layout
            .struct_field_offset("y", I64_BYTES, F64_BYTES)
            .unwrap();
        write_i64(comp.as_bytes_mut(), x_off, 3);
        write_i64(comp.as_bytes_mut(), y_off, 4);
        assert_eq!(read_i64(comp.as_bytes(), x_off), 3);
        assert_eq!(read_i64(comp.as_bytes(), y_off), 4);
    }

    #[test]
    fn flat_composite_array() {
        let layout = LayoutDescriptor::Array {
            element: Box::new(LayoutDescriptor::Scalar(ScalarKind::Int)),
            count: 4,
        };
        let mut comp = FlatComposite::zeroed(layout.size_in_bytes(I64_BYTES, F64_BYTES));
        assert_eq!(comp.len(), 32);

        for i in 0..4 {
            let off = layout.field_offset(i, I64_BYTES, F64_BYTES).unwrap();
            write_i64(comp.as_bytes_mut(), off, (i as i64) * 10);
        }
        for i in 0..4 {
            let off = layout.field_offset(i, I64_BYTES, F64_BYTES).unwrap();
            assert_eq!(read_i64(comp.as_bytes(), off), (i as i64) * 10);
        }
    }

    #[test]
    fn flat_composite_mixed_field_types() {
        let layout = LayoutDescriptor::Tuple(alloc::vec![
            LayoutDescriptor::Scalar(ScalarKind::Bool),
            LayoutDescriptor::Scalar(ScalarKind::Int),
            LayoutDescriptor::Scalar(ScalarKind::Byte),
        ]);
        let mut comp = FlatComposite::zeroed(layout.size_in_bytes(I64_BYTES, F64_BYTES));
        assert_eq!(comp.len(), 1 + 8 + 1);

        let off_bool = layout.field_offset(0, I64_BYTES, F64_BYTES).unwrap();
        let off_int = layout.field_offset(1, I64_BYTES, F64_BYTES).unwrap();
        let off_byte = layout.field_offset(2, I64_BYTES, F64_BYTES).unwrap();

        write_bool(comp.as_bytes_mut(), off_bool, true);
        write_i64(comp.as_bytes_mut(), off_int, -123);
        write_byte(comp.as_bytes_mut(), off_byte, 0xAB);

        assert!(read_bool(comp.as_bytes(), off_bool));
        assert_eq!(read_i64(comp.as_bytes(), off_int), -123);
        assert_eq!(read_byte(comp.as_bytes(), off_byte), 0xAB);
    }

    #[test]
    fn flat_composite_nested_inline_copy() {
        // A nested composite's body copies inline at an offset, and
        // reads back as the same byte range. This is how a nested
        // composite field is packed and read in the flat representation.
        let inner = FlatComposite::from_bytes(alloc::vec![1, 2, 3, 4]);
        let mut outer = FlatComposite::zeroed(10);
        outer.write_at(2, inner.as_bytes());
        assert_eq!(outer.slice_at(2, inner.len()), &[1, 2, 3, 4]);
        assert_eq!(outer.as_bytes(), &[0, 0, 1, 2, 3, 4, 0, 0, 0, 0]);
    }

    // -- Arena residence (B28 P2) --

    fn test_arena() -> Arena {
        Arena::with_capacity(4096)
    }

    #[test]
    fn arena_body_resolves_to_its_bytes() {
        let arena = test_arena();
        let body = FlatComposite::from_bytes(alloc::vec![1, 2, 3, 4])
            .in_arena(&arena)
            .unwrap();
        assert!(matches!(body, FlatComposite::Arena(_)));
        assert_eq!(body.resolve(&arena).unwrap(), &[1, 2, 3, 4]);
        assert!(body.is_valid(&arena));
    }

    #[test]
    fn arena_equality_is_content_not_handle() {
        // Two distinct arena allocations of the same content compare equal
        // (validity then content), unlike a handle-keyed equality.
        let arena = test_arena();
        let a = FlatComposite::from_bytes(alloc::vec![7, 8, 9])
            .in_arena(&arena)
            .unwrap();
        let b = FlatComposite::from_bytes(alloc::vec![7, 8, 9])
            .in_arena(&arena)
            .unwrap();
        let c = FlatComposite::from_bytes(alloc::vec![7, 8, 0])
            .in_arena(&arena)
            .unwrap();
        assert!(a.eq_in_arena(&b, &arena));
        assert!(!a.eq_in_arena(&c, &arena));
    }

    #[test]
    fn inline_and_arena_same_content_compare_equal_in_arena() {
        // Equality is content-based across the two representations.
        let arena = test_arena();
        let inline = FlatComposite::from_bytes(alloc::vec![5, 6, 7, 8]);
        let arena_body = FlatComposite::from_bytes(alloc::vec![5, 6, 7, 8])
            .in_arena(&arena)
            .unwrap();
        assert!(inline.eq_in_arena(&arena_body, &arena));
        assert!(arena_body.eq_in_arena(&inline, &arena));
    }

    #[test]
    fn reset_makes_an_arena_body_stale() {
        // A RESET advances the epoch; the prior body no longer exists, so
        // it is invalid and equals nothing (validity is orthogonal to and
        // gates content equality).
        let mut arena = test_arena();
        let a = FlatComposite::from_bytes(alloc::vec![1, 2, 3])
            .in_arena(&arena)
            .unwrap();
        let b = FlatComposite::from_bytes(alloc::vec![1, 2, 3])
            .in_arena(&arena)
            .unwrap();
        assert!(a.eq_in_arena(&b, &arena));
        arena.reset().unwrap();
        assert!(!a.is_valid(&arena));
        assert!(a.resolve(&arena).is_err());
        assert!(!a.eq_in_arena(&b, &arena));
    }

    #[test]
    fn empty_body_stays_inline_under_in_arena() {
        // A zero-length body needs no allocation and has no stable pointer.
        let arena = test_arena();
        let empty = FlatComposite::zeroed(0).in_arena(&arena).unwrap();
        assert!(matches!(empty, FlatComposite::Inline(_)));
        assert_eq!(empty.resolve(&arena).unwrap(), &[] as &[u8]);
    }
}
