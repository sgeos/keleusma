//! Flat-byte composite representation for Keleusma values.
//!
//! This module is parallel infrastructure introduced in B28 P0
//! (see [`docs/decisions/BACKLOG.md`](../../docs/decisions/BACKLOG.md)).
//! It defines the byte-level read and write helpers for fixed-
//! size primitive types and the [`FlatComposite`] container that
//! pairs a byte buffer with a [`LayoutDescriptor`].
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
//! No runtime path consumes this module yet. P0 is parallel
//! infrastructure; subsequent phases (P1 through P5) migrate the
//! composite runtime representation onto this foundation.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use crate::value_layout::LayoutDescriptor;

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

/// Container pairing a byte buffer with the layout descriptor
/// that interprets it.
///
/// The bytes hold the flat-byte representation of a composite
/// value. The layout descriptor identifies the byte offset and
/// type of each field. The two together form a self-describing
/// composite value that can be read and written through the
/// scalar helpers in this module without losing track of which
/// bytes correspond to which fields.
///
/// The layout is held behind an `Arc` so multiple composite
/// values of the same type share one descriptor allocation. The
/// runtime is expected to interrogate the layout repeatedly
/// during op handler execution; the `Arc` clone is cheap and
/// keeps the descriptor immortal across composite lifetimes.
///
/// `FlatComposite` is not yet consumed by any runtime path. P0
/// is parallel infrastructure; subsequent B28 phases wire it
/// into the runtime's composite value representation.
#[derive(Debug, Clone)]
pub struct FlatComposite {
    /// The bytes that hold the composite's field data.
    pub bytes: Vec<u8>,
    /// The layout that interprets the bytes.
    pub layout: Arc<LayoutDescriptor>,
}

impl FlatComposite {
    /// Construct a zero-initialised flat composite of the
    /// supplied layout.
    ///
    /// The byte buffer is sized to the layout's
    /// [`LayoutDescriptor::size_in_bytes`] result and filled
    /// with zeros. Zero is a valid initialiser for every
    /// supported scalar type:
    /// - `Unit`: zero bytes, no initialisation needed.
    /// - `Bool`: `0u8` represents `false`.
    /// - `Byte`: `0u8` is the zero `Byte`.
    /// - `Int`: a zeroed buffer represents `0`.
    /// - `Fixed`: a zeroed buffer represents `0` in any
    ///   Q-format with non-negative integer-bit count.
    /// - `Float`: a zeroed buffer represents `+0.0` in IEEE
    ///   754.
    ///
    /// For enum layouts, the zeroed discriminant byte selects
    /// the first declared variant. Callers that need a
    /// different initial variant must write the discriminant
    /// after construction.
    pub fn new(layout: Arc<LayoutDescriptor>, word_bytes: usize, float_bytes: usize) -> Self {
        let size = layout.size_in_bytes(word_bytes, float_bytes);
        Self {
            bytes: vec![0u8; size],
            layout,
        }
    }

    /// Byte size of this composite.
    pub fn size_in_bytes(&self) -> usize {
        self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_layout::ScalarKind;
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

    #[test]
    fn flat_composite_construction_size_matches_layout() {
        let layout = Arc::new(LayoutDescriptor::Tuple(alloc::vec![
            LayoutDescriptor::Scalar(ScalarKind::Int),
            LayoutDescriptor::Scalar(ScalarKind::Bool),
        ]));
        let comp = FlatComposite::new(layout, I64_BYTES, F64_BYTES);
        assert_eq!(comp.size_in_bytes(), 8 + 1);
        assert!(comp.bytes.iter().all(|&b| b == 0));
    }

    #[test]
    fn flat_composite_struct() {
        let layout = Arc::new(LayoutDescriptor::Struct {
            type_name: "Point".to_string(),
            fields: alloc::vec![
                ("x".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
                ("y".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
            ],
        });
        let mut comp = FlatComposite::new(layout.clone(), I64_BYTES, F64_BYTES);
        assert_eq!(comp.size_in_bytes(), 16);

        let x_off = layout
            .struct_field_offset("x", I64_BYTES, F64_BYTES)
            .unwrap();
        let y_off = layout
            .struct_field_offset("y", I64_BYTES, F64_BYTES)
            .unwrap();
        write_i64(&mut comp.bytes, x_off, 3);
        write_i64(&mut comp.bytes, y_off, 4);
        assert_eq!(read_i64(&comp.bytes, x_off), 3);
        assert_eq!(read_i64(&comp.bytes, y_off), 4);
    }

    #[test]
    fn flat_composite_array() {
        let layout = Arc::new(LayoutDescriptor::Array {
            element: Box::new(LayoutDescriptor::Scalar(ScalarKind::Int)),
            count: 4,
        });
        let mut comp = FlatComposite::new(layout.clone(), I64_BYTES, F64_BYTES);
        assert_eq!(comp.size_in_bytes(), 32);

        for i in 0..4 {
            let off = layout.field_offset(i, I64_BYTES, F64_BYTES).unwrap();
            write_i64(&mut comp.bytes, off, (i as i64) * 10);
        }
        for i in 0..4 {
            let off = layout.field_offset(i, I64_BYTES, F64_BYTES).unwrap();
            assert_eq!(read_i64(&comp.bytes, off), (i as i64) * 10);
        }
    }

    #[test]
    fn flat_composite_mixed_field_types() {
        let layout = Arc::new(LayoutDescriptor::Tuple(alloc::vec![
            LayoutDescriptor::Scalar(ScalarKind::Bool),
            LayoutDescriptor::Scalar(ScalarKind::Int),
            LayoutDescriptor::Scalar(ScalarKind::Byte),
        ]));
        let mut comp = FlatComposite::new(layout.clone(), I64_BYTES, F64_BYTES);
        assert_eq!(comp.size_in_bytes(), 1 + 8 + 1);

        let off_bool = layout.field_offset(0, I64_BYTES, F64_BYTES).unwrap();
        let off_int = layout.field_offset(1, I64_BYTES, F64_BYTES).unwrap();
        let off_byte = layout.field_offset(2, I64_BYTES, F64_BYTES).unwrap();

        write_bool(&mut comp.bytes, off_bool, true);
        write_i64(&mut comp.bytes, off_int, -123);
        write_byte(&mut comp.bytes, off_byte, 0xAB);

        assert!(read_bool(&comp.bytes, off_bool));
        assert_eq!(read_i64(&comp.bytes, off_int), -123);
        assert_eq!(read_byte(&comp.bytes, off_byte), 0xAB);
    }
}
