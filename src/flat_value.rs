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
/// The buffer is a `Vec<u8>` for now; a later B28 phase moves it to the
/// arena's top ephemeral head so composites carry no global-heap
/// allocation. Callers reach the bytes through the accessor methods
/// rather than the field directly, so that move does not churn call
/// sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatComposite {
    bytes: Vec<u8>,
}

impl FlatComposite {
    /// A zero-initialised body of `size` bytes. Zero is a valid initial
    /// value for every fixed-size scalar (`false`, `0`, `+0.0`) and
    /// selects an enum's first declared variant through its zero
    /// discriminant byte.
    pub fn zeroed(size: usize) -> Self {
        Self {
            bytes: vec![0u8; size],
        }
    }

    /// A body wrapping already-packed bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Byte length of the body.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// True when the body has no bytes (the `Unit`-only case).
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The body's bytes, for reading a field at a baked offset.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The body's bytes, mutable, for writing a field at a baked offset.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    /// Copy `src` into the body at `offset`, for packing a scalar's
    /// little-endian bytes or a nested composite's body inline. Panics
    /// if the range is out of bounds, which a correct compiler-baked
    /// offset never produces.
    pub fn write_at(&mut self, offset: usize, src: &[u8]) {
        self.bytes[offset..offset + src.len()].copy_from_slice(src);
    }

    /// Borrow `len` bytes at `offset`, for reading a nested composite's
    /// body or a field's raw bytes.
    pub fn slice_at(&self, offset: usize, len: usize) -> &[u8] {
        &self.bytes[offset..offset + len]
    }
}

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
}
