//! Layout descriptors for composite Keleusma values.
//!
//! This module is parallel infrastructure introduced in B28 P0
//! (see [`docs/decisions/BACKLOG.md`](../../docs/decisions/BACKLOG.md))
//! as the foundation for the V0.2.x runtime composite-Value
//! representation refactor. The Keleusma language admits only
//! fixed-size types in composite positions, and the verifier
//! computes worst-case memory usage bounds assuming fixed sizes.
//! The current runtime stores composites through `Vec<Value>` and
//! `String` indirection, which over-approximates worst-case
//! memory usage and pays heap-allocation overhead the language
//! does not require. B28 corrects the runtime to a flat-byte
//! representation aligned with the language guarantee.
//!
//! [`LayoutDescriptor`] describes the byte-level layout of a
//! composite type. Its [`LayoutDescriptor::size_in_bytes`],
//! [`LayoutDescriptor::field_offset`],
//! [`LayoutDescriptor::field_layout`], and
//! [`LayoutDescriptor::struct_field_offset`] methods compute the
//! layout information that subsequent B28 phases need to read
//! and write composite values through the flat-byte
//! representation.
//!
//! The descriptor stores the structural shape only. Byte sizes
//! depend on the runtime's word and float widths, which are
//! supplied as parameters to the size-related methods rather
//! than baked into the descriptor. This keeps the descriptor
//! independent of the [`crate::word::Word`] and
//! [`crate::float::Float`] type parameters of the parametric
//! virtual machine, matching the target descriptor's
//! cross-architecture portability model (see
//! [`crate::target::Target`]).
//!
//! No runtime path consumes this module yet. P0 is parallel
//! infrastructure; subsequent phases (P1 through P5) migrate the
//! composite runtime representation onto this foundation.

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// Tag enum identifying the fixed-size primitive types Keleusma
/// admits in composite positions.
///
/// The surface-language `Text` type is represented as a fixed-
/// size handle (`Text` variant) regardless of whether the
/// underlying value is a [`crate::bytecode::GenericValue::StaticStr`]
/// (rodata-resident) or [`crate::bytecode::GenericValue::KStr`]
/// (arena-resident). The handle size is `2 * word_bytes` (a
/// pointer-or-offset plus a length or epoch field). The runtime
/// distinguishes the two cases through a discriminant in the
/// handle; the layout pass treats both as the same scalar shape.
///
/// Opaque host references ([`crate::bytecode::GenericValue::Opaque`])
/// are represented as a fixed-size single-pointer handle
/// (`Opaque` variant). The byte size is `word_bytes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarKind {
    /// The unit type `()`. Zero bytes. Carries no information.
    Unit,
    /// Boolean. One byte. Stored as `0u8` for `false` and `1u8`
    /// for `true`.
    Bool,
    /// Eight-bit unsigned integer. One byte. Surface type
    /// `Byte`.
    Byte,
    /// Signed integer of the runtime's word width. Surface type
    /// `Word`. Byte size depends on the parametric `Word` type
    /// (`i8`, `i16`, `i32`, or `i64`).
    Int,
    /// Signed Q-format fixed-point of the runtime's word width.
    /// Byte size is the same as `Int`. The fraction-bit count
    /// is carried by the opcodes that produce or consume the
    /// value, not by the layout descriptor.
    Fixed,
    /// Floating-point of the runtime's float width. Surface
    /// type `Float`. Byte size depends on the parametric
    /// `Float` type (`f32` or `f64`). Gated behind the `floats`
    /// feature alongside the rest of the floating-point runtime
    /// surface.
    #[cfg(feature = "floats")]
    Float,
    /// Text reference. Fixed-size handle that carries either a
    /// rodata offset and length (for static strings) or an arena
    /// handle and epoch (for dynamic strings). Byte size is
    /// `2 * word_bytes`. The runtime distinguishes the two
    /// underlying representations through a discriminant in the
    /// handle; the layout pass treats both uniformly.
    Text,
    /// Opaque host reference. Fixed-size single-pointer handle
    /// to a host-managed `Arc<dyn HostOpaque>`. Byte size is
    /// `word_bytes`.
    Opaque,
}

impl ScalarKind {
    /// Byte size of this scalar under the supplied word and
    /// float widths.
    ///
    /// `word_bytes` is the byte width of the runtime's `Word`
    /// type. `float_bytes` is the byte width of the runtime's
    /// `Float` type. Both are supplied by the caller rather
    /// than baked into the descriptor so the same descriptor
    /// can serve runtimes with different word and float widths.
    pub fn size_in_bytes(&self, word_bytes: usize, float_bytes: usize) -> usize {
        let _ = float_bytes;
        match self {
            Self::Unit => 0,
            Self::Bool => 1,
            Self::Byte => 1,
            Self::Int => word_bytes,
            Self::Fixed => word_bytes,
            #[cfg(feature = "floats")]
            Self::Float => float_bytes,
            Self::Text => 2 * word_bytes,
            Self::Opaque => word_bytes,
        }
    }

    /// Encode this scalar kind as a single byte suitable for the
    /// wire-format operand byte of a `ReadScalarAt`,
    /// `WriteScalarAt`, `ReadDataField`, or `WriteDataField`
    /// opcode. The encoding is stable for the V0.2.x ISA reset.
    pub fn to_u8(&self) -> u8 {
        match self {
            Self::Unit => 0,
            Self::Bool => 1,
            Self::Byte => 2,
            Self::Int => 3,
            Self::Fixed => 4,
            #[cfg(feature = "floats")]
            Self::Float => 5,
            Self::Text => 6,
            Self::Opaque => 7,
        }
    }

    /// Decode a single byte back into a scalar kind. Returns
    /// `None` for any unrecognised tag (including the `Float`
    /// tag when the `floats` feature is disabled).
    pub fn from_u8(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::Unit),
            1 => Some(Self::Bool),
            2 => Some(Self::Byte),
            3 => Some(Self::Int),
            4 => Some(Self::Fixed),
            #[cfg(feature = "floats")]
            5 => Some(Self::Float),
            6 => Some(Self::Text),
            7 => Some(Self::Opaque),
            _ => None,
        }
    }
}

/// Byte-level layout descriptor for a Keleusma composite type.
///
/// The descriptor captures the structural shape of the type
/// (scalar, tuple, array, struct, enum) along with the layout
/// information needed to read and write the type through the
/// flat-byte representation. Field-name to byte-offset
/// resolution and per-field type lookup are exposed through
/// [`LayoutDescriptor::struct_field_offset`] and
/// [`LayoutDescriptor::field_layout`].
///
/// The descriptor stores no width information. Sizes and
/// offsets are computed on demand from the supplied word and
/// float byte widths, which keeps the descriptor independent of
/// the parametric `Word` and `Float` type parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutDescriptor {
    /// A fixed-size primitive type.
    Scalar(ScalarKind),
    /// A heterogeneous tuple of elements.
    Tuple(Vec<LayoutDescriptor>),
    /// A homogeneous array of `count` elements of the same
    /// `element` layout.
    Array {
        /// Layout of each array element.
        element: Box<LayoutDescriptor>,
        /// Number of elements in the array.
        count: usize,
    },
    /// A named struct with ordered fields.
    Struct {
        /// Name of the struct type. Used for diagnostics; the
        /// layout itself depends only on the ordered field
        /// list.
        type_name: String,
        /// Ordered `(field_name, field_layout)` pairs. The
        /// order is the source-declaration order; byte offsets
        /// follow the same order.
        fields: Vec<(String, LayoutDescriptor)>,
    },
    /// A named enum with one or more variants. Layout reserves
    /// one byte for the discriminant followed by enough bytes
    /// for the largest variant's payload.
    Enum {
        /// Name of the enum type. Used for diagnostics.
        type_name: String,
        /// Ordered `(variant_name, payload_layouts)` pairs.
        /// `payload_layouts` is empty for unit variants and
        /// holds one or more entries for tuple-style variants.
        variants: Vec<(String, Vec<LayoutDescriptor>)>,
    },
}

impl LayoutDescriptor {
    /// Total byte size of this composite under the supplied
    /// word and float widths.
    pub fn size_in_bytes(&self, word_bytes: usize, float_bytes: usize) -> usize {
        match self {
            Self::Scalar(kind) => kind.size_in_bytes(word_bytes, float_bytes),
            Self::Tuple(elems) => elems
                .iter()
                .map(|e| e.size_in_bytes(word_bytes, float_bytes))
                .sum(),
            Self::Array { element, count } => {
                element.size_in_bytes(word_bytes, float_bytes) * count
            }
            Self::Struct { fields, .. } => fields
                .iter()
                .map(|(_, t)| t.size_in_bytes(word_bytes, float_bytes))
                .sum(),
            Self::Enum { variants, .. } => {
                let payload_max = variants
                    .iter()
                    .map(|(_, payload)| {
                        payload
                            .iter()
                            .map(|t| t.size_in_bytes(word_bytes, float_bytes))
                            .sum::<usize>()
                    })
                    .max()
                    .unwrap_or(0);
                1 + payload_max
            }
        }
    }

    /// Byte offset of the indexed field within this composite.
    ///
    /// Returns `None` when the layout is not indexable (scalar
    /// or enum) or when `index` is out of bounds. For arrays,
    /// the offset is `index * element_size`. For tuples, the
    /// offset is the sum of preceding element sizes. For
    /// structs, the offset is the sum of preceding field sizes
    /// in declaration order.
    pub fn field_offset(
        &self,
        index: usize,
        word_bytes: usize,
        float_bytes: usize,
    ) -> Option<usize> {
        match self {
            Self::Tuple(elems) => {
                if index >= elems.len() {
                    None
                } else {
                    Some(
                        elems
                            .iter()
                            .take(index)
                            .map(|e| e.size_in_bytes(word_bytes, float_bytes))
                            .sum(),
                    )
                }
            }
            Self::Struct { fields, .. } => {
                if index >= fields.len() {
                    None
                } else {
                    Some(
                        fields
                            .iter()
                            .take(index)
                            .map(|(_, t)| t.size_in_bytes(word_bytes, float_bytes))
                            .sum(),
                    )
                }
            }
            Self::Array { element, count } => {
                if index >= *count {
                    None
                } else {
                    Some(element.size_in_bytes(word_bytes, float_bytes) * index)
                }
            }
            Self::Scalar(_) | Self::Enum { .. } => None,
        }
    }

    /// Layout of the indexed field within this composite.
    ///
    /// Returns `None` when the layout is not indexable (scalar
    /// or enum) or when `index` is out of bounds. For arrays,
    /// returns the element layout regardless of `index` (as long
    /// as `index < count`).
    pub fn field_layout(&self, index: usize) -> Option<&LayoutDescriptor> {
        match self {
            Self::Tuple(elems) => elems.get(index),
            Self::Struct { fields, .. } => fields.get(index).map(|(_, t)| t),
            Self::Array { element, count } => {
                if index >= *count {
                    None
                } else {
                    Some(element)
                }
            }
            Self::Scalar(_) | Self::Enum { .. } => None,
        }
    }

    /// Byte offset of the named struct field.
    ///
    /// Returns `None` when the layout is not a struct or when
    /// no field with the supplied name exists.
    pub fn struct_field_offset(
        &self,
        name: &str,
        word_bytes: usize,
        float_bytes: usize,
    ) -> Option<usize> {
        match self {
            Self::Struct { fields, .. } => {
                let mut offset = 0;
                for (field_name, field_type) in fields {
                    if field_name == name {
                        return Some(offset);
                    }
                    offset += field_type.size_in_bytes(word_bytes, float_bytes);
                }
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    const I64_BYTES: usize = 8;
    const F64_BYTES: usize = 8;
    const I32_BYTES: usize = 4;
    const F32_BYTES: usize = 4;

    #[test]
    fn scalar_unit_is_zero_bytes() {
        assert_eq!(ScalarKind::Unit.size_in_bytes(I64_BYTES, F64_BYTES), 0);
        assert_eq!(ScalarKind::Unit.size_in_bytes(I32_BYTES, F32_BYTES), 0);
    }

    #[test]
    fn scalar_bool_is_one_byte() {
        assert_eq!(ScalarKind::Bool.size_in_bytes(I64_BYTES, F64_BYTES), 1);
        assert_eq!(ScalarKind::Bool.size_in_bytes(I32_BYTES, F32_BYTES), 1);
    }

    #[test]
    fn scalar_byte_is_one_byte() {
        assert_eq!(ScalarKind::Byte.size_in_bytes(I64_BYTES, F64_BYTES), 1);
        assert_eq!(ScalarKind::Byte.size_in_bytes(I32_BYTES, F32_BYTES), 1);
    }

    #[test]
    fn scalar_int_follows_word_width() {
        assert_eq!(ScalarKind::Int.size_in_bytes(I64_BYTES, F64_BYTES), 8);
        assert_eq!(ScalarKind::Int.size_in_bytes(I32_BYTES, F32_BYTES), 4);
        assert_eq!(ScalarKind::Int.size_in_bytes(2, F32_BYTES), 2);
        assert_eq!(ScalarKind::Int.size_in_bytes(1, F32_BYTES), 1);
    }

    #[test]
    fn scalar_fixed_follows_word_width() {
        assert_eq!(ScalarKind::Fixed.size_in_bytes(I64_BYTES, F64_BYTES), 8);
        assert_eq!(ScalarKind::Fixed.size_in_bytes(I32_BYTES, F32_BYTES), 4);
    }

    #[cfg(feature = "floats")]
    #[test]
    fn scalar_float_follows_float_width() {
        assert_eq!(ScalarKind::Float.size_in_bytes(I64_BYTES, F64_BYTES), 8);
        assert_eq!(ScalarKind::Float.size_in_bytes(I32_BYTES, F32_BYTES), 4);
    }

    #[test]
    fn scalar_text_is_two_words() {
        assert_eq!(ScalarKind::Text.size_in_bytes(I64_BYTES, F64_BYTES), 16);
        assert_eq!(ScalarKind::Text.size_in_bytes(I32_BYTES, F32_BYTES), 8);
        assert_eq!(ScalarKind::Text.size_in_bytes(2, F32_BYTES), 4);
        assert_eq!(ScalarKind::Text.size_in_bytes(1, F32_BYTES), 2);
    }

    #[test]
    fn scalar_opaque_is_one_word() {
        assert_eq!(ScalarKind::Opaque.size_in_bytes(I64_BYTES, F64_BYTES), 8);
        assert_eq!(ScalarKind::Opaque.size_in_bytes(I32_BYTES, F32_BYTES), 4);
        assert_eq!(ScalarKind::Opaque.size_in_bytes(2, F32_BYTES), 2);
    }

    #[test]
    fn tuple_size_sums_elements() {
        let layout = LayoutDescriptor::Tuple(vec![
            LayoutDescriptor::Scalar(ScalarKind::Int),
            LayoutDescriptor::Scalar(ScalarKind::Bool),
            LayoutDescriptor::Scalar(ScalarKind::Byte),
        ]);
        assert_eq!(layout.size_in_bytes(I64_BYTES, F64_BYTES), 8 + 1 + 1);
        assert_eq!(layout.size_in_bytes(I32_BYTES, F32_BYTES), 4 + 1 + 1);
    }

    #[test]
    fn empty_tuple_is_zero_bytes() {
        let layout = LayoutDescriptor::Tuple(vec![]);
        assert_eq!(layout.size_in_bytes(I64_BYTES, F64_BYTES), 0);
    }

    #[test]
    fn array_size_multiplies_count() {
        let layout = LayoutDescriptor::Array {
            element: Box::new(LayoutDescriptor::Scalar(ScalarKind::Int)),
            count: 8,
        };
        assert_eq!(layout.size_in_bytes(I64_BYTES, F64_BYTES), 64);
        assert_eq!(layout.size_in_bytes(I32_BYTES, F32_BYTES), 32);
    }

    #[test]
    fn empty_array_is_zero_bytes() {
        let layout = LayoutDescriptor::Array {
            element: Box::new(LayoutDescriptor::Scalar(ScalarKind::Int)),
            count: 0,
        };
        assert_eq!(layout.size_in_bytes(I64_BYTES, F64_BYTES), 0);
    }

    #[test]
    fn nested_tuple_size() {
        let inner = LayoutDescriptor::Tuple(vec![
            LayoutDescriptor::Scalar(ScalarKind::Int),
            LayoutDescriptor::Scalar(ScalarKind::Int),
        ]);
        let outer =
            LayoutDescriptor::Tuple(vec![inner, LayoutDescriptor::Scalar(ScalarKind::Bool)]);
        assert_eq!(outer.size_in_bytes(I64_BYTES, F64_BYTES), 16 + 1);
    }

    #[test]
    fn struct_size_sums_fields() {
        let layout = LayoutDescriptor::Struct {
            type_name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
                ("y".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
            ],
        };
        assert_eq!(layout.size_in_bytes(I64_BYTES, F64_BYTES), 16);
    }

    #[test]
    fn enum_size_is_discriminant_plus_largest_variant() {
        let layout = LayoutDescriptor::Enum {
            type_name: "Option".to_string(),
            variants: vec![
                ("None".to_string(), vec![]),
                (
                    "Some".to_string(),
                    vec![LayoutDescriptor::Scalar(ScalarKind::Int)],
                ),
            ],
        };
        assert_eq!(layout.size_in_bytes(I64_BYTES, F64_BYTES), 1 + 8);
    }

    #[test]
    fn enum_with_all_unit_variants_is_one_byte() {
        let layout = LayoutDescriptor::Enum {
            type_name: "Color".to_string(),
            variants: vec![
                ("Red".to_string(), vec![]),
                ("Green".to_string(), vec![]),
                ("Blue".to_string(), vec![]),
            ],
        };
        assert_eq!(layout.size_in_bytes(I64_BYTES, F64_BYTES), 1);
    }

    #[test]
    fn enum_with_no_variants_is_one_byte() {
        let layout = LayoutDescriptor::Enum {
            type_name: "Never".to_string(),
            variants: vec![],
        };
        assert_eq!(layout.size_in_bytes(I64_BYTES, F64_BYTES), 1);
    }

    #[test]
    fn tuple_field_offset() {
        let layout = LayoutDescriptor::Tuple(vec![
            LayoutDescriptor::Scalar(ScalarKind::Int),
            LayoutDescriptor::Scalar(ScalarKind::Bool),
            LayoutDescriptor::Scalar(ScalarKind::Int),
        ]);
        assert_eq!(layout.field_offset(0, I64_BYTES, F64_BYTES), Some(0));
        assert_eq!(layout.field_offset(1, I64_BYTES, F64_BYTES), Some(8));
        assert_eq!(layout.field_offset(2, I64_BYTES, F64_BYTES), Some(9));
        assert_eq!(layout.field_offset(3, I64_BYTES, F64_BYTES), None);
    }

    #[test]
    fn struct_field_offset_by_index() {
        let layout = LayoutDescriptor::Struct {
            type_name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
                ("y".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
                ("z".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
            ],
        };
        assert_eq!(layout.field_offset(0, I64_BYTES, F64_BYTES), Some(0));
        assert_eq!(layout.field_offset(1, I64_BYTES, F64_BYTES), Some(8));
        assert_eq!(layout.field_offset(2, I64_BYTES, F64_BYTES), Some(16));
        assert_eq!(layout.field_offset(3, I64_BYTES, F64_BYTES), None);
    }

    #[test]
    fn array_field_offset() {
        let layout = LayoutDescriptor::Array {
            element: Box::new(LayoutDescriptor::Scalar(ScalarKind::Int)),
            count: 4,
        };
        assert_eq!(layout.field_offset(0, I64_BYTES, F64_BYTES), Some(0));
        assert_eq!(layout.field_offset(1, I64_BYTES, F64_BYTES), Some(8));
        assert_eq!(layout.field_offset(3, I64_BYTES, F64_BYTES), Some(24));
        assert_eq!(layout.field_offset(4, I64_BYTES, F64_BYTES), None);
    }

    #[test]
    fn struct_field_offset_by_name() {
        let layout = LayoutDescriptor::Struct {
            type_name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
                ("y".to_string(), LayoutDescriptor::Scalar(ScalarKind::Bool)),
                ("z".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
            ],
        };
        assert_eq!(
            layout.struct_field_offset("x", I64_BYTES, F64_BYTES),
            Some(0)
        );
        assert_eq!(
            layout.struct_field_offset("y", I64_BYTES, F64_BYTES),
            Some(8)
        );
        assert_eq!(
            layout.struct_field_offset("z", I64_BYTES, F64_BYTES),
            Some(9)
        );
        assert_eq!(
            layout.struct_field_offset("missing", I64_BYTES, F64_BYTES),
            None
        );
    }

    #[test]
    fn field_layout_returns_element_types() {
        let layout = LayoutDescriptor::Tuple(vec![
            LayoutDescriptor::Scalar(ScalarKind::Int),
            LayoutDescriptor::Scalar(ScalarKind::Bool),
        ]);
        assert_eq!(
            layout.field_layout(0),
            Some(&LayoutDescriptor::Scalar(ScalarKind::Int))
        );
        assert_eq!(
            layout.field_layout(1),
            Some(&LayoutDescriptor::Scalar(ScalarKind::Bool))
        );
        assert_eq!(layout.field_layout(2), None);
    }

    #[test]
    fn scalar_layout_has_no_fields() {
        let layout = LayoutDescriptor::Scalar(ScalarKind::Int);
        assert_eq!(layout.field_offset(0, I64_BYTES, F64_BYTES), None);
        assert_eq!(layout.field_layout(0), None);
        assert_eq!(layout.struct_field_offset("x", I64_BYTES, F64_BYTES), None);
    }

    #[test]
    fn struct_field_offset_under_narrow_word() {
        let layout = LayoutDescriptor::Struct {
            type_name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
                ("y".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
            ],
        };
        assert_eq!(layout.struct_field_offset("x", 2, F64_BYTES), Some(0));
        assert_eq!(layout.struct_field_offset("y", 2, F64_BYTES), Some(2));
    }
}
