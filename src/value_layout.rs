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
//! [`crate::value_layout::LayoutDescriptor`] describes the
//! byte-level layout of a composite type. Its
//! [`crate::value_layout::LayoutDescriptor::size_in_bytes`],
//! [`crate::value_layout::LayoutDescriptor::field_offset`],
//! [`crate::value_layout::LayoutDescriptor::field_layout`], and
//! [`crate::value_layout::LayoutDescriptor::struct_field_offset`]
//! methods compute the
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

    /// Stable one-byte tag for wire encoding inside a baked access
    /// operand (see [`crate::bytecode::TupleField`] and
    /// [`crate::wire_format`]). The mapping is fixed and independent of
    /// the `floats` feature so that `Float` keeps tag `5` whether or
    /// not the variant is compiled in. Tag `255` is reserved by the
    /// operand codec as a non-kind sentinel and is never returned here.
    pub fn to_tag(&self) -> u8 {
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

    /// Inverse of [`ScalarKind::to_tag`]. Returns `None` for an
    /// unknown tag, which the decoder treats as a corrupted operand.
    /// Tag `5` (`Float`) decodes only when the `floats` feature is
    /// enabled; without it the tag is unknown because the variant does
    /// not exist.
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
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

/// Tag enum identifying which composite `GenericValue` variant a
/// nested flat-composite field re-wraps to (B28 P2 nested inlining).
///
/// When a composite field is itself a transitively-flat composite, its
/// access operand carries a [`crate::bytecode::TupleField::FlatNested`]
/// (or the struct/enum analogue) recording the byte `offset` and `size`
/// of the child's body within the parent, plus this tag so the access
/// handler re-wraps the extracted byte range as the correct `Value`
/// variant. The mapping is fixed and independent of feature flags so the
/// wire encoding is stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeKind {
    /// Re-wrap as [`crate::bytecode::GenericValue::Tuple`].
    Tuple,
    /// Re-wrap as [`crate::bytecode::GenericValue::Array`].
    Array,
    /// Re-wrap as [`crate::bytecode::GenericValue::Struct`].
    Struct,
    /// Re-wrap as [`crate::bytecode::GenericValue::Enum`].
    Enum,
}

impl CompositeKind {
    /// Stable one-byte tag for wire encoding inside a baked nested
    /// access operand. Values are disjoint from [`ScalarKind::to_tag`]
    /// only by context; the codec selects the table by the operand's
    /// nested-vs-scalar discriminator, so reuse of small integers is
    /// safe.
    pub fn to_tag(&self) -> u8 {
        match self {
            Self::Tuple => 0,
            Self::Array => 1,
            Self::Struct => 2,
            Self::Enum => 3,
        }
    }

    /// Inverse of [`CompositeKind::to_tag`]. Returns `None` for an
    /// unknown tag, which the decoder treats as a corrupted operand.
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Tuple),
            1 => Some(Self::Array),
            2 => Some(Self::Struct),
            3 => Some(Self::Enum),
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
                // Discriminant occupies a full word, matching the
                // runtime flat enum body (`enum_with_widths` writes the
                // discriminant as a `ScalarKind::Int` at offset zero) and
                // the `Enum as Word` cast. The payload is padded to the
                // largest variant so every value of the type shares one
                // fixed size, which is what a nested enum field requires
                // (B28 P2 nested inlining).
                word_bytes + payload_max
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

    /// The flat-eligible scalar kind of this layout, or `None` when it is a
    /// composite or a reference scalar (B28 P2 nested inlining).
    ///
    /// Flat-eligible kinds are the non-float fixed-size scalars plus the
    /// opaque reference: `Unit`, `Bool`, `Byte`, `Int`, `Fixed`, `Opaque`.
    /// `Opaque` is flat as a `word_bytes` index into the VM's ephemeral
    /// opaque registry rather than the `Drop`-bearing `Arc` itself (B28
    /// P3); the VM interns at construction and resolves at access, and
    /// interning deduplicates by pointer identity so byte equality of two
    /// bodies coincides with `Arc::ptr_eq`. `Float` is excluded because
    /// raw-byte comparison would change its equality semantics, and `Text`
    /// is not yet flat. This is the single type-side flat-eligibility
    /// predicate; the compiler and the runtime value path agree with it by
    /// construction.
    pub fn flat_scalar_kind(&self) -> Option<ScalarKind> {
        match self {
            Self::Scalar(k) => match k {
                ScalarKind::Unit
                | ScalarKind::Bool
                | ScalarKind::Byte
                | ScalarKind::Int
                | ScalarKind::Fixed
                | ScalarKind::Opaque
                // Text is flat as a two-word `(data_ptr, len)` reference
                // into the arena string bytes (B28 P3); the epoch is
                // reattached at extraction, not stored in the field.
                | ScalarKind::Text => Some(*k),
                // Float is flat (B28 P3 item 5): it packs by its
                // little-endian bytes and a float-bearing composite is
                // compared field-wise (the compiler's Phase A), so the byte
                // residence does not change its IEEE `+0.0`/`-0.0`/`NaN`
                // equality semantics.
                #[cfg(feature = "floats")]
                ScalarKind::Float => Some(*k),
            },
            _ => None,
        }
    }

    /// The composite kind this layout re-wraps to as a nested flat field,
    /// or `None` for a scalar (B28 P2 nested inlining). Structural only; a
    /// caller pairs it with [`LayoutDescriptor::flat_byte_size`] to confirm
    /// the composite is actually flat-eligible before baking a nested form.
    pub fn flat_composite_kind(&self) -> Option<CompositeKind> {
        match self {
            Self::Tuple(_) => Some(CompositeKind::Tuple),
            Self::Array { .. } => Some(CompositeKind::Array),
            Self::Struct { .. } => Some(CompositeKind::Struct),
            Self::Enum { .. } => Some(CompositeKind::Enum),
            Self::Scalar(_) => None,
        }
    }

    /// Total flat byte size of this layout, or `None` when it is not
    /// transitively flat-eligible (B28 P2 nested inlining).
    ///
    /// This is the single source of the flat layout arithmetic that the
    /// compiler's access baking and enum-padding both consult. A scalar
    /// contributes its size when [`LayoutDescriptor::flat_scalar_kind`] is
    /// `Some`. A tuple, array, or struct is flat when every constituent is
    /// flat; the size is the sum (arrays multiply by the count). An enum is
    /// flat only when it is uniformly flat (every variant's payload flat),
    /// with size `word_bytes + payload_max` to match the runtime body padded
    /// to the largest variant. The built-in `Option` is always boxed (it is
    /// generic and absent from the type tables), so it returns `None`.
    pub fn flat_byte_size(&self, word_bytes: usize, float_bytes: usize) -> Option<usize> {
        match self {
            Self::Scalar(_) => self.flat_scalar_kind().and_then(|k| {
                // A flat `Text` field stores a host data pointer in its
                // first word, so it is flat only when the word slot is at
                // least the host pointer width; a narrow-word build keeps
                // `Text` boxed to avoid truncating the pointer (B28 P3).
                if matches!(k, ScalarKind::Text) && word_bytes < core::mem::size_of::<usize>() {
                    return None;
                }
                Some(k.size_in_bytes(word_bytes, float_bytes))
            }),
            Self::Tuple(elems) => {
                let mut total = 0usize;
                for e in elems {
                    total += e.flat_byte_size(word_bytes, float_bytes)?;
                }
                Some(total)
            }
            Self::Array { element, count } => {
                Some(count * element.flat_byte_size(word_bytes, float_bytes)?)
            }
            Self::Struct { fields, .. } => {
                let mut total = 0usize;
                for (_, t) in fields {
                    total += t.flat_byte_size(word_bytes, float_bytes)?;
                }
                Some(total)
            }
            Self::Enum {
                type_name,
                variants,
            } => {
                if type_name == "Option" {
                    return None;
                }
                let mut payload_max = 0usize;
                for (_, payload) in variants {
                    let mut sum = 0usize;
                    for p in payload {
                        sum += p.flat_byte_size(word_bytes, float_bytes)?;
                    }
                    if sum > payload_max {
                        payload_max = sum;
                    }
                }
                Some(word_bytes + payload_max)
            }
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
        // Discriminant is a full word (B28 P2): 8-byte disc + 8-byte payload.
        assert_eq!(layout.size_in_bytes(I64_BYTES, F64_BYTES), 8 + 8);
    }

    #[test]
    fn enum_with_all_unit_variants_is_one_word() {
        let layout = LayoutDescriptor::Enum {
            type_name: "Color".to_string(),
            variants: vec![
                ("Red".to_string(), vec![]),
                ("Green".to_string(), vec![]),
                ("Blue".to_string(), vec![]),
            ],
        };
        // Word-sized discriminant, empty payload (B28 P2).
        assert_eq!(layout.size_in_bytes(I64_BYTES, F64_BYTES), 8);
    }

    #[test]
    fn enum_with_no_variants_is_one_word() {
        let layout = LayoutDescriptor::Enum {
            type_name: "Never".to_string(),
            variants: vec![],
        };
        // Word-sized discriminant, empty payload (B28 P2).
        assert_eq!(layout.size_in_bytes(I64_BYTES, F64_BYTES), 8);
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

    #[test]
    fn flat_byte_size_scalar_eligibility() {
        assert_eq!(
            LayoutDescriptor::Scalar(ScalarKind::Int).flat_byte_size(I64_BYTES, F64_BYTES),
            Some(8)
        );
        assert_eq!(
            LayoutDescriptor::Scalar(ScalarKind::Bool).flat_byte_size(I64_BYTES, F64_BYTES),
            Some(1)
        );
        // Text is flat-eligible as a two-word `(data_ptr, len)` arena
        // reference (B28 P3), so its flat size is `2 * word_bytes`.
        assert_eq!(
            LayoutDescriptor::Scalar(ScalarKind::Text).flat_byte_size(I64_BYTES, F64_BYTES),
            Some(16)
        );
        // Opaque is flat-eligible as a `word_bytes` registry index (B28 P3).
        assert_eq!(
            LayoutDescriptor::Scalar(ScalarKind::Opaque).flat_byte_size(I64_BYTES, F64_BYTES),
            Some(8)
        );
        // A Float is flat-eligible (B28 P3 item 5): it occupies `float_bytes`
        // and a float-bearing composite is compared field-wise by the
        // compiler, so the flat residence preserves its IEEE equality.
        #[cfg(feature = "floats")]
        assert_eq!(
            LayoutDescriptor::Scalar(ScalarKind::Float).flat_byte_size(I64_BYTES, F64_BYTES),
            Some(8)
        );
    }

    #[test]
    fn flat_byte_size_nested_tuple_in_struct() {
        let inner = LayoutDescriptor::Tuple(vec![
            LayoutDescriptor::Scalar(ScalarKind::Int),
            LayoutDescriptor::Scalar(ScalarKind::Int),
        ]);
        let outer = LayoutDescriptor::Struct {
            type_name: "Holder".to_string(),
            fields: vec![
                ("coords".to_string(), inner),
                ("tag".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
            ],
        };
        assert_eq!(outer.flat_byte_size(I64_BYTES, F64_BYTES), Some(8 + 8 + 8));
    }

    #[test]
    fn flat_byte_size_struct_with_text_field_is_flat() {
        let layout = LayoutDescriptor::Struct {
            type_name: "Greeting".to_string(),
            fields: vec![
                ("id".to_string(), LayoutDescriptor::Scalar(ScalarKind::Int)),
                (
                    "msg".to_string(),
                    LayoutDescriptor::Scalar(ScalarKind::Text),
                ),
            ],
        };
        // Text is flat as a two-word reference (B28 P3): one word for the
        // Int field plus two words for the Text field.
        assert_eq!(layout.flat_byte_size(I64_BYTES, F64_BYTES), Some(8 + 16));
    }

    #[test]
    fn flat_byte_size_uniform_enum_pads_to_word_plus_max() {
        let layout = LayoutDescriptor::Enum {
            type_name: "Sig".to_string(),
            variants: vec![
                ("Off".to_string(), vec![]),
                (
                    "On".to_string(),
                    vec![LayoutDescriptor::Scalar(ScalarKind::Int)],
                ),
                (
                    "Span".to_string(),
                    vec![
                        LayoutDescriptor::Scalar(ScalarKind::Int),
                        LayoutDescriptor::Scalar(ScalarKind::Int),
                    ],
                ),
            ],
        };
        // Word discriminant plus the largest variant payload (two words).
        assert_eq!(layout.flat_byte_size(I64_BYTES, F64_BYTES), Some(8 + 16));
        assert_eq!(layout.flat_composite_kind(), Some(CompositeKind::Enum));
    }

    #[test]
    fn flat_byte_size_option_is_boxed() {
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
        assert_eq!(layout.flat_byte_size(I64_BYTES, F64_BYTES), None);
    }

    #[test]
    fn flat_byte_size_enum_with_text_payload_is_flat() {
        let layout = LayoutDescriptor::Enum {
            type_name: "Reply".to_string(),
            variants: vec![
                (
                    "Ok".to_string(),
                    vec![LayoutDescriptor::Scalar(ScalarKind::Int)],
                ),
                (
                    "Err".to_string(),
                    vec![LayoutDescriptor::Scalar(ScalarKind::Text)],
                ),
            ],
        };
        // Both payloads are flat now (Text is a two-word reference, B28 P3),
        // so the enum is uniformly flat: word discriminant plus the largest
        // payload (the two-word Text in `Err`).
        assert_eq!(layout.flat_byte_size(I64_BYTES, F64_BYTES), Some(8 + 16));
    }

    #[test]
    fn flat_scalar_kind_rejects_references() {
        assert_eq!(
            LayoutDescriptor::Scalar(ScalarKind::Int).flat_scalar_kind(),
            Some(ScalarKind::Int)
        );
        assert_eq!(
            LayoutDescriptor::Scalar(ScalarKind::Text).flat_scalar_kind(),
            Some(ScalarKind::Text)
        );
        assert_eq!(LayoutDescriptor::Tuple(vec![]).flat_scalar_kind(), None);
    }
}
