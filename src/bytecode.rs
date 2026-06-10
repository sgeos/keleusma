// rkyv's `Archive` derive generates sibling `Archived{Name}` and
// `{Name}Resolver` types adjacent to each derived item, along with
// an `impl Archive` whose associated types and `resolve` method are
// part of the public surface. The generated items inherit the
// parent's `pub` visibility but do not pick up the parent's doc
// comments, and rkyv 0.8's `attr(...)` forwarding does not cover
// the resolver type or the impl-block methods. Allow missing docs
// at the module level for these generated items; the source types
// they mirror carry the authoritative documentation, which is what
// a reader cares about.
#![allow(missing_docs)]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use rkyv::{Archive, Deserialize, Serialize};

use crate::kstring::KString;

/// A compile-time constant, the variant of [`Value`] that the compiler
/// emits into the bytecode's constant pool.
///
/// Strict subset of [`Value`]. Only variants that the rkyv archive can
/// faithfully serialize and deserialize. The runtime-only variant
/// [`Value::KStr`] is intentionally absent because it is produced
/// exclusively by native functions and runtime string operations,
/// never as a compile-time constant.
///
/// The runtime executes against the archived form
/// [`ArchivedConstValue`]. Each operand-stack push from a constant
/// goes through [`Value::from_const_archived`], which lifts the
/// archived form into a runtime `Value`.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, <__C as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source)),
    attr(allow(missing_docs))
)]
pub enum ConstValue {
    /// Unit value `()`.
    Unit,
    /// Boolean.
    Bool(bool),
    /// 64-bit signed integer.
    Int(i64),
    /// Eight-bit unsigned integer. Surface type is `Byte`.
    Byte(u8),
    /// Signed Q-format fixed-point. The wrapped `i64` holds the
    /// fixed-point bits; the fraction-bit count is target-scaled
    /// and is carried by the opcodes that consume the value
    /// rather than stored alongside.
    Fixed(i64),
    /// 64-bit floating-point number. Gated behind the `floats`
    /// cargo feature so flash-constrained targets that do not use
    /// floating-point arithmetic can compile the variant out.
    #[cfg(feature = "floats")]
    Float(f64),
    /// Immutable static string referenced from the rodata region.
    /// Source-level string literals compile to this variant.
    StaticStr(String),
    /// Tuple of constant values.
    Tuple(#[rkyv(omit_bounds)] Vec<ConstValue>),
    /// Fixed-size array of constant values.
    Array(#[rkyv(omit_bounds)] Vec<ConstValue>),
    /// Named struct with ordered fields.
    Struct {
        /// Name of the struct type.
        type_name: String,
        /// Ordered (field-name, field-value) pairs.
        #[rkyv(omit_bounds)]
        fields: Vec<(String, ConstValue)>,
    },
    /// Enum variant with optional payload.
    Enum {
        /// Name of the enum type.
        type_name: String,
        /// Name of the variant.
        variant: String,
        /// Variant discriminant, when the compiler resolved it (B28 P2).
        /// `Some` lets the value materialise into a flat enum body;
        /// `None` (e.g. folded from a boxed runtime value) materialises
        /// boxed.
        discriminant: Option<i64>,
        /// Positional payload values for tuple-variant constructions.
        /// Empty for unit variants.
        #[rkyv(omit_bounds)]
        fields: Vec<ConstValue>,
    },
    /// Option::None.
    None,
}

/// Runtime value in the Keleusma VM.
///
/// Superset of [`ConstValue`] that adds the runtime-only string
/// variant [`Value::KStr`] for arena-allocated strings with
/// epoch-tagged stale-pointer detection. KStr does not participate
/// in rkyv serialization. The constant-pool boundary is the
/// [`Value::from_const_archived`] lift and the
/// `ConstValue::try_from(&Value)` lower direction is intentionally
/// absent because runtime values cannot become compile-time
/// constants.
/// Type alias for the default 64-bit `GenericValue` shape.
/// Existing call sites continue to write `Value` (no angle
/// brackets); the alias expands to `GenericValue<i64, f64>` so
/// pattern matching, construction, and trait impls all resolve
/// to the concrete 64-bit specialization.
///
/// Sub-64-bit runtimes constructed via `Vm<W, A, F>` use a
/// different specialization (e.g. `GenericValue<i16, f32>`).
/// Hosts that ship narrow runtimes are encouraged to introduce a
/// local type alias for ergonomic call sites; see the
/// "Parametric VM" recipe in the Cookbook.
///
/// `Address` is intentionally not a `GenericValue` parameter
/// because no runtime-value variant carries an address payload;
/// addresses appear as opcode immediate operands and on the
/// `Vm` itself, not on the values flowing through the operand
/// stack.
pub type Value = GenericValue<i64, f64>;

/// Parametric runtime-value type. The bundled `Vm` uses
/// `GenericValue<i64, f64>` aliased as `Value`; sub-64-bit
/// runtimes use a different specialization. The `W: Word` and
/// `F: Float` constraints match the bytecode header's
/// `word_bits_log2` and `float_bits_log2` declared widths.
/// The body of a `Tuple` value during the B28 P2 migration.
///
/// A transitively-scalar tuple is `Flat`, a pure byte buffer with the
/// fields packed at compiler-baked offsets. A tuple containing a
/// reference field (`Text`, `Opaque`) or a not-yet-migrated nested
/// composite is `Boxed`, the pre-B28 `Vec` representation, which P3
/// removes. Construction chooses the form; the access handler dispatches
/// on it.
#[derive(Debug, Clone, PartialEq)]
pub enum TupleBody<W: crate::word::Word, F: crate::float::Float> {
    /// Flat bytes; fields read at compiler-baked offsets.
    Flat(crate::flat_value::FlatComposite),
    /// Boxed elements (pre-B28 representation; removed in P3).
    Boxed(alloc::vec::Vec<GenericValue<W, F>>),
}

impl<W: crate::word::Word, F: crate::float::Float> TupleBody<W, F> {
    /// The boxed elements. Panics on the `Flat` form, which the VM
    /// `NewTuple` handler does not yet construct in B28 P2; once it does,
    /// the callers of this helper move to reading flat bytes at baked
    /// offsets instead.
    pub fn elements(&self) -> &[GenericValue<W, F>] {
        match self {
            Self::Boxed(v) => v,
            Self::Flat(_) => unreachable!("flat tuple body is not constructed yet (B28 P2)"),
        }
    }

    /// The boxed elements by value. Panics on the `Flat` form, not yet
    /// constructed in B28 P2.
    pub fn into_elements(self) -> alloc::vec::Vec<GenericValue<W, F>> {
        match self {
            Self::Boxed(v) => v,
            Self::Flat(_) => unreachable!("flat tuple body is not constructed yet (B28 P2)"),
        }
    }
}

/// The byte body of an array value (B28 P2). An array is homogeneous, so
/// its flat body is `count * element_size` packed little-endian bytes with
/// no per-element offset table; the element kind is carried by the
/// [`ArrayElem`] operand the compiler bakes into [`Op::GetIndex`], and the
/// element size follows from that kind at the module-declared scalar
/// widths. A transitively-scalar array is `Flat`; an array whose element
/// type is a reference, float, or composite stays `Boxed`, the pre-B28
/// `Vec` form that P3 removes.
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayBody<W: crate::word::Word, F: crate::float::Float> {
    /// Flat bytes; elements read at `index * element_size`.
    Flat(crate::flat_value::FlatComposite),
    /// Boxed elements (pre-B28 representation; removed in P3).
    Boxed(alloc::vec::Vec<GenericValue<W, F>>),
}

impl<W: crate::word::Word, F: crate::float::Float> ArrayBody<W, F> {
    /// The boxed elements. Panics on the `Flat` form, which carries no
    /// element kind; flat-array reads go through [`Op::GetIndex`] with the
    /// baked [`ArrayElem`] kind, or through the host marshalling boundary
    /// which supplies the element type, never through this accessor.
    pub fn elements(&self) -> &[GenericValue<W, F>] {
        match self {
            Self::Boxed(v) => v,
            Self::Flat(_) => {
                unreachable!(
                    "flat array body has no element kind; read via GetIndex or marshalling"
                )
            }
        }
    }

    /// The boxed elements by value. Panics on the `Flat` form, as
    /// [`Self::elements`].
    pub fn into_elements(self) -> alloc::vec::Vec<GenericValue<W, F>> {
        match self {
            Self::Boxed(v) => v,
            Self::Flat(_) => {
                unreachable!(
                    "flat array body has no element kind; read via GetIndex or marshalling"
                )
            }
        }
    }
}

/// The byte body of a struct value (B28 P2). A struct is a named record;
/// its flat body packs the fields in declaration order with no type name
/// or field-name keys (those are compile-time information baked into the
/// access ops and the type test). A struct with a reference, float, or
/// nested-composite field stays `Boxed`, the pre-B28 representation that
/// carries the type name and the ordered (name, value) pairs, which P3
/// removes.
#[derive(Debug, Clone, PartialEq)]
pub enum StructBody<W: crate::word::Word, F: crate::float::Float> {
    /// Flat bytes; fields read at compiler-baked offsets.
    Flat(crate::flat_value::FlatComposite),
    /// Boxed named fields (pre-B28 representation; removed in P3). The
    /// payload is heap-boxed so this variant costs one pointer rather than
    /// a `String` plus a `Vec`; a boxed struct is comparatively rare on the
    /// operand stack, and keeping the variant small keeps every
    /// `GenericValue` slot small, which directly shrinks the pre-sized
    /// operand-stack arena footprint (B28 P3 item 5).
    Boxed(alloc::boxed::Box<BoxedStruct<W, F>>),
}

/// Heap payload of a non-flat struct value. Boxed inside
/// [`StructBody::Boxed`] to keep `GenericValue` small. Transitional
/// representation removed when boxed bodies relocate into the arena
/// (B28 P3 item 5 C4).
#[derive(Debug, Clone, PartialEq)]
pub struct BoxedStruct<W: crate::word::Word, F: crate::float::Float> {
    /// Name of the struct type.
    pub type_name: alloc::string::String,
    /// Ordered (field-name, field-value) pairs.
    pub fields: alloc::vec::Vec<(alloc::string::String, GenericValue<W, F>)>,
}

impl<W: crate::word::Word, F: crate::float::Float> StructBody<W, F> {
    /// Build a [`StructBody::Boxed`] from its parts, boxing the payload.
    pub fn boxed(
        type_name: alloc::string::String,
        fields: alloc::vec::Vec<(alloc::string::String, GenericValue<W, F>)>,
    ) -> Self {
        Self::Boxed(alloc::boxed::Box::new(BoxedStruct { type_name, fields }))
    }
}

/// The byte body of an enum value (B28 P2). The flat body is the variant's
/// `Word`-sized discriminant followed by the current variant's payload
/// packed in declaration order: `[disc: word_bytes][payload]`. The
/// discriminant matches the `Enum as Word` cast and is what the variant
/// test (`Op::IsEnum`) reads, since an enum is a sum type whose variant is
/// not statically known. The body is sized to the *current* variant (enums
/// are not yet inlined into other flat composites, so a per-value size is
/// sufficient); the worst-case-memory bound is still the largest variant.
/// An enum with a reference, float, or nested-composite payload stays
/// `Boxed`, the pre-B28 representation carrying the type and variant names
/// and the payload values, which P3 removes.
#[derive(Debug, Clone, PartialEq)]
pub enum EnumBody<W: crate::word::Word, F: crate::float::Float> {
    /// Flat bytes: `[discriminant: word_bytes][payload]`.
    Flat(crate::flat_value::FlatComposite),
    /// Boxed variant (pre-B28 representation; removed in P3). The payload
    /// is heap-boxed so this variant costs one pointer rather than two
    /// `String`s plus a `Vec` (the 72-byte form that previously made every
    /// `GenericValue` slot 72 bytes); keeping it small shrinks the pre-sized
    /// operand-stack arena footprint (B28 P3 item 5).
    Boxed(alloc::boxed::Box<BoxedEnum<W, F>>),
}

/// Heap payload of a non-flat enum value. Boxed inside [`EnumBody::Boxed`]
/// to keep `GenericValue` small. Transitional representation removed when
/// boxed bodies relocate into the arena (B28 P3 item 5 C4).
#[derive(Debug, Clone, PartialEq)]
pub struct BoxedEnum<W: crate::word::Word, F: crate::float::Float> {
    /// Name of the enum type.
    pub type_name: alloc::string::String,
    /// Name of the variant.
    pub variant: alloc::string::String,
    /// Positional payload values; empty for a unit variant.
    pub fields: alloc::vec::Vec<GenericValue<W, F>>,
}

impl<W: crate::word::Word, F: crate::float::Float> EnumBody<W, F> {
    /// Build an [`EnumBody::Boxed`] from its parts, boxing the payload.
    pub fn boxed(
        type_name: alloc::string::String,
        variant: alloc::string::String,
        fields: alloc::vec::Vec<GenericValue<W, F>>,
    ) -> Self {
        Self::Boxed(alloc::boxed::Box::new(BoxedEnum {
            type_name,
            variant,
            fields,
        }))
    }
}

#[derive(Debug, Clone)]
pub enum GenericValue<W: crate::word::Word, F: crate::float::Float> {
    /// Unit value `()`.
    Unit,
    /// Boolean.
    Bool(bool),
    /// Script-visible signed integer. Surface type is `Word`.
    /// The bit width is determined by the `W` parameter and
    /// matches the bytecode header's `word_bits_log2`.
    Int(W),
    /// Eight-bit unsigned integer. Surface type is `Byte`. Arithmetic
    /// uses wrapping `u8` semantics; conversions to and from `Word`
    /// go through `Op::WordToByte` and `Op::ByteToWord`.
    Byte(u8),
    /// Signed Q-format fixed-point. The wrapped `W` holds the
    /// fixed-point bits; the fraction-bit count is carried by the
    /// opcodes that produce or consume the value.
    Fixed(W),
    /// Script-visible floating-point number. The width is
    /// determined by the `F` parameter and matches the bytecode
    /// header's `float_bits_log2`. Gated behind the `floats`
    /// cargo feature alongside the rest of the floating-point
    /// runtime surface.
    #[cfg(feature = "floats")]
    Float(F),
    /// Immutable static string referenced from the rodata region. Source-level
    /// string literals compile to this variant. Permitted to flow through the
    /// dialogue type B and across hot updates subject to the host attestation
    /// for rodata pointer validity. See R31, R32, R33 and B5.
    StaticStr(String),
    /// Dynamic string allocated in the host-owned arena's top region.
    /// Carries a [`crate::kstring::KString`] handle that becomes
    /// [`keleusma_arena::Stale`] on access if the arena has been reset
    /// since the handle was issued. Subject to the cross-yield
    /// prohibition because the underlying storage does not survive a
    /// reset. The boundary type for native callers and the host that
    /// want bounded-memory accounting and stale-pointer detection.
    KStr(KString),
    /// Tuple of values. The body is flat bytes for a transitively-scalar
    /// tuple or boxed elements otherwise (B28 P2); see [`TupleBody`].
    Tuple(TupleBody<W, F>),
    /// Fixed-size array of values. The body is flat bytes for a
    /// transitively-scalar element type or boxed elements otherwise
    /// (B28 P2); see [`ArrayBody`].
    Array(ArrayBody<W, F>),
    /// Named struct. The body is flat bytes for a transitively-scalar
    /// field list or boxed named fields otherwise (B28 P2); see
    /// [`StructBody`].
    Struct(StructBody<W, F>),
    /// Enum variant with optional payload. The body is flat bytes for a
    /// transitively-scalar payload or boxed otherwise (B28 P2); see
    /// [`EnumBody`].
    Enum(EnumBody<W, F>),
    /// Option::None.
    None,
    /// Opaque host-managed value referenced through a shared
    /// reference-counted pointer. Produced by host-registered native
    /// functions that operate on Rust types the script does not
    /// introspect. The pointee implements the
    /// [`crate::opaque::HostOpaque`] marker trait; the script-side
    /// type is the opaque name registered through the type checker.
    ///
    /// Lifetime is independent of the arena: opaque values may
    /// cross the yield boundary in the dialogue type, persist across
    /// arena resets, and survive hot code swaps. Equality is by
    /// pointer identity, matching the convention for host-managed
    /// references.
    ///
    /// WCMU contribution is zero from the script side because the
    /// allocation is host-managed. Hosts that want to bound their
    /// own opaque heap supply a per-native attestation through
    /// [`crate::vm::Vm::set_native_bounds`].
    Opaque(alloc::sync::Arc<dyn crate::opaque::HostOpaque>),

    /// Phantom variant kept only when the `floats` feature is
    /// disabled, so the `F` type parameter is referenced non-
    /// recursively. Never constructed at runtime; pattern
    /// matches over `GenericValue` use a wildcard arm to absorb
    /// this case under either feature combination.
    #[cfg(not(feature = "floats"))]
    #[doc(hidden)]
    _PhantomFloat(core::marker::PhantomData<F>),
}

impl<W: crate::word::Word, F: crate::float::Float> PartialEq for GenericValue<W, F> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unit, Self::Unit) | (Self::None, Self::None) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Byte(a), Self::Byte(b)) => a == b,
            (Self::Fixed(a), Self::Fixed(b)) => a == b,
            #[cfg(feature = "floats")]
            (Self::Float(a), Self::Float(b)) => a == b,
            // Static strings compare equal if their contents match.
            (Self::StaticStr(a), Self::StaticStr(b)) => a == b,
            // KStr equality compares the captured handle (pointer and
            // epoch). Two KStr handles are equal only if they point to
            // the same arena allocation under the same epoch. Content
            // equality across distinct arena allocations is not checked
            // because the comparison would require an arena borrow that
            // `PartialEq` does not provide. Hosts that want content
            // equality must compare through `as_str_with_arena` against
            // a known arena.
            (Self::KStr(a), Self::KStr(b)) => a.epoch() == b.epoch(),
            (Self::Tuple(a), Self::Tuple(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Struct(a), Self::Struct(b)) => a == b,
            // Flat enum bodies compare with padding tolerance (B28 P2):
            // a compiler-padded body (`word + payload_max`) and an
            // unpadded variant-sized body of the same value differ only in
            // trailing zero padding. Comparing the overlapping prefix and
            // requiring each remainder to be zero makes them equal without
            // a type table. The discriminant word lies in the prefix (both
            // bodies are at least `word_bytes` long) and is unique per
            // variant, so distinct variants never alias. Boxed and mixed
            // pairs keep the derived comparison.
            (Self::Enum(EnumBody::Flat(a)), Self::Enum(EnumBody::Flat(b))) => {
                match (a.inline_bytes(), b.inline_bytes()) {
                    (Some(x), Some(y)) => flat_enum_bytes_eq(x, y),
                    // An arena enum body needs the arena to read, which
                    // `PartialEq` lacks; the VM materialises composites to
                    // inline before `CmpEq`, so this arm never sees an arena
                    // body in practice (B28 P2).
                    _ => false,
                }
            }
            (Self::Enum(a), Self::Enum(b)) => a == b,
            // Opaque equality is pointer identity. Two Arcs are
            // equal only if they share the same allocation. This
            // matches the convention for host-managed references
            // and avoids requiring `Eq` on the host's opaque type.
            (Self::Opaque(a), Self::Opaque(b)) => alloc::sync::Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// The flat-composite scalar kind of a value, or `None` when the value
/// is not a flat-eligible tuple field (B28 P2).
///
/// Eligible kinds are the non-reference, non-float fixed-size scalars.
/// `Float` is excluded because the flat body compares by raw bytes,
/// which would change the `+0.0`/`-0.0` and `NaN` semantics of tuple
/// equality. References, `None`, and composites are not flat-eligible.
/// The compiler's `type_flat_scalar_kind` mirrors this on the type
/// side so construction and baked access agree.
pub(crate) fn flat_tuple_scalar_kind<W: crate::word::Word, F: crate::float::Float>(
    v: &GenericValue<W, F>,
) -> Option<crate::value_layout::ScalarKind> {
    use crate::value_layout::ScalarKind as K;
    match v {
        GenericValue::Unit => Some(K::Unit),
        GenericValue::Bool(_) => Some(K::Bool),
        GenericValue::Byte(_) => Some(K::Byte),
        GenericValue::Int(_) => Some(K::Int),
        GenericValue::Fixed(_) => Some(K::Fixed),
        // A `Float` is flat (B28 P3 item 5): it packs by its little-endian
        // bytes (handled by `write_scalar_le`/`read_scalar_le`) and a
        // float-bearing composite is compared field-wise by the compiler, so
        // the byte residence does not change its IEEE equality semantics.
        #[cfg(feature = "floats")]
        GenericValue::Float(_) => Some(K::Float),
        // A `KStr` is flat as a two-word `(data_ptr, len)` arena reference
        // (B28 P3). `StaticStr` is heap-owned and not flat here; the VM
        // construct path copies it into the arena, converting it to a
        // `KStr`, before packing. `Opaque` stays non-flat (interned to a
        // one-word registry index by the VM, not packed from here).
        GenericValue::KStr(_) => Some(K::Text),
        _ => None,
    }
}

/// Byte length a value contributes as a field of a flat composite, or
/// `None` if it is not flat-eligible (B28 P2 nested inlining).
///
/// A flat-eligible scalar contributes its scalar size. A composite that
/// is itself in its `Flat` byte body contributes that body's byte
/// length, so it can be inlined into the parent's body. A boxed
/// composite or a reference-bearing value is not flat-eligible and forces
/// the parent boxed, exactly as `flat_tuple_scalar_kind` already does for
/// scalars.
pub(crate) fn flat_field_size<W: crate::word::Word, F: crate::float::Float>(
    v: &GenericValue<W, F>,
    word_bytes: usize,
    float_bytes: usize,
) -> Option<usize> {
    if let Some(kind) = flat_tuple_scalar_kind(v) {
        return Some(kind.size_in_bytes(word_bytes, float_bytes));
    }
    flat_body_bytes(v).map(|b| b.len())
}

/// Whether `v` is flat-eligible as a tuple or array element including the
/// opaque reference kind (B28 P3 item 3), the value-side mirror of the
/// compiler's `classify_flat_field` for tuples and arrays. A tuple or array
/// is built flat only when every element satisfies this. `Opaque` is
/// eligible (the VM interns it to a one-word registry index). Text is not
/// eligible here: flattening a tuple's text would hide its `KStr` from the
/// `materialise_kstrings`/`contains_dynstr` lifecycle and remove the ability
/// to yield a static-text tuple, so a text-bearing tuple stays boxed (its
/// arena residence is the concern of the boxed-body arena migration). Every
/// other case reduces to `flat_field_size` (a flat scalar or
/// transitively-flat nested composite; a float, a string, or a boxed
/// composite is not eligible). The VM interns an `Opaque` before packing, so
/// this predicate takes the pre-interning value.
pub(crate) fn flat_tuple_element_with_refs<W: crate::word::Word, F: crate::float::Float>(
    v: &GenericValue<W, F>,
    word_bytes: usize,
    float_bytes: usize,
) -> bool {
    match v {
        GenericValue::Opaque(_) => true,
        _ => flat_field_size(v, word_bytes, float_bytes).is_some(),
    }
}

/// Padding-tolerant equality of two flat enum bodies (B28 P2).
///
/// Compares the overlapping prefix and requires each trailing remainder
/// to be all-zero. This makes a compiler-padded body (`word + payload_max`)
/// equal to an unpadded variant-sized body of the same value, because the
/// only difference is deterministic zero padding. The discriminant word
/// lies within the prefix (both bodies are at least `word_bytes` long) and
/// is unique per variant, so two distinct variants always differ in the
/// prefix and never alias under this rule.
fn flat_enum_bytes_eq(a: &[u8], b: &[u8]) -> bool {
    let m = core::cmp::min(a.len(), b.len());
    a[..m] == b[..m] && a[m..].iter().all(|&x| x == 0) && b[m..].iter().all(|&x| x == 0)
}

/// The flat byte body of a composite value in its `Flat` representation,
/// or `None` for a boxed composite or a non-composite (B28 P2 nested
/// inlining). Callers inline these bytes into a parent composite body.
pub(crate) fn flat_body_bytes<W: crate::word::Word, F: crate::float::Float>(
    v: &GenericValue<W, F>,
) -> Option<&[u8]> {
    match v {
        GenericValue::Tuple(TupleBody::Flat(fc))
        | GenericValue::Array(ArrayBody::Flat(fc))
        | GenericValue::Struct(StructBody::Flat(fc))
        | GenericValue::Enum(EnumBody::Flat(fc)) => Some(fc.as_bytes()),
        _ => None,
    }
}

impl<W: crate::word::Word, F: crate::float::Float> GenericValue<W, F> {
    /// Construct a tuple value, choosing the flat byte body for a
    /// transitively-scalar tuple and the boxed body otherwise (B28 P2).
    ///
    /// This is the common constructor used by hosts, tests, and the
    /// runtime. It delegates to [`GenericValue::tuple_with_widths`] at
    /// the runtime's own scalar widths (from [`crate::word::Word::BITS_LOG2`]
    /// and [`crate::float::Float::BITS_LOG2`]), which equal the
    /// module-declared widths on the bundled runtime. Routing every
    /// construction through the same flat-or-boxed decision is what lets
    /// a given tuple type have one representation, which tuple equality
    /// and flat access both rely on. A reference-bearing or float-
    /// bearing tuple is not flat-eligible and stays boxed.
    pub fn tuple(elements: alloc::vec::Vec<Self>) -> Self {
        let word_bytes = (1usize << <W as crate::word::Word>::BITS_LOG2) / 8;
        let float_bytes = (1usize << <F as crate::float::Float>::BITS_LOG2) / 8;
        Self::tuple_with_widths(elements, word_bytes, float_bytes)
    }

    /// Construct a tuple value, choosing the flat byte body for a
    /// transitively-scalar tuple and the boxed body otherwise, using
    /// the given scalar widths (B28 P2).
    ///
    /// This is the single choke point for tuple construction so every
    /// path (the VM `NewTuple` handler, constant materialisation, and
    /// host marshalling) agrees on the representation for a given type.
    /// A flat body is produced only when every element is a
    /// flat-eligible scalar (see `flat_tuple_scalar_kind`) and the
    /// packed size fits the sixteen-bit access offset; the fields are
    /// written little-endian at packed offsets using `word_bytes` and
    /// `float_bytes`, the same widths the compiler bakes access offsets
    /// against.
    pub fn tuple_with_widths(
        elements: alloc::vec::Vec<Self>,
        word_bytes: usize,
        float_bytes: usize,
    ) -> Self {
        // A text element keeps the tuple boxed (B28 P3): flattening it would
        // hide the `KStr` from the `materialise_kstrings`/`contains_dynstr`
        // lifecycle and remove the ability to yield a static-text tuple.
        // Opaque is flat (B28 P3 item 3): the VM interns it to a one-word
        // index before calling here, so an interned element is an `Int` and
        // packs flat; a host- or constant-built tuple that still carries an
        // `Opaque` cannot intern it and stays boxed via `try_pack_flat`
        // returning `None`, the same host/VM gap struct and enum have.
        if elements
            .iter()
            .any(|e| matches!(e, Self::StaticStr(_) | Self::KStr(_)))
        {
            return Self::Tuple(TupleBody::Boxed(elements));
        }
        // A tuple has no padding (no minimum), so the packed size is exact.
        match Self::try_pack_flat(elements.iter(), 0, word_bytes, float_bytes) {
            Some(body) => Self::Tuple(TupleBody::Flat(body)),
            None => Self::Tuple(TupleBody::Boxed(elements)),
        }
    }

    /// Construct an array value at the runtime's own scalar widths,
    /// choosing the flat byte body for a transitively-scalar element type
    /// and the boxed body otherwise (B28 P2). The array analogue of
    /// [`GenericValue::tuple`].
    pub fn array(elements: alloc::vec::Vec<Self>) -> Self {
        let word_bytes = (1usize << <W as crate::word::Word>::BITS_LOG2) / 8;
        let float_bytes = (1usize << <F as crate::float::Float>::BITS_LOG2) / 8;
        Self::array_with_widths(elements, word_bytes, float_bytes)
    }

    /// Construct an array value, choosing the flat byte body for a
    /// transitively-scalar element type and the boxed body otherwise,
    /// using the given scalar widths (B28 P2).
    ///
    /// This is the single choke point for array construction so the VM
    /// `NewArray` handler, constant materialisation, and host marshalling
    /// all agree on the representation an array type uses, which equality
    /// relies on. The eligibility rule is the same as for a tuple field
    /// (`flat_tuple_scalar_kind`): a flat body is produced only when
    /// every element is a flat-eligible scalar and the packed size fits
    /// the sixteen-bit access offset. Because the array is homogeneous the
    /// elements share one kind, so the packed layout is `count * size`.
    pub fn array_with_widths(
        elements: alloc::vec::Vec<Self>,
        word_bytes: usize,
        float_bytes: usize,
    ) -> Self {
        // A text element keeps the array boxed (B28 P3); opaque is flat (B28
        // P3 item 3); see `tuple_with_widths`.
        if elements
            .iter()
            .any(|e| matches!(e, Self::StaticStr(_) | Self::KStr(_)))
        {
            return Self::Array(ArrayBody::Boxed(elements));
        }
        match Self::try_pack_flat(elements.iter(), 0, word_bytes, float_bytes) {
            Some(body) => Self::Array(ArrayBody::Flat(body)),
            None => Self::Array(ArrayBody::Boxed(elements)),
        }
    }

    /// Construct a struct value at the runtime's own scalar widths,
    /// choosing the flat byte body for a transitively-scalar field list and
    /// the boxed body otherwise (B28 P2). The struct analogue of
    /// [`GenericValue::tuple`]; `fields` must be in declaration order.
    pub fn struct_value(
        type_name: alloc::string::String,
        fields: alloc::vec::Vec<(alloc::string::String, Self)>,
    ) -> Self {
        let word_bytes = (1usize << <W as crate::word::Word>::BITS_LOG2) / 8;
        let float_bytes = (1usize << <F as crate::float::Float>::BITS_LOG2) / 8;
        Self::struct_with_widths(type_name, fields, word_bytes, float_bytes)
    }

    /// Construct a struct value, choosing the flat byte body for a
    /// transitively-scalar field list and the boxed body otherwise, using
    /// the given scalar widths (B28 P2).
    ///
    /// The single choke point for struct construction, so the VM
    /// `NewStruct` handler, constant materialisation, and host marshalling
    /// agree on the representation a struct type uses, which equality relies
    /// on. `fields` are packed in declaration order, the same order the
    /// compiler bakes field offsets against; the eligibility rule is the
    /// same as for a tuple field (`flat_tuple_scalar_kind`). A flat body
    /// carries no type name or field names.
    pub fn struct_with_widths(
        type_name: alloc::string::String,
        fields: alloc::vec::Vec<(alloc::string::String, Self)>,
        word_bytes: usize,
        float_bytes: usize,
    ) -> Self {
        match Self::try_pack_flat(fields.iter().map(|(_, v)| v), 0, word_bytes, float_bytes) {
            Some(body) => Self::Struct(StructBody::Flat(body)),
            None => Self::Struct(StructBody::boxed(type_name, fields)),
        }
    }

    /// Construct an enum value at the runtime's own scalar widths, choosing
    /// the flat byte body for a transitively-scalar payload and the boxed
    /// body otherwise (B28 P2). `disc` is the variant's discriminant value.
    pub fn enum_value(
        type_name: alloc::string::String,
        variant: alloc::string::String,
        disc: i64,
        fields: alloc::vec::Vec<Self>,
    ) -> Self {
        let word_bytes = (1usize << <W as crate::word::Word>::BITS_LOG2) / 8;
        let float_bytes = (1usize << <F as crate::float::Float>::BITS_LOG2) / 8;
        // Ad-hoc construction with no type-table knowledge: produce a
        // variant-sized (unpadded) body. Padding-tolerant flat-enum
        // equality lets this still compare equal to a compiler-padded
        // value of the same variant (B28 P2).
        Self::enum_with_widths(type_name, variant, disc, fields, 0, word_bytes, float_bytes)
    }

    /// Construct an enum value, choosing the flat byte body for a
    /// transitively-scalar payload and the boxed body otherwise, using the
    /// given scalar widths (B28 P2).
    ///
    /// The single choke point for enum construction, so the VM `NewEnum`
    /// handler, constant materialisation, and host marshalling agree on the
    /// representation. The flat body is `[disc: word_bytes][payload]`: the
    /// discriminant is written as a `Word` at offset zero, then the payload
    /// fields are packed in declaration order. A payload field that is not
    /// a flat-eligible scalar forces the boxed body.
    pub fn enum_with_widths(
        type_name: alloc::string::String,
        variant: alloc::string::String,
        disc: i64,
        fields: alloc::vec::Vec<Self>,
        min_payload: usize,
        word_bytes: usize,
        float_bytes: usize,
    ) -> Self {
        // The built-in `Option` enum is generic and is not registered in
        // the compiler's enum type tables, so the access ops bake the boxed
        // form for it; keep its construction boxed too, so the two agree
        // (B28 P2). `Option::None` is the separate `Value::None`; only
        // `Option::Some` reaches here.
        if type_name == "Option" {
            return Self::Enum(EnumBody::boxed(type_name, variant, fields));
        }
        // The flat enum body is `[disc word][payload]`. The discriminant is
        // the first packed field (an `Int` written as a `Word`), matching
        // the `Enum as Word` cast and what `Op::IsEnum` reads. `min_payload`
        // is the type's largest-variant payload, baked by the compiler so
        // every value of a uniformly-flat enum shares one fixed body size,
        // which a nested enum field's fixed slot requires (B28 P2). The
        // padded body is `word + min_payload`; the trailing slot is zero
        // padding, which padding-tolerant equality tolerates.
        let disc_value = Self::Int(<W as crate::word::Word>::from_i64_wrap(disc));
        let min_bytes = word_bytes + min_payload;
        match Self::try_pack_flat(
            core::iter::once(&disc_value).chain(fields.iter()),
            min_bytes,
            word_bytes,
            float_bytes,
        ) {
            Some(body) => Self::Enum(EnumBody::Flat(body)),
            None => Self::Enum(EnumBody::boxed(type_name, variant, fields)),
        }
    }

    /// Write this fixed-size scalar's little-endian bytes into `dst` at
    /// `offset` (B28 P2). The width of an `Int`/`Fixed` is `word_bytes`
    /// and of a `Float` is `float_bytes`, taken from the runtime's
    /// target descriptor, so the same routine serves narrow runtimes.
    /// `Unit` and `None` write nothing.
    ///
    /// This is the pack half of the composite construct handlers: each
    /// field's scalar is written at the offset the compiler baked.
    /// Reference scalars (`StaticStr`, `KStr`, `Opaque`) and composites
    /// are handled by later phases and panic here, which a correct
    /// compiler never reaches because it routes them differently.
    #[cfg_attr(not(feature = "floats"), allow(unused_variables))]
    pub fn write_scalar_le(
        &self,
        dst: &mut [u8],
        offset: usize,
        word_bytes: usize,
        float_bytes: usize,
    ) {
        match self {
            Self::Unit | Self::None => {}
            Self::Bool(b) => dst[offset] = u8::from(*b),
            Self::Byte(b) => dst[offset] = *b,
            Self::Int(w) | Self::Fixed(w) => {
                let le = w.to_i64().to_le_bytes();
                dst[offset..offset + word_bytes].copy_from_slice(&le[..word_bytes]);
            }
            #[cfg(feature = "floats")]
            Self::Float(f) => {
                let v = f.to_f64();
                match float_bytes {
                    8 => dst[offset..offset + 8].copy_from_slice(&v.to_le_bytes()),
                    4 => dst[offset..offset + 4].copy_from_slice(&(v as f32).to_le_bytes()),
                    other => panic!("write_scalar_le: unsupported float width {other}"),
                }
            }
            // A `KStr` Text field is two words: the arena data pointer then
            // the byte length, each `word_bytes` wide (B28 P3). The epoch is
            // reattached at the read side, not stored. This requires
            // `word_bytes` to be at least the host pointer width, which the
            // bundled `i64` runtime satisfies; a narrower-word target keeps
            // `Text` boxed (a separate compile-time gate).
            Self::KStr(ks) => {
                let (ptr, len) = ks.raw_parts();
                debug_assert!(
                    word_bytes >= core::mem::size_of::<usize>(),
                    "flat Text requires word_bytes >= host pointer width"
                );
                let pe = (ptr as u64).to_le_bytes();
                dst[offset..offset + word_bytes].copy_from_slice(&pe[..word_bytes]);
                let le = (len as u64).to_le_bytes();
                dst[offset + word_bytes..offset + 2 * word_bytes]
                    .copy_from_slice(&le[..word_bytes]);
            }
            other => panic!("write_scalar_le: not a fixed-size scalar: {other:?}"),
        }
    }

    /// Build a flat composite of `kind` from `values`, allocating
    /// `byte_size` bytes (B28 P4). The values are packed in order (for an
    /// enum the first is the discriminant word) and the body is padded to
    /// `byte_size`; the result is wrapped as the matching composite kind.
    /// Returns `None` if any value is not flat-eligible, which a correct
    /// compiler never produces for a `Flat` operand (the type is statically
    /// flat). The single flat-construction choke point for `Op::NewComposite`.
    pub fn new_composite_flat(
        kind: crate::value_layout::CompositeKind,
        values: alloc::vec::Vec<Self>,
        byte_size: usize,
        word_bytes: usize,
        float_bytes: usize,
    ) -> Option<Self> {
        use crate::value_layout::CompositeKind as C;
        let fc = Self::try_pack_flat(values.iter(), byte_size, word_bytes, float_bytes)?;
        Some(match kind {
            C::Tuple => Self::Tuple(TupleBody::Flat(fc)),
            C::Array => Self::Array(ArrayBody::Flat(fc)),
            C::Struct => Self::Struct(StructBody::Flat(fc)),
            C::Enum => Self::Enum(EnumBody::Flat(fc)),
        })
    }

    /// Build a boxed composite of `kind` from `values` (B28 P4). For a
    /// struct the `names` are the field names (declaration order); for an
    /// enum `names[0]` is the variant name and `type_name` the enum name; a
    /// tuple or array ignores `type_name` and `names`. The boxed form is the
    /// interim representation for a reference-bearing field or `Option`,
    /// removed at P3.
    pub fn new_composite_boxed(
        kind: crate::value_layout::CompositeKind,
        type_name: alloc::string::String,
        names: alloc::vec::Vec<alloc::string::String>,
        values: alloc::vec::Vec<Self>,
    ) -> Self {
        use crate::value_layout::CompositeKind as C;
        match kind {
            C::Tuple => Self::Tuple(TupleBody::Boxed(values)),
            C::Array => Self::Array(ArrayBody::Boxed(values)),
            C::Struct => Self::Struct(StructBody::boxed(
                type_name,
                names.into_iter().zip(values).collect(),
            )),
            C::Enum => {
                let variant = names.into_iter().next().unwrap_or_default();
                Self::Enum(EnumBody::boxed(type_name, variant, values))
            }
        }
    }

    /// Re-wrap a nested composite's extracted byte range as a flat
    /// composite `Value` of the given kind (B28 P2 nested inlining). The
    /// access handler slices the child body out of the parent and calls
    /// this to materialise the field value. The bytes are copied into a
    /// fresh body, so the result is independent of the parent.
    pub fn from_flat_nested_bytes(
        bytes: &[u8],
        variant: crate::value_layout::CompositeKind,
        epoch: u64,
    ) -> Self {
        use crate::value_layout::CompositeKind as C;
        // The child inherits the parent body's epoch so its own flat `Text`
        // field reattaches that epoch on read and resolves `Stale` after a
        // `RESET` rather than dereferencing reclaimed memory (B28 P3 item 1).
        let fc = crate::flat_value::FlatComposite::from_bytes_with_epoch(bytes.to_vec(), epoch);
        match variant {
            C::Tuple => Self::Tuple(TupleBody::Flat(fc)),
            C::Array => Self::Array(ArrayBody::Flat(fc)),
            C::Struct => Self::Struct(StructBody::Flat(fc)),
            C::Enum => Self::Enum(EnumBody::Flat(fc)),
        }
    }

    /// Migrate a flat composite value's body to the arena's top ephemeral
    /// head (B28 P2 arena residence). A `Flat`-bodied tuple, array, struct,
    /// or enum has its body copied to the arena and replaced with an
    /// epoch-guarded handle; any other value (a scalar, a boxed composite,
    /// a reference) is returned unchanged. The VM calls this on a
    /// freshly-constructed composite so it carries no global-heap allocation
    /// across a `loop` iteration's `RESET`.
    pub fn into_arena_body(
        self,
        arena: &keleusma_arena::Arena,
    ) -> Result<Self, allocator_api2::alloc::AllocError> {
        Ok(match self {
            Self::Tuple(TupleBody::Flat(fc)) => Self::Tuple(TupleBody::Flat(fc.in_arena(arena)?)),
            Self::Array(ArrayBody::Flat(fc)) => Self::Array(ArrayBody::Flat(fc.in_arena(arena)?)),
            Self::Struct(StructBody::Flat(fc)) => {
                Self::Struct(StructBody::Flat(fc.in_arena(arena)?))
            }
            Self::Enum(EnumBody::Flat(fc)) => Self::Enum(EnumBody::Flat(fc.in_arena(arena)?)),
            other => other,
        })
    }

    /// Materialise any arena-resident composite body in this value back to
    /// an owned `Inline` body (B28 P2 arena residence). A `Flat` body is
    /// copied out of the arena (its bytes are self-contained, so nested
    /// composites come with it); a `Boxed` body recurses into its element
    /// values, since those are separate values that may themselves be
    /// arena-resident. Scalars and references are returned unchanged.
    ///
    /// Used to bridge arena bodies across the three points that read bytes
    /// without an arena handle: the shared construction packer (which reads
    /// a child field's bytes to inline them), value equality, and the
    /// native-call boundary (where `from_value` has no arena).
    /// The originating arena epoch of this value's flat composite body, if
    /// it has one (B28 P3 item 1). A flat `Text` field is decoded by
    /// reattaching this epoch so a read after a `RESET` resolves `Stale`.
    /// Returns `None` for a boxed or non-composite value, whose reference
    /// fields (a bare `KStr`, an opaque index) carry their own validity.
    pub fn flat_ref_epoch(&self) -> Option<u64> {
        match self {
            Self::Tuple(TupleBody::Flat(fc)) => Some(fc.ref_epoch()),
            Self::Array(ArrayBody::Flat(fc)) => Some(fc.ref_epoch()),
            Self::Struct(StructBody::Flat(fc)) => Some(fc.ref_epoch()),
            Self::Enum(EnumBody::Flat(fc)) => Some(fc.ref_epoch()),
            _ => None,
        }
    }

    pub fn materialized(self, arena: &keleusma_arena::Arena) -> Self {
        match self {
            Self::Tuple(TupleBody::Flat(fc)) => Self::Tuple(TupleBody::Flat(fc.to_inline(arena))),
            Self::Array(ArrayBody::Flat(fc)) => Self::Array(ArrayBody::Flat(fc.to_inline(arena))),
            Self::Struct(StructBody::Flat(fc)) => {
                Self::Struct(StructBody::Flat(fc.to_inline(arena)))
            }
            Self::Enum(EnumBody::Flat(fc)) => Self::Enum(EnumBody::Flat(fc.to_inline(arena))),
            Self::Tuple(TupleBody::Boxed(elems)) => Self::Tuple(TupleBody::Boxed(
                elems.into_iter().map(|e| e.materialized(arena)).collect(),
            )),
            Self::Array(ArrayBody::Boxed(elems)) => Self::Array(ArrayBody::Boxed(
                elems.into_iter().map(|e| e.materialized(arena)).collect(),
            )),
            Self::Struct(StructBody::Boxed(b)) => {
                let BoxedStruct { type_name, fields } = *b;
                Self::Struct(StructBody::boxed(
                    type_name,
                    fields
                        .into_iter()
                        .map(|(n, v)| (n, v.materialized(arena)))
                        .collect(),
                ))
            }
            Self::Enum(EnumBody::Boxed(b)) => {
                let BoxedEnum {
                    type_name,
                    variant,
                    fields,
                } = *b;
                Self::Enum(EnumBody::boxed(
                    type_name,
                    variant,
                    fields.into_iter().map(|e| e.materialized(arena)).collect(),
                ))
            }
            other => other,
        }
    }

    /// Append a flat composite field's bytes to `buf`: a scalar's
    /// little-endian bytes or a nested flat composite's body bytes (B28
    /// P2). The caller has verified flat eligibility through
    /// `flat_field_size`, so a value that is neither a flat scalar nor a
    /// flat composite never reaches here. Appending (rather than writing at
    /// a pre-zeroed offset) means the packed region is written exactly
    /// once; only the trailing slack (an enum's padding to the largest
    /// variant) is zero-filled by the caller (B28 P4 alloc model).
    fn push_flat_field(
        &self,
        buf: &mut alloc::vec::Vec<u8>,
        word_bytes: usize,
        float_bytes: usize,
    ) {
        if flat_tuple_scalar_kind(self).is_some() {
            let start = buf.len();
            let size = flat_tuple_scalar_kind(self)
                .expect("scalar checked")
                .size_in_bytes(word_bytes, float_bytes);
            buf.resize(start + size, 0);
            self.write_scalar_le(buf, start, word_bytes, float_bytes);
        } else if let Some(bytes) = flat_body_bytes(self) {
            buf.extend_from_slice(bytes);
        }
    }

    /// Pack `values` into a flat byte body, padded to at least `min_bytes`
    /// (B28 P4). Returns `None` if any value is not flat-eligible (a
    /// reference or float field) or the packed body exceeds the sixteen-bit
    /// access offset, in which case the caller falls back to the boxed body.
    ///
    /// The fields are appended in order, so each packed byte is written
    /// exactly once; the body is then grown to `min_bytes` with zeros,
    /// which is the only zero-fill and is non-empty solely for an enum
    /// padded to its largest variant. `min_bytes` is the explicit
    /// allocation size the compiler bakes and the worst-case-memory-usage
    /// verifier sums.
    fn try_pack_flat<'a, I>(
        values: I,
        min_bytes: usize,
        word_bytes: usize,
        float_bytes: usize,
    ) -> Option<crate::flat_value::FlatComposite>
    where
        I: IntoIterator<Item = &'a Self>,
        Self: 'a,
    {
        let mut buf = alloc::vec::Vec::new();
        for v in values {
            flat_field_size(v, word_bytes, float_bytes)?;
            v.push_flat_field(&mut buf, word_bytes, float_bytes);
        }
        // Grow to the minimum (the enum padding slack); never shrink.
        if buf.len() < min_bytes {
            buf.resize(min_bytes, 0);
        }
        if buf.len() > u16::MAX as usize {
            return None;
        }
        Some(crate::flat_value::FlatComposite::from_bytes(buf))
    }

    /// Read a fixed-size scalar of `kind` from `src` at `offset` (B28
    /// P2), the read half of the composite access handlers. `Int` and
    /// `Fixed` are sign-extended from `word_bytes`; `Float` is widened
    /// from `float_bytes`. `kind` is the value the compiler baked into
    /// the access instruction. Panics on the reference kinds and on a
    /// `kind` outside the fixed-size scalar set, which later phases
    /// handle.
    #[cfg_attr(not(feature = "floats"), allow(unused_variables))]
    pub fn read_scalar_le(
        src: &[u8],
        offset: usize,
        kind: crate::value_layout::ScalarKind,
        word_bytes: usize,
        float_bytes: usize,
    ) -> Self {
        use crate::value_layout::ScalarKind;
        match kind {
            ScalarKind::Unit => Self::Unit,
            ScalarKind::Bool => Self::Bool(src[offset] != 0),
            ScalarKind::Byte => Self::Byte(src[offset]),
            ScalarKind::Int | ScalarKind::Fixed => {
                let mut buf = [0u8; 8];
                buf[..word_bytes].copy_from_slice(&src[offset..offset + word_bytes]);
                let mut n = i64::from_le_bytes(buf);
                // Sign-extend a narrow word from its top bit.
                if word_bytes < 8 {
                    let bits = word_bytes * 8;
                    let sign = 1i64 << (bits - 1);
                    if n & sign != 0 {
                        n |= !((1i64 << bits) - 1);
                    }
                }
                let w = W::from_i64_wrap(n);
                if matches!(kind, ScalarKind::Fixed) {
                    Self::Fixed(w)
                } else {
                    Self::Int(w)
                }
            }
            #[cfg(feature = "floats")]
            ScalarKind::Float => {
                let v = match float_bytes {
                    8 => {
                        let mut buf = [0u8; 8];
                        buf.copy_from_slice(&src[offset..offset + 8]);
                        f64::from_le_bytes(buf)
                    }
                    4 => {
                        let mut buf = [0u8; 4];
                        buf.copy_from_slice(&src[offset..offset + 4]);
                        f32::from_le_bytes(buf) as f64
                    }
                    other => panic!("read_scalar_le: unsupported float width {other}"),
                };
                Self::Float(F::from_f64(v))
            }
            ScalarKind::Text | ScalarKind::Opaque => {
                panic!("read_scalar_le: reference kinds are handled in B28 P3")
            }
        }
    }

    /// Walk the value recursively and replace every `KStr` variant
    /// with an equivalent `StaticStr` whose contents come from the
    /// supplied arena. Use this when transporting a value across a
    /// Vm boundary: `KStr` handles reference the original arena
    /// through an epoch-tagged pointer, so a value snapshotted from
    /// one Vm and restored into a Vm backed by a different arena
    /// would carry a stale handle. Materialising to `StaticStr`
    /// breaks the arena dependency so the value is portable.
    ///
    /// Composite variants (`Tuple`, `Array`, `Struct`, `Enum`) are
    /// walked recursively. Scalar variants are cloned unchanged.
    /// `Opaque` values are cloned by `Arc` increment as usual; the
    /// `HostOpaque` trait makes no assumption about arena residency.
    ///
    /// Stale `KStr` handles produce an empty `StaticStr`. A stale
    /// handle here means the original arena was already dropped
    /// between snapshot and materialisation, which should not happen
    /// in the documented REPL pattern but is handled defensively.
    pub fn materialise_kstrings(&self, arena: &keleusma_arena::Arena) -> Self {
        match self {
            Self::KStr(handle) => match handle.get(arena) {
                Ok(s) => Self::StaticStr(alloc::string::String::from(s)),
                Err(_) => Self::StaticStr(alloc::string::String::new()),
            },
            // A flat tuple is transitively scalar, so it holds no KStr
            // and materialises to itself. A boxed tuple may carry a
            // KStr and is walked element-wise.
            Self::Tuple(TupleBody::Flat(_)) => self.clone(),
            Self::Tuple(TupleBody::Boxed(items)) => Self::tuple(
                items
                    .iter()
                    .map(|v| v.materialise_kstrings(arena))
                    .collect(),
            ),
            // A flat array is transitively scalar and holds no KStr; a
            // boxed array is walked element-wise.
            Self::Array(ArrayBody::Flat(_)) => self.clone(),
            Self::Array(ArrayBody::Boxed(items)) => Self::array(
                items
                    .iter()
                    .map(|v| v.materialise_kstrings(arena))
                    .collect(),
            ),
            Self::Struct(StructBody::Flat(_)) => self.clone(),
            Self::Struct(StructBody::Boxed(b)) => Self::struct_value(
                b.type_name.clone(),
                b.fields
                    .iter()
                    .map(|(k, v)| (k.clone(), v.materialise_kstrings(arena)))
                    .collect(),
            ),
            // A flat enum is transitively scalar and holds no KStr.
            Self::Enum(EnumBody::Flat(_)) => self.clone(),
            Self::Enum(EnumBody::Boxed(b)) => Self::Enum(EnumBody::boxed(
                b.type_name.clone(),
                b.variant.clone(),
                b.fields
                    .iter()
                    .map(|v| v.materialise_kstrings(arena))
                    .collect(),
            )),
            other => other.clone(),
        }
    }

    /// Return a human-readable type name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Unit => "Unit",
            Self::Bool(_) => "Bool",
            Self::Int(_) => "Int",
            Self::Byte(_) => "Byte",
            Self::Fixed(_) => "Fixed",
            #[cfg(feature = "floats")]
            Self::Float(_) => "Float",
            Self::StaticStr(_) => "StaticStr",
            Self::KStr(_) => "KStr",
            Self::Tuple(_) => "Tuple",
            Self::Array(_) => "Array",
            Self::Struct { .. } => "Struct",
            Self::Enum(_) => "Enum",
            Self::None => "None",
            // Returning a `&'static str` for an opaque value would
            // require leaking the host-supplied name, so we surface
            // a generic literal here. Diagnostics that need the
            // host's specific name read it through
            // [`GenericValue::opaque_type_name`].
            Self::Opaque(_) => "Opaque",
            #[cfg(not(feature = "floats"))]
            Self::_PhantomFloat(_) => unreachable!("_PhantomFloat is never constructed"),
        }
    }

    /// Return the host-supplied script-side type name for an
    /// opaque value, or `None` if the value is not opaque.
    pub fn opaque_type_name(&self) -> Option<&'static str> {
        match self {
            Self::Opaque(o) => Some(o.type_name()),
            _ => None,
        }
    }

    /// Borrow the underlying UTF-8 contents of a static string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::StaticStr(s) => Some(s.as_str()),
            _ => Option::None,
        }
    }

    /// Borrow the underlying UTF-8 contents of any string variant,
    /// resolving `KStr` through the supplied arena.
    pub fn as_str_with_arena<'a>(
        &'a self,
        arena: &'a keleusma_arena::Arena,
    ) -> Result<Option<&'a str>, keleusma_arena::Stale> {
        match self {
            Self::StaticStr(s) => Ok(Some(s.as_str())),
            Self::KStr(h) => h.get(arena).map(Some),
            _ => Ok(Option::None),
        }
    }

    /// Returns true if the value is an arena-resident dynamic
    /// string or transitively contains one.
    pub fn contains_dynstr(&self) -> bool {
        match self {
            Self::KStr(_) => true,
            // A flat tuple is transitively scalar and cannot hold a
            // dynamic string; a boxed tuple is walked element-wise.
            Self::Tuple(TupleBody::Flat(_)) => false,
            Self::Tuple(TupleBody::Boxed(items)) => items.iter().any(Self::contains_dynstr),
            Self::Array(ArrayBody::Flat(_)) => false,
            Self::Array(ArrayBody::Boxed(items)) => items.iter().any(Self::contains_dynstr),
            Self::Struct(StructBody::Flat(_)) => false,
            Self::Struct(StructBody::Boxed(b)) => b.fields.iter().any(|(_, v)| v.contains_dynstr()),
            Self::Enum(EnumBody::Flat(_)) => false,
            Self::Enum(EnumBody::Boxed(b)) => b.fields.iter().any(Self::contains_dynstr),
            _ => false,
        }
    }

    /// Lift an archived constant pool entry into a runtime
    /// `GenericValue<W, F>`.
    ///
    /// The constant pool stores [`ConstValue`] entries with fixed
    /// `i64` and `f64` payloads; this lift converts each constant
    /// to the runtime's `W` and `F` types via `Word::from_i64_wrap`
    /// and `Float::from_f64`. The conversion truncates / rounds
    /// when the runtime's word or float width is narrower than
    /// the bytecode's; programs whose constants do not fit are
    /// rejected at load time by the bytecode-header width check.
    pub fn from_const_archived(
        c: &ArchivedConstValue,
        word_bytes: usize,
        float_bytes: usize,
    ) -> Self {
        match c {
            ArchivedConstValue::Unit => Self::Unit,
            ArchivedConstValue::Bool(b) => Self::Bool(*b),
            ArchivedConstValue::Int(i) => Self::Int(W::from_i64_wrap(i.to_native())),
            ArchivedConstValue::Byte(b) => Self::Byte(*b),
            ArchivedConstValue::Fixed(i) => Self::Fixed(W::from_i64_wrap(i.to_native())),
            #[cfg(feature = "floats")]
            ArchivedConstValue::Float(f) => Self::Float(F::from_f64(f.to_native())),
            ArchivedConstValue::StaticStr(s) => {
                use alloc::string::ToString;
                Self::StaticStr(s.as_str().to_string())
            }
            // A constant tuple materialises through the same flat-or-boxed
            // choice as every other construction path, so a scalar
            // constant tuple matches a runtime-built one and the baked
            // flat access reads it correctly (B28 P2).
            ArchivedConstValue::Tuple(items) => Self::tuple_with_widths(
                items
                    .iter()
                    .map(|c| Self::from_const_archived(c, word_bytes, float_bytes))
                    .collect(),
                word_bytes,
                float_bytes,
            ),
            ArchivedConstValue::Array(items) => Self::array_with_widths(
                items
                    .iter()
                    .map(|c| Self::from_const_archived(c, word_bytes, float_bytes))
                    .collect(),
                word_bytes,
                float_bytes,
            ),
            ArchivedConstValue::Struct { type_name, fields } => {
                use alloc::string::ToString;
                Self::struct_with_widths(
                    type_name.as_str().to_string(),
                    fields
                        .iter()
                        .map(|kv| {
                            (
                                kv.0.as_str().to_string(),
                                Self::from_const_archived(&kv.1, word_bytes, float_bytes),
                            )
                        })
                        .collect(),
                    word_bytes,
                    float_bytes,
                )
            }
            ArchivedConstValue::Enum {
                type_name,
                variant,
                discriminant,
                fields,
            } => {
                use alloc::string::ToString;
                let materialised: alloc::vec::Vec<Self> = fields
                    .iter()
                    .map(|c| Self::from_const_archived(c, word_bytes, float_bytes))
                    .collect();
                // A resolved discriminant lets the constant materialise
                // into the flat body that matches the baked access; an
                // unresolved one stays boxed (B28 P2).
                match discriminant.as_ref().map(|d| d.to_native()) {
                    Some(disc) => Self::enum_with_widths(
                        type_name.as_str().to_string(),
                        variant.as_str().to_string(),
                        disc,
                        materialised,
                        // Constants carry no per-type padding hint; a const
                        // enum materialises variant-sized and relies on
                        // padding-tolerant equality. Const composites that
                        // would nest an enum are not flat-folded (see the
                        // compiler's const path), so a variant-sized const
                        // enum is never inlined into a fixed parent slot.
                        0,
                        word_bytes,
                        float_bytes,
                    ),
                    None => Self::Enum(EnumBody::boxed(
                        type_name.as_str().to_string(),
                        variant.as_str().to_string(),
                        materialised,
                    )),
                }
            }
            ArchivedConstValue::None => Self::None,
        }
    }
}

/// Classification of a compiled function chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum BlockType {
    /// Atomic total function (`fn`). No yields, no streaming.
    Func,
    /// Non-atomic total function (`yield fn`). Must contain at least one Yield.
    Reentrant,
    /// Productive divergent function (`loop fn`). Contains Stream/Reset and Yield.
    Stream,
}

/// The specific cause of an [`Op::Trap`]. The compiler encodes the
/// kind in the trap instruction's operand, and the virtual machine
/// surfaces it through `VmError::Trap` so a host can categorize the
/// fault without parsing a message string. These are the compiler-
/// emitted traps for partial operations whose unhandled case has no
/// in-band result, as distinct from the data faults that already
/// have their own `VmError` variants such as division by zero and
/// out-of-bounds indexing.
///
/// B35 (Partial Operation Handling) introduced this kind in place of
/// the prior free-form trap message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapKind {
    /// A newtype refinement predicate returned false at a
    /// construction site.
    RefinementFailed,
    /// No head of a multiheaded function matched the arguments.
    NoMatchingHead,
    /// No arm of a `match` expression matched the scrutinee. This is
    /// reachable only when every arm carries a `when` guard, since
    /// the type checker proves unguarded matches exhaustive.
    NoMatchingArm,
    /// No arm of a checked-arithmetic construct matched the outcome.
    /// Reachable only through guarded arms, defensive otherwise.
    CheckedArithNoArm,
    /// An enum-to-`Word` cast met a `Value::Enum` whose variant is
    /// outside the declared set. Reachable only through a host-
    /// constructed enum value.
    EnumVariantUnmapped,
    /// A checked division or modulo met a zero divisor that no
    /// `zero_divisor` arm handled. The virtual machine surfaces this
    /// as `VmError::DivisionByZero`, the same error a plain division
    /// by zero produces.
    ZeroDivisor,
    /// A debug `assert` whose condition evaluated to false. Emitted
    /// only by debug builds (B29); release builds compile the assert
    /// out entirely.
    AssertionFailed,
}

impl TrapKind {
    /// The `u16` code carried in the [`Op::Trap`] operand.
    pub fn code(self) -> u16 {
        match self {
            TrapKind::RefinementFailed => 0,
            TrapKind::NoMatchingHead => 1,
            TrapKind::NoMatchingArm => 2,
            TrapKind::CheckedArithNoArm => 3,
            TrapKind::EnumVariantUnmapped => 4,
            TrapKind::ZeroDivisor => 5,
            TrapKind::AssertionFailed => 6,
        }
    }

    /// Decode a trap kind from an [`Op::Trap`] operand. Returns
    /// `None` for an unrecognized code, which indicates malformed
    /// bytecode.
    pub fn from_code(code: u16) -> Option<TrapKind> {
        match code {
            0 => Some(TrapKind::RefinementFailed),
            1 => Some(TrapKind::NoMatchingHead),
            2 => Some(TrapKind::NoMatchingArm),
            3 => Some(TrapKind::CheckedArithNoArm),
            4 => Some(TrapKind::EnumVariantUnmapped),
            5 => Some(TrapKind::ZeroDivisor),
            6 => Some(TrapKind::AssertionFailed),
            _ => None,
        }
    }
}

/// Baked operand of [`Op::GetTupleField`] (B28 P2).
///
/// The compiler resolves the access at compile time from the
/// ephemeral layout and bakes one of two forms. `Flat` reads the
/// field directly from the composite's byte buffer at `offset` as
/// `kind`, which is the flat representation a transitively-scalar
/// tuple uses. `Boxed` indexes the pre-B28 `Vec` body positionally
/// and is the fallback for a tuple that still carries a reference
/// field or a not-yet-migrated nested composite. The two forms agree
/// with the construction handler by static type, so a given tuple
/// type is always one or the other; the access handler dispatches on
/// the runtime body and faults on a form mismatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TupleField {
    /// Flat read at a compiler-baked byte `offset`, interpreting the
    /// bytes as `kind`. The offset is packed little-endian, the same
    /// layout the construction handler writes.
    Flat {
        /// Byte offset of the field within the composite body.
        offset: u16,
        /// Fixed-size scalar kind to read at the offset.
        kind: crate::value_layout::ScalarKind,
    },
    /// Flat read of a nested composite field: extract `size` bytes at
    /// `offset` from the parent body and re-wrap them as `variant`
    /// (B28 P2 nested inlining). The byte range is a complete child
    /// flat-composite body; the access handler wraps it in a fresh
    /// `Value` of the matching composite kind.
    FlatNested {
        /// Byte offset of the nested composite within the parent body.
        offset: u16,
        /// Byte length of the nested composite body.
        size: u16,
        /// Composite variant to re-wrap the extracted bytes as.
        variant: crate::value_layout::CompositeKind,
    },
    /// Positional index into the boxed `Vec` body (pre-B28 form).
    Boxed {
        /// Zero-based element index.
        index: u8,
    },
}

/// Baked operand of [`Op::GetField`] for struct field access (B28 P2).
///
/// Mirrors [`TupleField`] (a tuple is an anonymous struct), but the boxed
/// form carries the field-name constant-pool index rather than a
/// positional index, because the pre-B28 boxed struct body looks fields up
/// by name. The flat form reads at the compiler-baked byte offset; a struct
/// type is one form or the other by static type, and the access handler
/// dispatches on the runtime body and faults on a form mismatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructField {
    /// Flat read at a compiler-baked byte `offset`, interpreting the bytes
    /// as `kind`. The offset is packed little-endian, the same layout the
    /// construction handler writes.
    Flat {
        /// Byte offset of the field within the composite body.
        offset: u16,
        /// Fixed-size scalar kind to read at the offset.
        kind: crate::value_layout::ScalarKind,
    },
    /// Flat read of a nested composite field: extract `size` bytes at
    /// `offset` from the parent body and re-wrap them as `variant`
    /// (B28 P2 nested inlining).
    FlatNested {
        /// Byte offset of the nested composite within the parent body.
        offset: u16,
        /// Byte length of the nested composite body.
        size: u16,
        /// Composite variant to re-wrap the extracted bytes as.
        variant: crate::value_layout::CompositeKind,
    },
    /// Constant-pool index of the field name, looked up in the boxed body
    /// (pre-B28 form).
    Boxed {
        /// Field-name constant-pool index.
        name_const: u16,
    },
}

/// Baked operand of [`Op::GetEnumField`] for enum-payload access (B28 P2).
///
/// An enum payload field is positional (like a tuple element), so the boxed
/// form carries the index. The flat form carries the byte `offset` within
/// the flat body (already including the leading discriminant word) and the
/// field `kind`. The compiler bakes the form per variant; the access
/// handler dispatches on the runtime body and faults on a form mismatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnumField {
    /// Flat read at a compiler-baked byte `offset` (past the discriminant
    /// word), interpreting the bytes as `kind`.
    Flat {
        /// Byte offset of the payload field within the flat enum body.
        offset: u16,
        /// Fixed-size scalar kind to read at the offset.
        kind: crate::value_layout::ScalarKind,
    },
    /// Flat read of a nested composite payload field: extract `size`
    /// bytes at `offset` (past the discriminant word) from the flat enum
    /// body and re-wrap them as `variant` (B28 P2 nested inlining).
    FlatNested {
        /// Byte offset of the nested composite within the flat enum body.
        offset: u16,
        /// Byte length of the nested composite body.
        size: u16,
        /// Composite variant to re-wrap the extracted bytes as.
        variant: crate::value_layout::CompositeKind,
    },
    /// Positional index into the boxed payload (pre-B28 form).
    Boxed {
        /// Zero-based payload-field index.
        index: u8,
    },
}

/// Baked operand of [`Op::GetIndex`] (B28 P2).
///
/// An array is homogeneous, so unlike a tuple field the element offset
/// is not a compile-time constant; it is `index * element_size`,
/// computed at run time from the index on the stack. The baked operand
/// therefore carries only the element `kind`, from which the element
/// size follows at the runtime's scalar widths. `Flat` reads the
/// element directly from the array's flat byte body; `Boxed` indexes
/// the pre-B28 `Vec` body. The two forms agree with the construction
/// handler by static type, and the access handler dispatches on the
/// runtime body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayElem {
    /// Flat read at `index * element_size`, interpreting the element
    /// bytes as `kind`. The element size is `kind.size_in_bytes` at the
    /// module-declared scalar widths, the same widths the construction
    /// handler packs against.
    Flat {
        /// Fixed-size scalar kind of each element.
        kind: crate::value_layout::ScalarKind,
    },
    /// Flat read of a nested composite element: each element occupies
    /// `size` bytes, so the element offset is `index * size`. Extract the
    /// element's bytes and re-wrap them as `variant` (B28 P2 nested
    /// inlining).
    FlatNested {
        /// Byte length of each nested composite element.
        size: u16,
        /// Composite variant to re-wrap the extracted bytes as.
        variant: crate::value_layout::CompositeKind,
    },
    /// Positional index into the boxed `Vec` body (pre-B28 form).
    Boxed,
}

/// Baked operand of [`Op::NewComposite`] (B28 P4).
///
/// One operand for all four composite kinds. The `Flat` form carries the
/// composite [`crate::value_layout::CompositeKind`], the `count` of values
/// to pop and pack, and the explicit `byte_size` to allocate on the arena
/// top head, which the worst-case-memory-usage verifier sums (conceptually
/// `ALLOCATEBYTES`). For a flat enum the first packed value is the
/// discriminant word, so `count` includes it. The `Boxed` form keeps the
/// boxed body for a reference-bearing field or `Option`; `meta` indexes the
/// chunk's boxed-composite metadata (a struct template, or an enum
/// type-and-variant pair) and is unused for a boxed tuple or array. The
/// boxed form is removed at P3 when reference fields become handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewCompositeOperand {
    /// Allocate `byte_size` bytes, pack `count` popped values, wrap as
    /// `kind`.
    Flat {
        /// Which composite value variant to wrap the packed body as.
        kind: crate::value_layout::CompositeKind,
        /// Number of values to pop and pack (an enum's leading
        /// discriminant counts as one).
        count: u16,
        /// Bytes to allocate; the explicit allocation the WCMU pass sums.
        byte_size: u16,
    },
    /// Build the boxed body: pop `count` values; `meta` indexes the boxed
    /// metadata (struct template or enum type-and-variant).
    Boxed {
        /// Which composite value variant to build.
        kind: crate::value_layout::CompositeKind,
        /// Number of values to pop.
        count: u16,
        /// Index into the chunk's boxed-composite metadata.
        meta: u16,
    },
}

impl NewCompositeOperand {
    /// The composite kind this operand builds.
    pub fn kind(&self) -> crate::value_layout::CompositeKind {
        match self {
            Self::Flat { kind, .. } | Self::Boxed { kind, .. } => *kind,
        }
    }

    /// The number of operand-stack values the construction pops.
    pub fn count(&self) -> u16 {
        match self {
            Self::Flat { count, .. } | Self::Boxed { count, .. } => *count,
        }
    }

    /// The explicit flat allocation byte size, or zero for the boxed form
    /// (B28 P4). This is the value the worst-case-memory-usage verifier
    /// adds to the arena top-head bound for the construction.
    pub fn alloc_bytes(&self) -> u32 {
        match self {
            Self::Flat { byte_size, .. } => *byte_size as u32,
            Self::Boxed { .. } => 0,
        }
    }
}

/// A bytecode instruction.
///
/// V0.2.0 Phase 7c moved opcode serialization out of the rkyv
/// archive and into the [`crate::wire_format`] opcode stream; the
/// rkyv derives retire alongside `ArchivedModule` and
/// `op_from_archived` in Phase 8.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    /// Push a constant from the chunk's constant pool.
    Const(u16),

    /// Push local variable by slot index.
    GetLocal(u16),
    /// Pop and store to local variable slot.
    SetLocal(u16),

    /// Push data segment slot value onto stack.
    GetData(u16),
    /// Pop value and store into data segment slot.
    SetData(u16),

    /// Indexed read from a data-segment array. The first immediate is
    /// the array's base slot, the second is the array's total slot
    /// count. The opcode pops a `Value::Int` index from the operand
    /// stack, checks `0 <= index < total`, traps if the index is out
    /// of range, and pushes `data[base + index]`. Used by the compiler
    /// for `state.field[i]` reads when `state.field` is an array-typed
    /// data field.
    GetDataIndexed(u16, u16),
    /// Indexed write to a data-segment array. The first immediate is
    /// the array's base slot, the second is the array's total slot
    /// count. The opcode pops the `Value::Int` index, then pops the
    /// new value, checks `0 <= index < total`, traps if out of range,
    /// and stores `data[base + index] = value`.
    SetDataIndexed(u16, u16),
    /// Bounds check against the value on top of the operand stack
    /// without modifying the stack. The immediate is the exclusive
    /// upper bound. Traps when the top is not a `Value::Int`, when
    /// the value is negative, or when the value is greater than or
    /// equal to the bound. Used by the compiler to validate each
    /// level of a multi-dimensional `state.field[i][j]...` access
    /// before the per-level stride arithmetic computes the flat
    /// offset.
    BoundsCheck(u16),

    /// Binary addition.
    Add,
    /// Binary subtraction.
    Sub,
    /// Binary multiplication.
    Mul,
    /// Binary division.
    Div,
    /// Binary modulo.
    Mod,
    /// Unary negation.
    Neg,

    /// Equality comparison.
    CmpEq,
    /// Inequality comparison.
    CmpNe,
    /// Less than comparison.
    CmpLt,
    /// Greater than comparison.
    CmpGt,
    /// Less than or equal comparison.
    CmpLe,
    /// Greater than or equal comparison.
    CmpGe,

    /// Logical NOT.
    Not,

    // -- Block-structured control flow --
    /// Pop bool; if false, skip to target (matching Else or EndIf).
    /// Target is an op index within the current chunk; chunks are
    /// capped at `u16::MAX` ops by the compiler.
    If(u16),
    /// Skip to target (matching EndIf). Reached when then-block
    /// falls through. Target is an op index within the current
    /// chunk.
    Else(u16),
    /// Block delimiter for If/Else. No-op at runtime.
    EndIf,

    /// Begin loop block. Target is past EndLoop (used by Break and
    /// BreakIf). Target is an op index within the current chunk.
    Loop(u16),
    /// Back-edge to instruction after matching Loop. Target is an
    /// op index within the current chunk.
    EndLoop(u16),
    /// Unconditional forward jump past enclosing EndLoop. Target is
    /// an op index within the current chunk.
    Break(u16),
    /// Pop bool; if true, forward jump past enclosing EndLoop.
    /// Target is an op index within the current chunk.
    BreakIf(u16),

    // -- Streaming --
    /// Stream block entry marker. No-op at runtime.
    Stream,
    /// Clear arena, return VmState::Reset to host.
    Reset,

    // -- Functions --
    /// Call compiled function by chunk index with N arguments.
    Call(u16, u8),
    /// Return from the current function.
    Return,

    /// Yield: pop output value, suspend. On resume, input is pushed.
    Yield,

    /// Duplicate top of stack.
    Dup,

    /// Build a composite of any kind from the top values (B28 P4). The
    /// single construction instruction that consolidates `NewStruct`,
    /// `NewTuple`, `NewArray`, and `NewEnum`: a tuple is an anonymous
    /// struct, an array a homogeneous struct, and a flat enum a struct
    /// whose first packed value is the discriminant word. The flat form
    /// carries the explicit allocation byte size the worst-case-memory-usage
    /// verifier sums (conceptually `ALLOCATEBYTES`); the boxed form (a
    /// reference-bearing field, or `Option`) carries the metadata index.
    NewComposite(NewCompositeOperand),

    /// Pop struct, push field value. The baked [`StructField`] operand
    /// selects a flat read at a compiler-baked byte offset or a by-name
    /// lookup in the boxed body (B28 P2).
    GetField(StructField),
    /// Pop index (Int), pop array, push element. The baked
    /// [`ArrayElem`] operand selects a flat read at `index * size` or a
    /// positional index into the boxed body (B28 P2).
    GetIndex(ArrayElem),
    /// Pop tuple, push element. The baked [`TupleField`] operand
    /// selects a flat read at an offset or a positional index into the
    /// boxed body (B28 P2).
    GetTupleField(TupleField),
    /// Pop enum, push payload field. The baked [`EnumField`] operand
    /// selects a flat read at a compiler-baked byte offset (past the
    /// discriminant word) or a positional index into the boxed body
    /// (B28 P2).
    GetEnumField(EnumField),
    /// Pop composite value, push its length as Int.
    Len,

    /// Peek at TOS: push true if matching enum type and variant, false
    /// otherwise. Operands are the enum-name and variant-name constant-pool
    /// indices and the variant discriminant constant index. The boxed body
    /// compares the variant name; the flat body compares the leading
    /// discriminant word to the constant (B28 P2).
    IsEnum(u16, u16, u16),
    /// Peek at TOS: push true if matching struct type, false otherwise.
    IsStruct(u16),

    /// Cast i64 to f64.
    IntToFloat,
    /// Cast f64 to i64 (truncation).
    FloatToInt,
    /// Cast `Word` to `Byte`. Pops a `Value::Int`, masks to the
    /// low eight bits, pushes `Value::Byte`. Defined for any
    /// `Value::Int`; out-of-range Words wrap mod 256.
    WordToByte,
    /// Cast `Byte` to `Word`. Pops a `Value::Byte`, zero-extends
    /// to `i64`, pushes `Value::Int`.
    ByteToWord,
    /// Cast `Word` to `Fixed` with the given fraction-bit count.
    /// Pops a `Value::Int`, left-shifts by `frac_bits`, pushes
    /// `Value::Fixed`. Overflow saturates at `i64::MAX`/`MIN`.
    WordToFixed(u8),
    /// Cast `Fixed` (with the given fraction-bit count) to `Word`.
    /// Pops a `Value::Fixed`, arithmetic-right-shifts by
    /// `frac_bits`, pushes `Value::Int`. Truncates toward
    /// negative infinity per arithmetic shift.
    FixedToWord(u8),
    /// Multiply two `Fixed` operands sharing the given fraction-bit
    /// count. Pops two `Value::Fixed`, computes
    /// `(a as i128 * b as i128) >> frac_bits`, pushes
    /// `Value::Fixed`. Saturates at `i64::MAX`/`MIN` on overflow.
    FixedMul(u8),
    /// Divide two `Fixed` operands sharing the given fraction-bit
    /// count. Pops two `Value::Fixed`, computes
    /// `(a as i128 << frac_bits) / b as i128`, pushes
    /// `Value::Fixed`. Saturates at `i64::MAX`/`MIN`. Returns
    /// `VmError::DivisionByZero` for `b == 0`.
    FixedDiv(u8),

    /// Halt execution with a runtime error.
    Trap(u16),

    /// Overflow-checked Word addition. Pops two `Value::Int`
    /// operands, computes the true sum in `i128`, and pushes three
    /// slots: the high 64 bits as `Value::Int`, the low 64 bits as
    /// `Value::Int`, and an outcome flag `Value::Int(0)` (ok),
    /// `Value::Int(1)` (overflow), or `Value::Int(2)` (underflow).
    /// The compiler stashes all three into temporary locals at the
    /// dispatch site. The construct's surface form is `expr {
    /// ok(v) => ..., overflow(h, l) => ..., underflow(h, l) =>
    /// ... }`.
    CheckedAdd,
    /// Overflow-checked Word subtraction. Same stack effect as
    /// `Op::CheckedAdd`. The true difference is computed in `i128`
    /// and split into high and low halves before the flag.
    CheckedSub,
    /// Overflow-checked multiplication parameterized by a Q-format
    /// fraction-bit count (B35 P3d-iii). The operand is `0` for
    /// integer multiplication and greater than zero for `Fixed`
    /// multiplication, where the `i128` product is arithmetic-shifted
    /// right by that many bits before the range check, so `0`
    /// fraction bits is exactly integer multiply. For integer
    /// operands the true product is computed in `i128` and the high
    /// half is the load-bearing value for big-number multiplication;
    /// for `Fixed` operands the shifted result is a single word and
    /// the high slot is unused. Same stack effect as `CheckedAdd`.
    CheckedMul(u8),
    /// Overflow-checked Word negation. Pops one `Value::Int` and
    /// pushes three slots in the same shape: high, low, flag. The
    /// only overflow case is `-i64::MIN`, in which the high half
    /// is `0` and the low half is `i64::MIN` (the wrapped result).
    CheckedNeg,
    /// Overflow-checked division parameterized by a Q-format
    /// fraction-bit count (B35 P3d-iii). The operand is `0` for
    /// integer division and greater than zero for `Fixed` division,
    /// where the dividend is left-shifted by that many bits in the
    /// `i128` domain before dividing, so `0` fraction bits is exactly
    /// integer divide. A zero divisor reifies as flag `3`
    /// (zero_divisor) carrying the numerator; an unhandled zero
    /// divisor surfaces as `VmError::DivisionByZero`. For integer
    /// operands the only overflow case is `i64::MIN / -1`; for `Fixed`
    /// operands an out-of-range quotient wraps the single-word result.
    /// Same stack shape as the other `Op::Checked*` variants.
    CheckedDiv(u8),
    /// Overflow-checked Word modulo. Same stack shape. Division
    /// by zero traps. The only overflow case is `i64::MIN % -1`,
    /// whose mathematical result is `0` but whose computation
    /// overflows on the underlying `i64::MIN / -1`. The construct
    /// routes to the overflow arm with `high = 0`, `low = 0` in
    /// that case. All other inputs route through ok with `high =
    /// 0` and the wrapped remainder as `low`.
    CheckedMod,

    // -- V0.2.0 ISA additions (B20). Additive in Phase 1; compiler
    // -- emission and removal of legacy opcodes lands in later phases.
    /// Push an inline immediate value. Encoding:
    /// `0 = Unit`, `1 = true`, `2 = false`, `3 = None`,
    /// `4..19 = Int(operand - 4)`, `20..255 = reserved`.
    PushImmediate(u8),

    /// Pop `n` values from the top of the stack and discard them.
    /// Replaces single-slot `Op::Pop` and multi-slot pop sequences.
    /// `n = 0` is a no-op (admissible but redundant).
    PopN(u8),

    /// Bitwise AND of two `Value::Int` operands. Pops two, pushes one.
    BitAnd,
    /// Bitwise OR of two `Value::Int` operands. Pops two, pushes one.
    BitOr,
    /// Bitwise XOR of two `Value::Int` operands. Pops two, pushes one.
    BitXor,
    /// Logical shift-left of a `Value::Int` by a `Value::Int` count.
    /// Count is masked to the word width (`count & (word_bits - 1)`)
    /// so behavior is defined for all counts. Pops count then value;
    /// pushes the shifted value.
    Shl,
    /// Arithmetic right shift of a `Value::Int` by a `Value::Int`
    /// count (sign-preserving). Count is masked to the word width.
    /// Pops count then value; pushes the shifted value.
    Shr,

    /// Call a verified native function with attested WCET/WCMU
    /// bounds. Cost folds into the iteration's WCET/WCMU budget per
    /// host attestation. Emitted by the compiler for `use module::name`
    /// imports. The runtime cross-checks the registered native's
    /// classification at `Vm::new`; a native registered through
    /// `register_external_native` referenced here is rejected at
    /// load time.
    CallVerifiedNative(u16, u8),

    /// Call an external native function. Iteration cost budget
    /// pauses for the call duration; the verifier tracks invocation
    /// count per iteration instead of per-call cost. Emitted by the
    /// compiler for `use external module::name` imports. The runtime
    /// cross-checks the registered native's classification at
    /// `Vm::new`; a native registered through
    /// `register_verified_native` referenced here is rejected at
    /// load time.
    CallExternalNative(u16, u8),
}

/// Size in bytes of one operand-stack slot, namely the real
/// `size_of::<Value>()` of the bundled 64-bit runtime.
///
/// This is bound to the actual `core::mem::size_of::<GenericValue<i64,
/// f64>>()` rather than a hand-maintained literal, so it can never drift
/// from the runtime representation (a prior literal of 32 understated the
/// real 72-byte value, which under-reported every WCMU figure derived from
/// it). It auto-tracks any change to the value layout, including the B28
/// flat-model shrink that reduced the value to 40 bytes.
///
/// **Soundness as a nominal figure.** The compiler bakes WCMU figures into
/// representation-independent bytecode that may also run on a narrow
/// `GenericVm<W, A, F>` whose slot is smaller, so this bundled-runtime size
/// is a conservative upper bound for those narrower runtimes (their
/// `GenericValue` is no larger, because the dominant `FlatComposite` body
/// is not parameterised by the scalar widths). The binding admission check
/// in [`crate::vm::GenericVm::new`] still uses each runtime's own
/// `size_of::<GenericValue<W, F>>()`, so the per-runtime bound is exact;
/// this constant governs the compile-time advisory header and the nominal
/// cost model. Future work under B10 may parameterise it by target through
/// a [`CostModel`].
pub const VALUE_SLOT_SIZE_BYTES: u32 = core::mem::size_of::<Value>() as u32;

/// Context passed to an [`OpCost::Dynamic`] cost evaluator.
///
/// Carries the abstract-interpretation results that bear on the
/// opcode's cost. The WCMU text-size tracking pass populates the
/// `lhs_text_len` and `rhs_text_len` fields when evaluating the
/// heap-allocation cost of text-producing opcodes (`Op::Add` on
/// text, plus host-registered text-producing natives). Fields that
/// the analysis cannot bound are reported as `u32::MAX` (the
/// saturation value for the length lattice), which conservatively
/// propagates an "unbounded" verdict to the surrounding analysis.
///
/// Forward-looking. Populated stubbed-out in V0.2.0; the WCMU
/// text-size tracking pass in V0.2.x is the first consumer.
#[derive(Clone, Copy, Debug, Default)]
pub struct OpCostContext {
    /// Upper-bound length in bytes of the left text operand for
    /// text-producing opcodes. `u32::MAX` denotes unbounded.
    pub lhs_text_len: u32,
    /// Upper-bound length in bytes of the right text operand for
    /// text-producing opcodes. `u32::MAX` denotes unbounded.
    pub rhs_text_len: u32,
}

/// Cost of an opcode under a [`CostModel`].
///
/// `Fixed(n)` is the existing case where the cost is a compile-time
/// constant per opcode. For example, `Op::Add` on `i64` operands
/// always costs two pipelined cycles regardless of operand values.
///
/// `Dynamic(f)` is for operations whose cost depends on runtime
/// data. The concrete motivating case is the heap byte allocation
/// of `Op::Add` on text operands, where the resulting `KString`
/// length is the sum of the operand lengths. The WCMU pass
/// invokes the dynamic variant with an [`OpCostContext`] populated
/// from the abstract-interpretation results; the WCET pass
/// currently treats `Dynamic` as a sentinel forwarding to the
/// abstract-interpretation pass.
///
/// Hosts that supply a custom cost model may choose `Fixed` for
/// all opcodes if they prefer a simpler accounting model. The
/// abstract-interpretation pass falls back to a conservative
/// upper bound when a dynamic cost cannot be evaluated.
#[derive(Clone, Copy)]
pub enum OpCost {
    /// Cost is a compile-time constant per opcode.
    Fixed(u32),
    /// Cost depends on runtime data carried in [`OpCostContext`].
    Dynamic(fn(&OpCostContext) -> u32),
}

impl OpCost {
    /// Evaluate the cost against a context. `Fixed` returns the
    /// inner value directly; `Dynamic` invokes the function pointer
    /// against the supplied context.
    pub fn evaluate(&self, ctx: &OpCostContext) -> u32 {
        match self {
            OpCost::Fixed(n) => *n,
            OpCost::Dynamic(f) => f(ctx),
        }
    }
}

/// Per-target cost model used by the WCET and WCMU analyses.
///
/// Units. WCMU is reported in **bytes**. WCET is reported in
/// **pipelined cycles**. A pipelined cycle is a CPU cycle in which
/// the host's pipeline operates at steady-state throughput, assuming
/// warm instruction and data caches, correctly predicted branches,
/// and no contention on the memory bus. The pipelined-cycle metric
/// is what CPU optimization tables call "throughput" or "reciprocal
/// throughput" per instruction. It is observable through standard
/// benchmarking with warm caches and a stable predictor.
///
/// What the analysis bounds, and what it does not. The pipelined-
/// cycle bound is sound for the abstract metric. Actual cycles on
/// real hardware exceed the bound by the host's stall budget,
/// covering cache misses, branch mispredictions, and memory-bus
/// contention. Wall-clock time additionally depends on the clock
/// period and on frequency scaling. The conversion from pipelined-
/// cycle bound to wall-clock WCET is a platform-specific scalar,
/// conventionally called the calibration factor or dilation factor
/// in the WCET literature. The host establishes this factor during
/// deployment validation. For many practical applications, the
/// pipelined-cycle bound multiplied by a measured calibration factor
/// is an effective approximation of the worst-case wall-clock
/// execution time.
///
/// Custom cost models. Hosts construct a `CostModel` by setting
/// `value_slot_bytes` to the runtime's value-slot size and
/// `op_cycles` to a function pointer that returns the pipelined-cycle
/// cost for each opcode. The function pointer is reentrant and must
/// not allocate or fail. The convention is that the function
/// pattern-matches on the `Op` variant and returns the corresponding
/// cycle count from a target-specific table.
///
/// The bundled [`NOMINAL_COST_MODEL`] supplies unmeasured pipelined-
/// cycle estimates that the existing analysis APIs use when no
/// custom model is provided. The estimates are suitable for relative
/// ordering of programs on a single platform but are not validated
/// against any specific host CPU.
#[derive(Clone, Copy)]
pub struct CostModel {
    /// Bytes per operand-stack slot for the host runtime. Determines
    /// the conversion from slot count to byte count in the WCMU
    /// analysis. The current 64-bit Keleusma runtime uses 32 bytes
    /// per slot; a future 32-bit runtime would use a smaller value.
    pub value_slot_bytes: u32,

    /// Function returning the nominal cycle cost for the given
    /// opcode. The nominal cost model uses an unmeasured table whose
    /// values are relative weights rather than measured cycles.
    /// Hosts override this for measured per-target cycle tables.
    pub op_cycles: fn(&Op) -> u32,
}

impl CostModel {
    /// Compute the nominal cycle cost for the opcode under this
    /// cost model.
    pub fn cycles(&self, op: &Op) -> u32 {
        (self.op_cycles)(op)
    }

    /// Compute the WCMU byte cost of an operand-stack slot count
    /// under this cost model.
    pub fn slots_to_bytes(&self, slots: u32) -> u32 {
        slots.saturating_mul(self.value_slot_bytes)
    }

    /// Compute the heap byte allocation for the opcode under this
    /// cost model. For composite-construction opcodes, multiplies
    /// the field count by the cost model's `value_slot_bytes`.
    /// Text-producing opcodes (`Op::Add` on text) are reported via
    /// [`Self::heap_alloc_cost`] as [`OpCost::Dynamic`]; the
    /// fixed-cost view returned here saturates such cases to zero
    /// because the heap cost is not knowable without abstract
    /// interpretation. The WCMU pass that tracks text sizes must
    /// use [`Self::heap_alloc_cost`] instead.
    pub fn heap_alloc_bytes(&self, op: &Op, chunk: &Chunk) -> u32 {
        match self.heap_alloc_cost(op, chunk) {
            OpCost::Fixed(n) => n,
            OpCost::Dynamic(_) => 0,
        }
    }

    /// Compute the heap allocation cost for the opcode under this
    /// cost model as an [`OpCost`].
    ///
    /// Composite-construction opcodes (struct, enum, array, tuple)
    /// report `OpCost::Fixed` because their size is known at the
    /// opcode site. `Op::Add` on text operands reports
    /// `OpCost::Dynamic` because the allocated `KString` length is
    /// the sum of the operand lengths, which the verifier learns
    /// only through the abstract-interpretation text-size pass.
    pub fn heap_alloc_cost(&self, op: &Op, _chunk: &Chunk) -> OpCost {
        match op {
            // NewComposite carries its exact flat allocation size in the
            // operand (B28 P4), so the worst-case-memory-usage bound is the
            // precise byte count rather than a `count * VALUE_SLOT` estimate.
            // The boxed form reports zero flat bytes (its body is the heap
            // `Vec`, accounted separately).
            Op::NewComposite(op) => OpCost::Fixed(op.alloc_bytes()),
            Op::Add => OpCost::Dynamic(add_text_heap_alloc_bytes),
            _ => OpCost::Fixed(0),
        }
    }
}

/// Dynamic heap-allocation cost for `Op::Add` on text operands.
///
/// Returns the sum of the operand lengths saturated at `u32::MAX`.
/// The WCMU pass evaluates this against an [`OpCostContext`]
/// populated from the per-slot text-size lattice. When either
/// operand length is `u32::MAX` (unbounded), the result saturates
/// to `u32::MAX` so the outer analysis propagates an unbounded
/// verdict.
fn add_text_heap_alloc_bytes(ctx: &OpCostContext) -> u32 {
    ctx.lhs_text_len.saturating_add(ctx.rhs_text_len)
}

/// Default cost model for the bundled runtime. WCMU value-slot size
/// matches the runtime's `VALUE_SLOT_SIZE_BYTES`. WCET pipelined
/// cycles come from the unmeasured table provided by
/// [`nominal_op_cycles`].
///
/// **Pipelined-cycle caveat.** The bundled values are unmeasured
/// estimates chosen for relative ordering, not measured pipelined
/// cycles for any specific host CPU. The scale is one cycle for data
/// movement and trivial control flow, two for arithmetic and
/// comparison, three for division and field lookup, five for
/// composite construction, ten for function calls. A program whose
/// pipelined-cycle WCET exceeds another program's pipelined-cycle
/// WCET on the same platform is more expensive in the relative
/// sense. Hosts that need a wall-clock bound apply a platform-
/// specific calibration factor to convert pipelined cycles to actual
/// cycles and to wall-clock time. A measured-cycle CostModel
/// improves the approximation by replacing the bundled estimates
/// with measured pipelined cycles for the target CPU.
pub const NOMINAL_COST_MODEL: CostModel = CostModel {
    value_slot_bytes: VALUE_SLOT_SIZE_BYTES,
    op_cycles: nominal_op_cycles,
};

/// The pipelined-cycle cost table used by [`NOMINAL_COST_MODEL`].
/// Returns unmeasured pipelined-cycle estimates per the documented
/// scale. The values are intended to be replaced with measured
/// pipelined cycles during deployment validation.
pub fn nominal_op_cycles(op: &Op) -> u32 {
    match op {
        Op::Const(_)
        | Op::GetLocal(_)
        | Op::SetLocal(_)
        | Op::GetData(_)
        | Op::SetData(_)
        | Op::Dup
        | Op::Not => 1,

        Op::If(_)
        | Op::Else(_)
        | Op::EndIf
        | Op::Loop(_)
        | Op::EndLoop(_)
        | Op::Break(_)
        | Op::BreakIf(_)
        | Op::Stream
        | Op::Reset
        | Op::Yield
        | Op::Trap(_) => 1,

        Op::Add
        | Op::Sub
        | Op::CheckedAdd
        | Op::CheckedSub
        | Op::CheckedMul(_)
        | Op::CheckedNeg
        | Op::CheckedDiv(_)
        | Op::CheckedMod
        | Op::Mul
        | Op::Neg
        | Op::CmpEq
        | Op::CmpNe
        | Op::CmpLt
        | Op::CmpGt
        | Op::CmpLe
        | Op::CmpGe
        | Op::GetIndex(_)
        | Op::GetTupleField(_)
        | Op::GetEnumField(_)
        | Op::Len
        | Op::IntToFloat
        | Op::FloatToInt
        | Op::WordToByte
        | Op::ByteToWord
        | Op::WordToFixed(_)
        | Op::FixedToWord(_)
        | Op::FixedMul(_)
        | Op::FixedDiv(_)
        | Op::Return
        | Op::GetDataIndexed(_, _)
        | Op::SetDataIndexed(_, _)
        | Op::BoundsCheck(_) => 2,

        Op::Div | Op::Mod | Op::GetField(_) | Op::IsEnum(_, _, _) | Op::IsStruct(_) => 3,

        Op::NewComposite(_) => 5,

        Op::Call(_, _) => 10,

        // V0.2.0 ISA additions.
        Op::PushImmediate(_) | Op::PopN(_) => 1,
        Op::BitAnd | Op::BitOr | Op::BitXor | Op::Shl | Op::Shr => 2,
        Op::CallVerifiedNative(_, _) | Op::CallExternalNative(_, _) => 10,
    }
}

impl Op {
    /// Return the WCET cost of this instruction in **pipelined
    /// cycles** per the [`NOMINAL_COST_MODEL`].
    ///
    /// **Unit.** The result is a count of pipelined cycles. A
    /// pipelined cycle is a CPU cycle in which the host's pipeline
    /// operates at steady-state throughput, assuming warm caches,
    /// correctly predicted branches, and no memory-bus contention.
    /// The bundled values are unmeasured estimates chosen for
    /// relative ordering of programs on a single platform. The scale
    /// is one cycle for data movement and trivial control flow, two
    /// for arithmetic and comparison, three for division and field
    /// lookup, five for composite construction, ten for function
    /// calls. The values are not validated against any specific host
    /// CPU. Hosts that need wall-clock WCET apply a platform-specific
    /// calibration factor to the pipelined-cycle bound, or construct
    /// a custom [`CostModel`] whose `op_cycles` returns measured
    /// pipelined cycles for the target hardware.
    ///
    /// This method is a thin wrapper over [`NOMINAL_COST_MODEL`].
    /// Analysis APIs that take an explicit `&CostModel` parameter
    /// allow per-target cost tables to flow through without changing
    /// the rest of the analysis.
    pub fn cost(&self) -> u32 {
        NOMINAL_COST_MODEL.cycles(self)
    }

    /// Number of operand-stack slots pushed by this instruction.
    ///
    /// This is the maximum the operand stack can grow during execution of
    /// this single instruction relative to its starting depth. Used by the
    /// WCMU analysis to compute peak stack consumption.
    pub fn stack_growth(&self) -> u32 {
        match self {
            Op::Const(_) | Op::GetLocal(_) | Op::GetData(_) | Op::Dup => 1,

            Op::Not | Op::Neg => 0,

            // CheckedAdd / CheckedSub / CheckedMul / CheckedDiv /
            // CheckedMod pop two operands and push (high, low,
            // flag); net delta +1. CheckedNeg pops one and pushes
            // three; net delta +2. The high half is the i128
            // intermediate's high 64 bits, providing the load-
            // bearing value for big-number multiplication.
            Op::CheckedAdd
            | Op::CheckedSub
            | Op::CheckedMul(_)
            | Op::CheckedDiv(_)
            | Op::CheckedMod => 1,
            Op::CheckedNeg => 2,

            Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Mod
            | Op::CmpEq
            | Op::CmpNe
            | Op::CmpLt
            | Op::CmpGt
            | Op::CmpLe
            | Op::CmpGe => 0,

            Op::SetLocal(_) | Op::SetData(_) => 0,

            // GetDataIndexed pops one index, pushes one value.
            Op::GetDataIndexed(_, _) => 1,
            // SetDataIndexed pops index and value.
            Op::SetDataIndexed(_, _) => 0,
            // BoundsCheck does not change the stack.
            Op::BoundsCheck(_) => 0,

            Op::If(_) | Op::BreakIf(_) => 0,
            Op::Else(_) | Op::EndIf | Op::Loop(_) | Op::EndLoop(_) | Op::Break(_) => 0,
            Op::Stream | Op::Reset => 0,
            Op::Yield => 0,

            Op::Call(_, _) => 1,
            Op::Return => 0,

            Op::NewComposite(_) => 1,

            Op::GetField(_)
            | Op::GetIndex(_)
            | Op::GetTupleField(_)
            | Op::GetEnumField(_)
            | Op::Len => 0,

            Op::IsEnum(_, _, _) | Op::IsStruct(_) => 0,

            Op::IntToFloat
            | Op::FloatToInt
            | Op::WordToByte
            | Op::ByteToWord
            | Op::WordToFixed(_)
            | Op::FixedToWord(_) => 0,
            Op::FixedMul(_) | Op::FixedDiv(_) => 0,

            Op::Trap(_) => 0,

            // V0.2.0 ISA additions.
            Op::PushImmediate(_) => 1,
            Op::PopN(_) => 0,
            Op::BitAnd | Op::BitOr | Op::BitXor | Op::Shl | Op::Shr => 0,
            // A native call pushes one result, or two slots
            // `(code, flag)` when the error-reify flag (high bit of
            // the argument-count byte, B35 P7) is set.
            Op::CallVerifiedNative(_, n) | Op::CallExternalNative(_, n) => {
                if n & 0x80 != 0 {
                    2
                } else {
                    1
                }
            }
        }
    }

    /// Number of operand-stack slots popped by this instruction.
    pub fn stack_shrink(&self) -> u32 {
        match self {
            Op::Const(_) | Op::GetLocal(_) | Op::GetData(_) | Op::Dup => 0,

            Op::Not | Op::Neg => 0,

            // CheckedAdd / CheckedSub / CheckedMul / CheckedDiv /
            // CheckedMod net +1 (pop 2, push 3). CheckedNeg net +2
            // (pop 1, push 3). The growth/shrink split records
            // peak vs. final; shrink is zero because there is no
            // net pop.
            Op::CheckedAdd
            | Op::CheckedSub
            | Op::CheckedMul(_)
            | Op::CheckedNeg
            | Op::CheckedDiv(_)
            | Op::CheckedMod => 0,

            Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Mod
            | Op::CmpEq
            | Op::CmpNe
            | Op::CmpLt
            | Op::CmpGt
            | Op::CmpLe
            | Op::CmpGe => 1,

            Op::SetLocal(_) | Op::SetData(_) => 1,

            // GetDataIndexed pops the index, SetDataIndexed pops the
            // index then the value, BoundsCheck does not pop.
            Op::GetDataIndexed(_, _) => 1,
            Op::SetDataIndexed(_, _) => 2,
            Op::BoundsCheck(_) => 0,

            Op::If(_) | Op::BreakIf(_) => 1,
            Op::Else(_) | Op::EndIf | Op::Loop(_) | Op::EndLoop(_) | Op::Break(_) => 0,
            Op::Stream | Op::Reset => 0,
            Op::Yield => 1,

            Op::Call(_, n) => *n as u32,
            Op::Return => 0,

            // NewComposite pops `count` values (an enum's leading
            // discriminant counts as one) (B28 P4).
            Op::NewComposite(c) => c.count() as u32,

            Op::GetField(_) | Op::GetIndex(_) | Op::GetTupleField(_) | Op::GetEnumField(_) => 1,
            Op::Len => 0,

            Op::IsEnum(_, _, _) | Op::IsStruct(_) => 0,

            Op::IntToFloat
            | Op::FloatToInt
            | Op::WordToByte
            | Op::ByteToWord
            | Op::WordToFixed(_)
            | Op::FixedToWord(_) => 0,
            Op::FixedMul(_) | Op::FixedDiv(_) => 0,

            Op::Trap(_) => 0,

            // V0.2.0 ISA additions.
            Op::PushImmediate(_) => 0,
            Op::PopN(n) => *n as u32,
            // Bit ops pop 2, push 1; net shrink = 1 in the same
            // convention as `Add` etc.
            Op::BitAnd | Op::BitOr | Op::BitXor | Op::Shl | Op::Shr => 1,
            // Pop the argument count; the high bit is the error-reify
            // flag (B35 P7), not part of the count.
            Op::CallVerifiedNative(_, n) | Op::CallExternalNative(_, n) => (*n & 0x7F) as u32,
        }
    }

    /// WCMU heap allocation by this instruction in **bytes** under
    /// the [`NOMINAL_COST_MODEL`].
    ///
    /// **Unit.** The result is a count of bytes. The byte count is
    /// computed as the field-slot count multiplied by the cost
    /// model's `value_slot_bytes`. The slot count is target-
    /// independent (a structural property of the opcode); the byte
    /// conversion depends on the runtime's value representation.
    ///
    /// For composite-construction instructions, the size is the count
    /// of stored field slots times `value_slot_bytes`. For
    /// `NewStruct`, the field count comes from the chunk's struct
    /// templates and is looked up through the provided `chunk`
    /// reference.
    ///
    /// Calls and native calls report zero local heap. The transitive
    /// heap contribution of a `Call` is the WCMU of the called
    /// function and is computed at the analysis level. The heap
    /// contribution of a `CallNative` comes from the host's WCMU
    /// attestation recorded against the native function entry.
    ///
    /// This method is a thin wrapper over
    /// [`CostModel::heap_alloc_bytes`] using [`NOMINAL_COST_MODEL`].
    /// Analysis APIs that take an explicit `&CostModel` allow
    /// per-target value-slot sizes to flow through without changing
    /// the rest of the analysis.
    pub fn heap_alloc(&self, chunk: &Chunk) -> u32 {
        NOMINAL_COST_MODEL.heap_alloc_bytes(self, chunk)
    }
}

/// Template for struct construction.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct StructTemplate {
    /// Struct type name.
    pub type_name: String,
    /// Field names in order.
    pub field_names: Vec<String>,
}

/// A named slot in the data segment.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct DataSlot {
    /// Slot name (for host initialization and debugging).
    pub name: String,
    /// Slot visibility to the host. Shared slots are accessible
    /// through `Vm::set_data` and `Vm::get_data`. Private slots
    /// are script-only; the host API rejects access. Both
    /// persist across resets. Source declaration uses the
    /// `shared` (default) and `private` modifiers on `data`
    /// blocks.
    pub visibility: SlotVisibility,
}

/// Slot visibility flag carried in [`DataSlot::visibility`].
///
/// Mirrors `ast::DataVisibility` at the bytecode layer so the
/// runtime can enforce the host-API boundary without reading
/// the source AST. Serialized as part of the data layout in the
/// bytecode body; it is not part of the framing header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum SlotVisibility {
    /// Host-visible slot. The default. `Vm::set_data` and
    /// `Vm::get_data` admit this slot.
    Shared,
    /// Script-only slot. The host API rejects this slot.
    Private,
}

/// Data segment layout declaration.
///
/// Defines the fixed-size, fixed-layout set of persistent values that
/// survive across RESET boundaries. The host initializes data slots
/// before execution begins. Scripts read and write slots by index.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct DataLayout {
    /// Named slots in declaration order. Slot index corresponds to
    /// the `GetData`/`SetData` operand.
    pub slots: Vec<DataSlot>,
}

/// A compiled function.
///
/// V0.2.0 Phase 7c moved the on-the-wire representation to
/// [`crate::wire_format::WireChunk`], which carries the same
/// per-chunk metadata minus the ops (which live in the opcode
/// stream section). `Chunk` is the in-memory representation;
/// the rkyv derives retire in Phase 8.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Function name (for debugging and lookup).
    pub name: String,
    /// Bytecode instructions.
    pub ops: Vec<Op>,
    /// Constant pool. Stores compile-time constants only.
    pub constants: Vec<ConstValue>,
    /// Struct field layout templates.
    pub struct_templates: Vec<StructTemplate>,
    /// Total local variable slots (including parameters).
    pub local_count: u16,
    /// Number of parameters.
    pub param_count: u8,
    /// Block type classification for structural verification.
    pub block_type: BlockType,
    /// Parameter type tags, one per parameter. Used by
    /// `Vm::call` to reject ill-typed arguments before any
    /// bytecode runs. Composite types (struct, enum, tuple,
    /// array, option, opaque) record [`TypeTag::Composite`]
    /// which the runtime accepts without further checking.
    /// For Stream chunks, the single entry also serves as the
    /// resume value's type (see [`crate::vm::Vm::resume`]).
    pub param_types: Vec<TypeTag>,
    /// Optional strippable debug metadata (B29). `None` for a release
    /// build or a stripped artefact; `Some` when the chunk carries
    /// development aids such as source spans and variable names. The
    /// debug pool is held entirely here and never in `ops`, so the
    /// opcode sequence is byte-identical whether or not the pool is
    /// present. See [`crate::debug_meta`].
    pub debug_pool: Option<crate::debug_meta::DebugPool>,
}

/// Compact representation of a primitive parameter type for
/// runtime call validation. Composite types (struct, enum,
/// tuple, array, option, opaque, function values) collapse to
/// [`TypeTag::Composite`]; the runtime accepts any non-primitive
/// `Value` for a `Composite` parameter without further checking.
///
/// Fixed-point types record only the canonical tag and not the
/// fraction-bit count; the type checker has already enforced
/// fraction-bit compatibility at compile time, so the runtime
/// only needs to confirm the operand is `Value::Fixed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum TypeTag {
    /// Non-primitive type. The runtime does not check shape; any
    /// `Value` is accepted.
    Composite,
    /// Eight-bit unsigned integer. Accepts `Value::Byte`.
    Byte,
    /// Target-word signed integer. Accepts `Value::Int`.
    Word,
    /// Signed Q-format fixed-point. Accepts `Value::Fixed`.
    Fixed,
    /// Target-float. Accepts `Value::Float`.
    Float,
    /// Boolean. Accepts `Value::Bool`.
    Bool,
    /// Unit `()`. Accepts `Value::Unit`.
    Unit,
    /// UTF-8 text. Accepts `Value::StaticStr` or `Value::KStr`.
    Text,
}

impl TypeTag {
    /// Lift an [`ArchivedTypeTag`] into a [`TypeTag`]. The archive
    /// form is a unit-variant enum with the same discriminant
    /// layout, so the lift is a one-to-one match.
    pub fn from_archived(archived: &ArchivedTypeTag) -> Self {
        match archived {
            ArchivedTypeTag::Composite => TypeTag::Composite,
            ArchivedTypeTag::Byte => TypeTag::Byte,
            ArchivedTypeTag::Word => TypeTag::Word,
            ArchivedTypeTag::Fixed => TypeTag::Fixed,
            ArchivedTypeTag::Float => TypeTag::Float,
            ArchivedTypeTag::Bool => TypeTag::Bool,
            ArchivedTypeTag::Unit => TypeTag::Unit,
            ArchivedTypeTag::Text => TypeTag::Text,
        }
    }

    /// Returns `true` if `value` is admissible for a parameter
    /// declared with this tag. Generic over the parametric value
    /// type so the bundled `Vm<i64, u64, f64>` and a host-
    /// instantiated narrower `Vm<W, A, F>` share the same check.
    pub fn admits<W: crate::word::Word, F: crate::float::Float>(
        &self,
        value: &GenericValue<W, F>,
    ) -> bool {
        match self {
            TypeTag::Composite => true,
            TypeTag::Byte => matches!(value, GenericValue::Byte(_)),
            TypeTag::Word => matches!(value, GenericValue::Int(_)),
            TypeTag::Fixed => matches!(value, GenericValue::Fixed(_)),
            #[cfg(feature = "floats")]
            TypeTag::Float => matches!(value, GenericValue::Float(_)),
            #[cfg(not(feature = "floats"))]
            TypeTag::Float => false,
            TypeTag::Bool => matches!(value, GenericValue::Bool(_)),
            TypeTag::Unit => matches!(value, GenericValue::Unit),
            TypeTag::Text => {
                matches!(value, GenericValue::StaticStr(_) | GenericValue::KStr(_))
            }
        }
    }

    /// Human-readable name for the tag, suitable for error
    /// messages.
    pub fn name(&self) -> &'static str {
        match self {
            TypeTag::Composite => "Composite",
            TypeTag::Byte => "Byte",
            TypeTag::Word => "Word",
            TypeTag::Fixed => "Fixed",
            TypeTag::Float => "Float",
            TypeTag::Bool => "Bool",
            TypeTag::Unit => "Unit",
            TypeTag::Text => "Text",
        }
    }
}

/// A compiled Keleusma module.
///
/// V0.2.0 Phase 7c cut the on-the-wire serialization over to
/// the section-partitioned wire format defined in
/// [`crate::wire_format`]; the rkyv archive of the full
/// `Module` is no longer produced or consumed. `Module` is the
/// in-memory representation; serialization flows through
/// `Module::to_bytes` -> `module_to_wire_bytes` and
/// deserialization through `module_from_wire_bytes` ->
/// `Module`. The Phase 8 publication readiness pass drops the
/// rkyv derives.
#[derive(Debug, Clone)]
pub struct Module {
    /// Compiled function chunks.
    pub chunks: Vec<Chunk>,
    /// Declared native function names (from `use` declarations).
    pub native_names: Vec<String>,
    /// Entry point chunk index (the `main` function).
    pub entry_point: Option<usize>,
    /// Data segment layout. If present, defines persistent slots that
    /// survive across RESET boundaries.
    pub data_layout: Option<DataLayout>,
    /// Word size required by this bytecode, encoded as the base-2
    /// exponent. Actual width in bits is `1 << word_bits_log2`. The
    /// runtime accepts the bytecode when the recorded value is at most
    /// the runtime's `RUNTIME_WORD_BITS_LOG2`. The VM masks integer
    /// arithmetic to the declared width using sign-extending shift.
    /// Mirrored in the framing header for fast pre-decode rejection.
    pub word_bits_log2: u8,
    /// Address size required by this bytecode, encoded as the base-2
    /// exponent. Actual width in bits is `1 << addr_bits_log2`. The
    /// runtime accepts the bytecode when the recorded value is at most
    /// the runtime's `RUNTIME_ADDRESS_BITS_LOG2`. Mirrored in the
    /// framing header for fast pre-decode rejection.
    pub addr_bits_log2: u8,
    /// Floating-point width required by this bytecode, encoded as the
    /// base-2 exponent. Actual width in bits is `1 << float_bits_log2`.
    /// The runtime accepts the bytecode when the recorded value is at
    /// most the runtime's `RUNTIME_FLOAT_BITS_LOG2`. The current
    /// runtime uses f64 exclusively (exponent 6); narrower or wider
    /// floats are reserved for future portability work tracked under
    /// B10. Mirrored in the framing header for fast pre-decode
    /// rejection.
    pub float_bits_log2: u8,
    /// Declared worst-case execution time per Stream-to-Reset slice,
    /// in pipelined cycles. Producer's claim about the maximum cycles
    /// the script consumes between two yield boundaries.
    ///
    /// - `0` means **auto**: the producer did not declare a value;
    ///   the runtime computes the bound at load time through its own
    ///   verifier pass.
    /// - `u32::MAX` means **overflow**: the producer attempted to
    ///   compute the bound but the result exceeds the field's range.
    ///   Programs declaring `u32::MAX` are rejected at the safe
    ///   constructor `Vm::new` because no representable bound exists.
    /// - Any other value is the producer's bound. The safe runtime
    ///   accepts the value as-is; trust skip applies to declared
    ///   values just as it does to arena capacity.
    ///
    /// Mirrored in the framing header for inspection without body
    /// decode.
    pub wcet_cycles: u32,
    /// Declared worst-case memory usage per Stream-to-Reset slice,
    /// in bytes. Same `0`/`u32::MAX` conventions as
    /// [`Module::wcet_cycles`]. Total of stack and heap regions.
    /// Mirrored in the framing header.
    pub wcmu_bytes: u32,
    /// Worst-case bytes the runtime needs for its own ephemeral
    /// tracking structures per Stream-to-Reset slice, beyond the
    /// script-value WCMU (B28 P3 item 5, Phase C).
    ///
    /// These are the runtime's per-instance bookkeeping lists — the
    /// opaque registry, and (as the relocation lands) the backing of
    /// boxed composite bodies — which the runtime allocates inside the
    /// arena (the top ephemeral region) and pre-sizes once, as the
    /// first allocations after each RESET, rather than growing during
    /// an iteration. The runtime reads this value to pre-size those
    /// lists, and `auto_arena_capacity_for` adds it to the arena size.
    /// It is a runtime-only figure: native code never observes it, and
    /// it is distinct from the native-WCMU attestation path. `0` means
    /// the module needs no such tracking memory. Carried in the framing
    /// header's reserved word at offset 56.
    pub aux_arena_bytes: u32,
    /// Bit flags describing static properties of the module.
    /// Currently defined bits.
    ///
    /// - `0x01` (`FLAG_EPHEMERAL`). The module is provably
    ///   ephemeral: at every yield or return that crosses the
    ///   host-VM boundary, no arena-resident value is observed,
    ///   and at every resume or entry no value loaded from arena
    ///   memory allocated prior to that resume or entry is read.
    ///   Hosts that observe this bit may reuse a single arena
    ///   across many modules of this kind, sized to the largest
    ///   module's WCMU.
    ///
    /// Unused bits are reserved for future declarations and must
    /// be zero. The runtime treats any unrecognised bits as
    /// reserved and ignores them.
    ///
    /// Mirrored in the framing header.
    pub flags: u8,
    /// Bytes of shared data declared by this module. Shared
    /// data lives in the Vm's owned slot storage and is
    /// host-visible through `Vm::set_data` and `Vm::get_data`.
    /// Survives RESET. Mirrored in the framing header.
    pub shared_data_bytes: u32,
    /// Bytes of private data declared by this module. Private
    /// data lives in the arena's persistent (`.data`) region
    /// and is not exposed through the host API. Survives
    /// RESET. The host sizes its arena's persistent capacity to
    /// match this value before loading the module. Mirrored in
    /// the framing header.
    pub private_data_bytes: u32,
    /// CRC-32 hash of the data-segment layout. Used by
    /// [`crate::vm::Vm::replace_module`] to reject hot swaps
    /// against incompatible schemas before any data is loaded.
    /// Computed from a canonical serialisation of each slot's
    /// name and visibility in declaration order; see
    /// [`compute_schema_hash`] for the exact byte sequence. A
    /// module with no data layout reports zero. The check is
    /// strict by default; hosts that need to swap across
    /// incompatible schemas (different data declaration, same
    /// arena capacity) call
    /// [`crate::vm::Vm::replace_module_unchecked`] to bypass it.
    pub schema_hash: u32,
}

/// Bit flags defined for [`Module::flags`].
///
/// See [`Module::flags`] for the semantic description of each bit.
/// Unused bits are reserved.
pub const FLAG_EPHEMERAL: u8 = 0x01;

/// Magic prefix identifying serialized Keleusma bytecode (`KELE`).
pub const BYTECODE_MAGIC: [u8; 4] = *b"KELE";

/// Wire format version for serialized bytecode. Bytecode produced under a
/// different version is rejected at load time.
///
/// V0.2 development releases briefly used version 2 before this crate
/// achieved public adoption; the version was rolled back to 1 when the
/// header was extended with the flags byte and the shared and private
/// data byte counts. Bytecode produced under any earlier development
/// build is rejected at load time on header-shape mismatch through the
/// CRC trailer.
pub const BYTECODE_VERSION: u16 = 1;

/// Word size in bits assumed by this binary build, encoded as the
/// base-2 exponent. Actual width in bits is `1 << RUNTIME_WORD_BITS_LOG2`.
/// Default value is `6` (64-bit words). The `narrow-word-8`,
/// `narrow-word-16`, and `narrow-word-32` Cargo features lower the
/// value to `3`, `4`, and `5` respectively, narrowing the framing-level
/// upper bound on bytecode this binary admits. The narrowest enabled
/// feature wins, preserving Cargo's additive-features semantics. See
/// B16 step 12 in `docs/decisions/BACKLOG.md` for the rationale.
#[cfg(feature = "narrow-word-8")]
pub const RUNTIME_WORD_BITS_LOG2: u8 = 3;
/// Word size in bits assumed by this binary build (log2 form).
#[cfg(all(feature = "narrow-word-16", not(feature = "narrow-word-8")))]
pub const RUNTIME_WORD_BITS_LOG2: u8 = 4;
/// Word size in bits assumed by this binary build (log2 form).
#[cfg(all(
    feature = "narrow-word-32",
    not(any(feature = "narrow-word-8", feature = "narrow-word-16"))
))]
pub const RUNTIME_WORD_BITS_LOG2: u8 = 5;
/// Word size in bits assumed by this binary build (log2 form).
#[cfg(not(any(
    feature = "narrow-word-8",
    feature = "narrow-word-16",
    feature = "narrow-word-32"
)))]
pub const RUNTIME_WORD_BITS_LOG2: u8 = 6;

/// Address size in bits assumed by this binary build, encoded as the
/// base-2 exponent. Actual width in bits is
/// `1 << RUNTIME_ADDRESS_BITS_LOG2`. Default value is `6` (64-bit
/// addresses). The `narrow-address-8`, `narrow-address-16`, and
/// `narrow-address-32` Cargo features lower the value following the
/// same narrowest-wins rule as `RUNTIME_WORD_BITS_LOG2`.
#[cfg(feature = "narrow-address-8")]
pub const RUNTIME_ADDRESS_BITS_LOG2: u8 = 3;
/// Address size in bits assumed by this binary build (log2 form).
#[cfg(all(feature = "narrow-address-16", not(feature = "narrow-address-8")))]
pub const RUNTIME_ADDRESS_BITS_LOG2: u8 = 4;
/// Address size in bits assumed by this binary build (log2 form).
#[cfg(all(
    feature = "narrow-address-32",
    not(any(feature = "narrow-address-8", feature = "narrow-address-16"))
))]
pub const RUNTIME_ADDRESS_BITS_LOG2: u8 = 5;
/// Address size in bits assumed by this binary build (log2 form).
#[cfg(not(any(
    feature = "narrow-address-8",
    feature = "narrow-address-16",
    feature = "narrow-address-32"
)))]
pub const RUNTIME_ADDRESS_BITS_LOG2: u8 = 6;

/// Floating-point width in bits assumed by this binary build,
/// encoded as the base-2 exponent. Actual width in bits is
/// `1 << RUNTIME_FLOAT_BITS_LOG2`. Default value is `6` (f64). The
/// `narrow-float-32` Cargo feature lowers the value to `5`,
/// rejecting f64 bytecode at the framing level.
#[cfg(feature = "narrow-float-32")]
pub const RUNTIME_FLOAT_BITS_LOG2: u8 = 5;
/// Floating-point width in bits assumed by this binary build (log2 form).
#[cfg(not(feature = "narrow-float-32"))]
pub const RUNTIME_FLOAT_BITS_LOG2: u8 = 6;

/// Header length in bytes. The fields are
///
/// - bytes 0..4: magic (`KELE`)
/// - bytes 4..6: version (u16 little-endian)
/// - bytes 6..10: total framing length (u32 little-endian, includes
///   header and CRC trailer)
/// - bytes 10..11: word_bits_log2 (u8). Actual width is `1 << value`.
/// - bytes 11..12: addr_bits_log2 (u8). Actual width is `1 << value`.
/// - bytes 12..13: float_bits_log2 (u8). Actual width is `1 << value`.
/// - bytes 13..14: flags (u8). Bit 0 is `FLAG_EPHEMERAL`. Other
///   bits reserved and must be zero.
/// - bytes 14..16: reserved (zero), preserved for backward layout.
/// - bytes 16..20: declared WCET in pipelined cycles per Stream-to-Reset
///   slice (u32 little-endian). `0` means auto (runtime computes).
///   `u32::MAX` means overflow (rejected at safe `Vm::new`).
/// - bytes 20..24: declared WCMU in bytes per Stream-to-Reset slice
///   (u32 little-endian). Same `0`/`u32::MAX` conventions.
/// - bytes 24..28: shared data bytes (u32 little-endian).
/// - bytes 28..32: private data bytes (u32 little-endian).
///
/// Reflected polynomial for the standard CRC-32 (IEEE 802.3, gzip, PNG,
/// ZIP). Reflected form of 0x04C11DB7. Paired with init 0xFFFFFFFF,
/// refin/refout true, and xor-out 0xFFFFFFFF. The V0.2.0 wire format
/// uses the residue self-inclusion property to verify integrity in a
/// single pass over the framed buffer.
const CRC32_POLY: u32 = 0xEDB88320;

/// CRC-32 of the data-segment layout's canonical byte serialisation.
///
/// Canonical form: for each slot in declaration order, emit
///
/// - the slot name's UTF-8 bytes,
/// - a single null byte `0x00` as separator,
/// - one byte for the visibility tag (`0x53` `'S'` for Shared,
///   `0x50` `'P'` for Private),
/// - a single newline `0x0A` as slot terminator.
///
/// The trailing newline keeps adjacent slots disambiguated when
/// one slot's name is a prefix of the next. A module with no
/// data layout returns 0.
///
/// The hash is computed at compile time and stored in
/// [`Module::schema_hash`]; [`crate::vm::Vm::replace_module`]
/// compares the values across a hot swap. The hash covers slot
/// names and visibility but not per-slot type tags; the layout
/// does not carry per-slot type information at the bytecode
/// level. Type-level checks remain a future extension.
pub fn compute_schema_hash(layout: Option<&DataLayout>) -> u32 {
    let layout = match layout {
        Some(l) => l,
        None => return 0,
    };
    if layout.slots.is_empty() {
        return 0;
    }
    let mut buf: Vec<u8> = Vec::new();
    for slot in &layout.slots {
        buf.extend_from_slice(slot.name.as_bytes());
        buf.push(0x00);
        let vis_tag = match slot.visibility {
            SlotVisibility::Shared => b'S',
            SlotVisibility::Private => b'P',
        };
        buf.push(vis_tag);
        buf.push(b'\n');
    }
    crc32(&buf)
}

pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ CRC32_POLY
            } else {
                crc >> 1
            };
        }
    }
    crc ^ 0xFFFFFFFF
}

/// A failure encountered while loading or saving precompiled bytecode.
///
/// Returned by [`Module::to_bytes`] and [`Module::from_bytes`]. The runtime
/// converts this into [`crate::vm::VmError::LoadError`] when used through
/// [`crate::vm::Vm::load_bytes`] and the related convenience constructors.
#[derive(Debug, Clone)]
pub enum LoadError {
    /// The header magic bytes did not match `KELE`.
    BadMagic,
    /// The buffer was shorter than the required header plus footer, or
    /// the recorded length field exceeds the slice length, or the
    /// recorded length is below the minimum framing size.
    Truncated,
    /// The bytecode version is not supported by this runtime.
    UnsupportedVersion {
        /// Version recorded in the bytecode header.
        got: u16,
        /// Version the runtime supports.
        expected: u16,
    },
    /// The recorded word size exponent exceeds what this runtime build
    /// supports. Values are log-base-2 exponents. The bytecode is
    /// admitted when `got <= max_supported`.
    WordSizeMismatch {
        /// Word size exponent recorded in the bytecode header.
        got: u8,
        /// Maximum word size exponent this runtime build supports.
        max_supported: u8,
    },
    /// The recorded address size exponent exceeds what this runtime
    /// build supports. Values are log-base-2 exponents. The bytecode is
    /// admitted when `got <= max_supported`.
    AddressSizeMismatch {
        /// Address size exponent recorded in the bytecode header.
        got: u8,
        /// Maximum address size exponent this runtime build supports.
        max_supported: u8,
    },
    /// The recorded floating-point width exponent exceeds what this
    /// runtime build supports. Values are log-base-2 exponents. The
    /// bytecode is admitted when `got <= max_supported`.
    FloatSizeMismatch {
        /// Float width exponent recorded in the bytecode header.
        got: u8,
        /// Maximum float width exponent this runtime build supports.
        max_supported: u8,
    },
    /// The CRC-32 trailer did not satisfy the algebraic self-inclusion
    /// residue. The bytecode is corrupted or was produced by a different
    /// CRC implementation.
    BadChecksum,
    /// The declared WCET in the framing header is `u32::MAX`, signaling
    /// that the producer attempted to compute a bound but the result
    /// exceeded the field's range. No representable bound exists, so
    /// safe loading is refused.
    WcetOverflow,
    /// The declared WCMU in the framing header is `u32::MAX`, signaling
    /// that the producer attempted to compute a bound but the result
    /// exceeded the field's range. No representable bound exists, so
    /// safe loading is refused.
    WcmuOverflow,
    /// The body could not be encoded or decoded.
    Codec(String),
    /// The bytecode's framing header carries `FLAG_REQUIRES_SIGNATURE`
    /// but no key in the host's trust matrix verifies the attached
    /// signature, or the signed-extension metadata is inconsistent.
    /// Hosts respond by either refusing the module or registering an
    /// additional [`crate::vm::Vm::register_verifying_key`] entry.
    InvalidSignature,
    /// The bytecode is signed but the runtime build does not include
    /// the `signatures` cargo feature. The host has no way to verify
    /// the signature, so loading is refused at framing time.
    SignaturesUnsupported,
}

impl core::fmt::Display for LoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoadError::BadMagic => f.write_str("bytecode header missing magic 'KELE'"),
            LoadError::Truncated => f.write_str(
                "bytecode truncated, recorded length exceeds slice, or below minimum framing",
            ),
            LoadError::UnsupportedVersion { got, expected } => {
                write!(
                    f,
                    "bytecode version {} not supported, expected {}",
                    got, expected
                )
            }
            LoadError::WordSizeMismatch { got, max_supported } => {
                write!(
                    f,
                    "bytecode requires {}-bit words, runtime supports up to {}-bit",
                    1u32 << got,
                    1u32 << max_supported
                )
            }
            LoadError::AddressSizeMismatch { got, max_supported } => {
                write!(
                    f,
                    "bytecode requires {}-bit addresses, runtime supports up to {}-bit",
                    1u32 << got,
                    1u32 << max_supported
                )
            }
            LoadError::FloatSizeMismatch { got, max_supported } => {
                write!(
                    f,
                    "bytecode requires {}-bit floats, runtime supports up to {}-bit",
                    1u32 << got,
                    1u32 << max_supported
                )
            }
            LoadError::BadChecksum => f.write_str("bytecode CRC-32 residue check failed"),
            LoadError::WcetOverflow => {
                f.write_str("declared WCET is u32::MAX (overflow); no representable bound")
            }
            LoadError::WcmuOverflow => {
                f.write_str("declared WCMU is u32::MAX (overflow); no representable bound")
            }
            LoadError::Codec(msg) => write!(f, "bytecode codec error: {}", msg),
            LoadError::InvalidSignature => {
                f.write_str("bytecode signature did not verify against any registered key")
            }
            LoadError::SignaturesUnsupported => f.write_str(
                "bytecode is signed but the runtime build does not include the `signatures` feature",
            ),
        }
    }
}

impl core::error::Error for LoadError {}

impl Module {
    /// Serialize the module to a self-describing byte vector.
    ///
    /// The output begins with the twelve-byte header (magic, version,
    /// total length, word size, address size), then the module body in
    /// postcard wire format, then a four-byte little-endian CRC-32
    /// trailer. The CRC covers the entire framed range. The algebraic
    /// self-inclusion residue of the CRC parameterization makes the
    /// trailer part of the checksummed range.
    ///
    /// All multi-byte integer fields in the framing are stored in
    /// little-endian order. Postcard stores its own multi-byte values in
    /// little-endian or as varints. The wire format is therefore
    /// identical bytes regardless of producer or consumer host
    /// endianness.
    ///
    /// Returns [`LoadError::Codec`] if postcard rejects any field. The
    /// `Module` type is composed entirely of types that postcard supports,
    /// so encode failures are not expected in practice and indicate
    /// corruption of the runtime data.
    pub fn to_bytes(&self) -> Result<Vec<u8>, LoadError> {
        // V0.2.0 Phase 7c cuts the producer over to the section-
        // partitioned wire format defined in `wire_format.rs`. The
        // ops live in the opcode stream and the operand pool;
        // every other Module field is rkyv-archived in the
        // auxiliary body section. See `docs/architecture/WIRE_FORMAT.md`
        // for the framing-header layout and the section semantics.
        crate::wire_format::module_to_wire_bytes(self)
    }

    /// Deserialize a module from a self-describing byte slice.
    ///
    /// Validation order is truncation, magic, length, CRC residue,
    /// version, word size, address size, and body decode. The slice is
    /// truncated to the recorded length before the CRC check so that
    /// bytecode embedded in a larger buffer is supported. Trailing
    /// bytes after the recorded length are ignored.
    ///
    /// The CRC is checked before the version, word size, and address
    /// size because a corrupted byte in any of those fields would
    /// otherwise be reported as a mismatch rather than the more
    /// accurate `BadChecksum`.
    ///
    /// Does not run structural verification or resource bounds checks.
    /// Pass the result to [`crate::vm::Vm::new`] for full verification or
    /// to [`crate::vm::Vm::new_unchecked`] for trust-based skipping of
    /// the bounds checks.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, LoadError> {
        // V0.2.0 Phase 7c routes the consumer through the wire-
        // format reader. The framing, magic, version, length,
        // and CRC residue checks run inside
        // `module_from_wire_bytes`; the opcode stream and
        // operand pool sections supply the chunk ops while the
        // auxiliary body's rkyv archive supplies the rest of the
        // module.
        crate::wire_format::module_from_wire_bytes(bytes)
    }

    /// Validate framing and return a borrowed archived view of the module.
    ///
    /// Performs the same framing checks as [`Module::from_bytes`] (magic,
    /// length, CRC residue, version, word size, address size) and then
    /// runs `rkyv::access` on the body to obtain a `&'a ArchivedModule`
    /// without deserialization.
    ///
    /// The body must be 8-byte aligned within the slice. Because the
    /// header is sixteen bytes, the body is 8-byte aligned within the
    /// slice when the slice base itself is 8-byte aligned. Hosts that compute
    /// or load bytecode into an `rkyv::util::AlignedVec` or a static
    /// buffer with `#[repr(align(8))]` satisfy this requirement.
    /// Bytecode placed by the linker into a section that aligns to at
    /// least 8 bytes also satisfies it.
    ///
    /// Returns `LoadError::Codec` with an alignment message when the
    /// body is not aligned, or when the rkyv structural validator
    /// rejects the body. Returns the other `LoadError` variants for
    /// header validation failures.
    pub fn access_bytes(
        bytes: &[u8],
    ) -> Result<&crate::wire_format::ArchivedWireAuxBody, LoadError> {
        use alloc::format;
        // V0.2.0 Phase 7c routes the zero-copy view through the
        // wire format. `parse_wire_sections` validates the
        // framing header, CRC residue, and section bounds; the
        // header-mirrored target widths are checked separately
        // through `read_header_fields`. The returned auxiliary
        // body slice points into the input buffer at the
        // wire-format aux_body section; that section is rkyv-
        // archived and lives on an 8-byte aligned offset.
        let header = crate::wire_format::read_header_fields(bytes)?;
        if header.word_bits_log2 > RUNTIME_WORD_BITS_LOG2 {
            return Err(LoadError::WordSizeMismatch {
                got: header.word_bits_log2,
                max_supported: RUNTIME_WORD_BITS_LOG2,
            });
        }
        if header.addr_bits_log2 > RUNTIME_ADDRESS_BITS_LOG2 {
            return Err(LoadError::AddressSizeMismatch {
                got: header.addr_bits_log2,
                max_supported: RUNTIME_ADDRESS_BITS_LOG2,
            });
        }
        if header.float_bits_log2 > RUNTIME_FLOAT_BITS_LOG2 {
            return Err(LoadError::FloatSizeMismatch {
                got: header.float_bits_log2,
                max_supported: RUNTIME_FLOAT_BITS_LOG2,
            });
        }
        if header.wcet_cycles == u32::MAX {
            return Err(LoadError::WcetOverflow);
        }
        if header.wcmu_bytes == u32::MAX {
            return Err(LoadError::WcmuOverflow);
        }
        let sections = crate::wire_format::parse_wire_sections(bytes)?;
        if !(sections.aux_body.as_ptr() as usize).is_multiple_of(8) {
            return Err(LoadError::Codec(format!(
                "auxiliary body not 8-byte aligned (slice base 0x{:x}); use Module::from_bytes for unaligned input",
                bytes.as_ptr() as usize
            )));
        }
        rkyv::access::<crate::wire_format::ArchivedWireAuxBody, rkyv::rancor::Error>(
            sections.aux_body,
        )
        .map_err(|e| LoadError::Codec(format!("rkyv access failed: {}", e)))
    }

    /// Deserialize a module from an aligned byte slice without the
    /// AlignedVec copy step that [`Module::from_bytes`] performs.
    ///
    /// Validates the framing through [`Module::access_bytes`] and then
    /// calls `rkyv::deserialize` on the validated archived form. Returns
    /// an owned `Module` for compatibility with the existing execution
    /// path. The wire-format validation runs in place against the input
    /// slice. The deserialization step still allocates the owned form.
    ///
    /// True zero-copy execution against `&ArchivedModule` is recorded as
    /// the next iteration of P10. Path B requires lifetime-parameterizing
    /// the Vm and rewriting the execution loop to read from
    /// `&ArchivedModule`. The current view path delivers in-place
    /// validation and is the architectural foundation for Phase 2.
    ///
    /// Requires the body to be 8-byte aligned. See [`Module::access_bytes`]
    /// for the alignment contract.
    pub fn view_bytes(bytes: &[u8]) -> Result<Module, LoadError> {
        // V0.2.0 Phase 7c routes view_bytes through the wire
        // format. The aux body's archived form does not carry
        // the ops; the wire-format reader assembles each chunk's
        // ops from the opcode stream section.
        crate::wire_format::module_from_wire_bytes(bytes)
    }
}

impl ConstValue {
    /// Lower a runtime [`Value`] into a compile-time [`ConstValue`].
    ///
    /// Returns `Err` for the runtime-only variant [`Value::KStr`]
    /// which cannot be embedded in the bytecode's constant pool.
    /// The compiler is the sole caller and uses this at the boundary
    /// where it pushes constants to a chunk's pool.
    pub fn try_from_value(value: Value) -> Result<Self, &'static str> {
        match value {
            Value::Unit => Ok(ConstValue::Unit),
            Value::Bool(b) => Ok(ConstValue::Bool(b)),
            Value::Int(i) => Ok(ConstValue::Int(i)),
            Value::Byte(b) => Ok(ConstValue::Byte(b)),
            Value::Fixed(i) => Ok(ConstValue::Fixed(i)),
            #[cfg(feature = "floats")]
            Value::Float(f) => Ok(ConstValue::Float(f)),
            Value::StaticStr(s) => Ok(ConstValue::StaticStr(s)),
            Value::KStr(_) => Err("KStr cannot be a compile-time constant"),
            Value::Opaque(_) => Err("Opaque cannot be a compile-time constant"),
            Value::Tuple(items) => items
                .into_elements()
                .into_iter()
                .map(ConstValue::try_from_value)
                .collect::<Result<Vec<_>, _>>()
                .map(ConstValue::Tuple),
            Value::Array(items) => items
                .into_elements()
                .into_iter()
                .map(ConstValue::try_from_value)
                .collect::<Result<Vec<_>, _>>()
                .map(ConstValue::Array),
            Value::Struct(StructBody::Boxed(b)) => {
                let BoxedStruct { type_name, fields } = *b;
                let cfields: Result<Vec<_>, _> = fields
                    .into_iter()
                    .map(|(n, v)| ConstValue::try_from_value(v).map(|cv| (n, cv)))
                    .collect();
                Ok(ConstValue::Struct {
                    type_name,
                    fields: cfields?,
                })
            }
            // A flat struct body carries no field names or values to
            // recover; compile-time constant folding runs before flat
            // construction, so a flat struct never reaches this path.
            Value::Struct(StructBody::Flat(_)) => {
                Err("a flat struct cannot be converted to a compile-time constant")
            }
            Value::Enum(EnumBody::Boxed(b)) => {
                let BoxedEnum {
                    type_name,
                    variant,
                    fields,
                } = *b;
                let cfields: Result<Vec<_>, _> =
                    fields.into_iter().map(ConstValue::try_from_value).collect();
                // A boxed runtime enum carries no discriminant to recover,
                // so the constant materialises boxed (B28 P2).
                Ok(ConstValue::Enum {
                    type_name,
                    variant,
                    discriminant: None,
                    fields: cfields?,
                })
            }
            // A flat enum carries a discriminant and bytes, not the
            // variant name and values a constant needs; constant folding
            // runs before flat construction, so this is unreachable on a
            // valid path.
            Value::Enum(EnumBody::Flat(_)) => {
                Err("a flat enum cannot be converted to a compile-time constant")
            }
            Value::None => Ok(ConstValue::None),
            #[cfg(not(feature = "floats"))]
            Value::_PhantomFloat(_) => unreachable!("_PhantomFloat is never constructed"),
        }
    }

    /// Lift a [`ConstValue`] into a runtime [`Value`].
    ///
    /// Inverse of [`ConstValue::try_from_value`] for the constant
    /// subset. Always succeeds because every `ConstValue` variant has
    /// a corresponding `Value` variant.
    pub fn into_value(self) -> Value {
        match self {
            ConstValue::Unit => Value::Unit,
            ConstValue::Bool(b) => Value::Bool(b),
            ConstValue::Int(i) => Value::Int(i),
            ConstValue::Byte(b) => Value::Byte(b),
            ConstValue::Fixed(i) => Value::Fixed(i),
            #[cfg(feature = "floats")]
            ConstValue::Float(f) => Value::Float(f),
            ConstValue::StaticStr(s) => Value::StaticStr(s),
            // The bundled `Value` is `GenericValue<i64, f64>`, so the
            // scalar widths are eight bytes each. Routing through
            // `tuple_with_widths` keeps this constant tuple's body
            // representation identical to the runtime and archived
            // paths (B28 P2).
            ConstValue::Tuple(items) => Value::tuple_with_widths(
                items.into_iter().map(ConstValue::into_value).collect(),
                8,
                8,
            ),
            ConstValue::Array(items) => Value::array_with_widths(
                items.into_iter().map(ConstValue::into_value).collect(),
                8,
                8,
            ),
            ConstValue::Struct { type_name, fields } => Value::struct_with_widths(
                type_name,
                fields
                    .into_iter()
                    .map(|(n, v)| (n, v.into_value()))
                    .collect(),
                8,
                8,
            ),
            // A resolved discriminant materialises the flat body that
            // matches the baked access; otherwise boxed (B28 P2).
            ConstValue::Enum {
                type_name,
                variant,
                discriminant,
                fields,
            } => {
                let vals: Vec<Value> = fields.into_iter().map(ConstValue::into_value).collect();
                match discriminant {
                    Some(disc) => Value::enum_with_widths(type_name, variant, disc, vals, 0, 8, 8),
                    None => Value::Enum(EnumBody::boxed(type_name, variant, vals)),
                }
            }
            ConstValue::None => Value::None,
        }
    }
}

impl PartialEq for ConstValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ConstValue::Unit, ConstValue::Unit) | (ConstValue::None, ConstValue::None) => true,
            (ConstValue::Bool(a), ConstValue::Bool(b)) => a == b,
            (ConstValue::Int(a), ConstValue::Int(b)) => a == b,
            (ConstValue::Byte(a), ConstValue::Byte(b)) => a == b,
            (ConstValue::Fixed(a), ConstValue::Fixed(b)) => a == b,
            #[cfg(feature = "floats")]
            (ConstValue::Float(a), ConstValue::Float(b)) => a == b,
            (ConstValue::StaticStr(a), ConstValue::StaticStr(b)) => a == b,
            (ConstValue::Tuple(a), ConstValue::Tuple(b))
            | (ConstValue::Array(a), ConstValue::Array(b)) => a == b,
            (
                ConstValue::Struct {
                    type_name: na,
                    fields: fa,
                },
                ConstValue::Struct {
                    type_name: nb,
                    fields: fb,
                },
            ) => na == nb && fa == fb,
            (
                ConstValue::Enum {
                    type_name: na,
                    variant: va,
                    fields: fa,
                    ..
                },
                ConstValue::Enum {
                    type_name: nb,
                    variant: vb,
                    fields: fb,
                    ..
                },
            ) => na == nb && va == vb && fa == fb,
            _ => false,
        }
    }
}

/// Convert an archived `ConstValue` to its owned [`Value`] form.
///
/// Recursive. Materializes the entire value tree as owned. For
/// constants loaded into the operand stack at runtime under the
/// zero-copy execution path. The cost per load is proportional to the
/// constant's size; for primitive constants the cost is one match arm
/// and a small copy. For string and composite constants the cost
/// includes a heap allocation.
pub fn value_from_archived<W: crate::word::Word, F: crate::float::Float>(
    archived: &ArchivedConstValue,
    word_bytes: usize,
    float_bytes: usize,
) -> GenericValue<W, F> {
    GenericValue::<W, F>::from_const_archived(archived, word_bytes, float_bytes)
}

/// Sign-extending truncation to a narrower-than-runtime word width.
///
/// When bytecode declares a word size narrower than the runtime
/// supports, the VM applies this mask to the low half of each
/// integer-arithmetic result so the result fits the bytecode's
/// declared width. For `word_bits_log2 >= 6` the function is the
/// identity, since the runtime's native i64 already matches or
/// exceeds the declared width.
///
/// V0.2.0 Consolidation B and the post-V0.2.0 follow-on. The
/// `Op::Add` / `Op::Sub` / `Op::Mul` / `Op::Neg` family no longer
/// accepts `Int` operands; the compiler routes `Int` arithmetic
/// through `CheckedXxx` followed by `PopN(2)`. The checked
/// dispatch applies this truncation to the `low` half so the
/// wrapping result matches the bytecode's declared width, and the
/// flag detection through [`declared_width_range`] reports
/// overflow against the declared (narrower) range rather than the
/// runtime width.
pub(crate) fn truncate_int_to_declared_width(value: i64, word_bits_log2: u8) -> i64 {
    if word_bits_log2 >= 6 {
        return value;
    }
    let bits = 1u32 << word_bits_log2;
    let shift = 64 - bits;
    (value << shift) >> shift
}

#[cfg(test)]
mod cost_model_tests {
    use super::*;

    #[test]
    fn nominal_cost_model_value_slot_bytes_matches_constant() {
        assert_eq!(NOMINAL_COST_MODEL.value_slot_bytes, VALUE_SLOT_SIZE_BYTES);
    }

    #[test]
    fn runtime_width_constants_track_narrowing_features() {
        // B16 step 12: the RUNTIME_*_BITS_LOG2 constants reflect the
        // narrowing Cargo features in effect for this build. The
        // narrowest enabled feature wins per dimension. With no
        // narrowing features enabled the defaults are 6/6/6 (i64,
        // u64, f64). The test pins the constants per feature
        // combination so future refactors do not regress the
        // narrowest-wins rule.
        #[cfg(feature = "narrow-word-8")]
        assert_eq!(RUNTIME_WORD_BITS_LOG2, 3);
        #[cfg(all(feature = "narrow-word-16", not(feature = "narrow-word-8")))]
        assert_eq!(RUNTIME_WORD_BITS_LOG2, 4);
        #[cfg(all(
            feature = "narrow-word-32",
            not(any(feature = "narrow-word-8", feature = "narrow-word-16"))
        ))]
        assert_eq!(RUNTIME_WORD_BITS_LOG2, 5);
        #[cfg(not(any(
            feature = "narrow-word-8",
            feature = "narrow-word-16",
            feature = "narrow-word-32"
        )))]
        assert_eq!(RUNTIME_WORD_BITS_LOG2, 6);

        #[cfg(feature = "narrow-address-8")]
        assert_eq!(RUNTIME_ADDRESS_BITS_LOG2, 3);
        #[cfg(all(feature = "narrow-address-16", not(feature = "narrow-address-8")))]
        assert_eq!(RUNTIME_ADDRESS_BITS_LOG2, 4);
        #[cfg(all(
            feature = "narrow-address-32",
            not(any(feature = "narrow-address-8", feature = "narrow-address-16"))
        ))]
        assert_eq!(RUNTIME_ADDRESS_BITS_LOG2, 5);
        #[cfg(not(any(
            feature = "narrow-address-8",
            feature = "narrow-address-16",
            feature = "narrow-address-32"
        )))]
        assert_eq!(RUNTIME_ADDRESS_BITS_LOG2, 6);

        #[cfg(feature = "narrow-float-32")]
        assert_eq!(RUNTIME_FLOAT_BITS_LOG2, 5);
        #[cfg(not(feature = "narrow-float-32"))]
        assert_eq!(RUNTIME_FLOAT_BITS_LOG2, 6);
    }

    #[test]
    fn nominal_cost_model_cycles_match_op_cost_method() {
        // The Op::cost backward-compatibility wrapper must agree with
        // the nominal cost model's cycle table for every variant. Pick
        // a representative sample across the cost tiers.
        let ops: alloc::vec::Vec<Op> = alloc::vec![
            Op::Const(0),
            Op::PushImmediate(0),
            Op::Add,
            Op::Mul,
            Op::Div,
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: crate::value_layout::CompositeKind::Array,
                count: 2,
                byte_size: 16,
            }),
            Op::Call(0, 0),
            Op::Yield,
        ];
        for op in &ops {
            assert_eq!(NOMINAL_COST_MODEL.cycles(op), op.cost());
        }
    }

    #[test]
    fn cost_model_slots_to_bytes_uses_slot_size() {
        let model = CostModel {
            value_slot_bytes: 8,
            op_cycles: nominal_op_cycles,
        };
        assert_eq!(model.slots_to_bytes(0), 0);
        assert_eq!(model.slots_to_bytes(1), 8);
        assert_eq!(model.slots_to_bytes(4), 32);
    }

    #[test]
    fn cost_model_heap_alloc_bytes_is_operand_exact_not_slot_scaled() {
        // B28 P4: NewComposite carries its precise flat allocation size
        // in the operand, so the reported heap allocation is that byte
        // count verbatim and is independent of the model's
        // `value_slot_bytes`. Two models with different slot sizes must
        // agree on the composite's heap cost.
        let nominal = NOMINAL_COST_MODEL;
        let custom = CostModel {
            value_slot_bytes: VALUE_SLOT_SIZE_BYTES / 2,
            op_cycles: nominal_op_cycles,
        };
        let chunk = Chunk {
            name: alloc::string::String::from("test"),
            ops: alloc::vec::Vec::new(),
            constants: alloc::vec::Vec::new(),
            struct_templates: alloc::vec::Vec::new(),
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Func,
            param_types: alloc::vec::Vec::new(),
            debug_pool: None,
        };
        let op = Op::NewComposite(NewCompositeOperand::Flat {
            kind: crate::value_layout::CompositeKind::Array,
            count: 4,
            byte_size: 32,
        });
        let nominal_bytes = nominal.heap_alloc_bytes(&op, &chunk);
        let custom_bytes = custom.heap_alloc_bytes(&op, &chunk);
        assert_eq!(nominal_bytes, 32);
        assert_eq!(custom_bytes, 32);
        assert_eq!(custom_bytes, nominal_bytes);
    }

    #[test]
    fn custom_cost_model_returns_custom_cycles() {
        // Demonstrate that a host-supplied op_cycles function flows
        // through the model. The custom function returns a flat 100
        // for every op; the model's `cycles` must return that value.
        fn flat_hundred(_op: &Op) -> u32 {
            100
        }
        let custom = CostModel {
            value_slot_bytes: VALUE_SLOT_SIZE_BYTES,
            op_cycles: flat_hundred,
        };
        assert_eq!(custom.cycles(&Op::Add), 100);
        assert_eq!(custom.cycles(&Op::PushImmediate(0)), 100);
        assert_eq!(custom.cycles(&Op::Call(0, 0)), 100);
    }

    #[test]
    fn op_cost_fixed_evaluates_to_inner_value() {
        let ctx = OpCostContext::default();
        assert_eq!(OpCost::Fixed(42).evaluate(&ctx), 42);
        assert_eq!(OpCost::Fixed(0).evaluate(&ctx), 0);
    }

    #[test]
    fn op_cost_dynamic_invokes_function_with_context() {
        fn sum_lengths(ctx: &OpCostContext) -> u32 {
            ctx.lhs_text_len.saturating_add(ctx.rhs_text_len)
        }
        let cost = OpCost::Dynamic(sum_lengths);
        let ctx = OpCostContext {
            lhs_text_len: 100,
            rhs_text_len: 200,
        };
        assert_eq!(cost.evaluate(&ctx), 300);
    }

    #[test]
    fn op_cost_dynamic_saturates_at_u32_max_for_unbounded_operand() {
        fn sum_lengths(ctx: &OpCostContext) -> u32 {
            ctx.lhs_text_len.saturating_add(ctx.rhs_text_len)
        }
        let cost = OpCost::Dynamic(sum_lengths);
        let ctx = OpCostContext {
            lhs_text_len: u32::MAX,
            rhs_text_len: 100,
        };
        assert_eq!(cost.evaluate(&ctx), u32::MAX);
    }

    #[test]
    fn heap_alloc_cost_text_add_is_dynamic() {
        let chunk = Chunk {
            name: alloc::string::String::from("test"),
            ops: alloc::vec::Vec::new(),
            constants: alloc::vec::Vec::new(),
            struct_templates: alloc::vec::Vec::new(),
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Func,
            param_types: alloc::vec::Vec::new(),
            debug_pool: None,
        };
        let cost = NOMINAL_COST_MODEL.heap_alloc_cost(&Op::Add, &chunk);
        assert!(matches!(cost, OpCost::Dynamic(_)));
        let ctx = OpCostContext {
            lhs_text_len: 5,
            rhs_text_len: 6,
        };
        assert_eq!(cost.evaluate(&ctx), 11);
    }

    #[test]
    fn heap_alloc_cost_composite_is_fixed() {
        let chunk = Chunk {
            name: alloc::string::String::from("test"),
            ops: alloc::vec::Vec::new(),
            constants: alloc::vec::Vec::new(),
            struct_templates: alloc::vec::Vec::new(),
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Func,
            param_types: alloc::vec::Vec::new(),
            debug_pool: None,
        };
        let op = Op::NewComposite(NewCompositeOperand::Flat {
            kind: crate::value_layout::CompositeKind::Array,
            count: 3,
            byte_size: 24,
        });
        let cost = NOMINAL_COST_MODEL.heap_alloc_cost(&op, &chunk);
        assert!(matches!(cost, OpCost::Fixed(_)));
        assert_eq!(cost.evaluate(&OpCostContext::default()), 24);
    }

    #[test]
    fn heap_alloc_bytes_text_add_reports_zero_in_fixed_view() {
        // The Fixed-view accessor saturates dynamic costs to zero
        // because they require abstract-interpretation context.
        let chunk = Chunk {
            name: alloc::string::String::from("test"),
            ops: alloc::vec::Vec::new(),
            constants: alloc::vec::Vec::new(),
            struct_templates: alloc::vec::Vec::new(),
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Func,
            param_types: alloc::vec::Vec::new(),
            debug_pool: None,
        };
        assert_eq!(NOMINAL_COST_MODEL.heap_alloc_bytes(&Op::Add, &chunk), 0);
    }
}

#[cfg(test)]
mod flat_scalar_bridge_tests {
    use super::*;
    use crate::value_layout::ScalarKind;

    type V = Value; // GenericValue<i64, f64>

    // Bundled runtime widths.
    const W8: usize = 8;
    const F8: usize = 8;

    fn roundtrip(v: V, kind: ScalarKind, word_bytes: usize, float_bytes: usize, size: usize) -> V {
        let mut buf = alloc::vec![0u8; size];
        v.write_scalar_le(&mut buf, 0, word_bytes, float_bytes);
        V::read_scalar_le(&buf, 0, kind, word_bytes, float_bytes)
    }

    #[test]
    fn bool_byte_roundtrip() {
        assert_eq!(
            roundtrip(V::Bool(true), ScalarKind::Bool, W8, F8, 1),
            V::Bool(true)
        );
        assert_eq!(
            roundtrip(V::Bool(false), ScalarKind::Bool, W8, F8, 1),
            V::Bool(false)
        );
        assert_eq!(
            roundtrip(V::Byte(0xAB), ScalarKind::Byte, W8, F8, 1),
            V::Byte(0xAB)
        );
    }

    #[test]
    fn int_roundtrip_full_word() {
        for n in [0i64, 42, -5, i64::MAX, i64::MIN, -1] {
            assert_eq!(roundtrip(V::Int(n), ScalarKind::Int, W8, F8, 8), V::Int(n));
        }
    }

    #[test]
    fn fixed_roundtrip_returns_fixed_kind() {
        assert_eq!(
            roundtrip(V::Fixed(-123), ScalarKind::Fixed, W8, F8, 8),
            V::Fixed(-123)
        );
    }

    #[test]
    fn int_narrow_word_sign_extends() {
        // A 2-byte word: low 16 bits stored, sign-extended on read.
        assert_eq!(roundtrip(V::Int(-5), ScalarKind::Int, 2, F8, 2), V::Int(-5));
        assert_eq!(
            roundtrip(V::Int(1234), ScalarKind::Int, 2, F8, 2),
            V::Int(1234)
        );
        // -32768 is the most-negative 16-bit value.
        assert_eq!(
            roundtrip(V::Int(-32768), ScalarKind::Int, 2, F8, 2),
            V::Int(-32768)
        );
    }

    #[test]
    fn unit_writes_no_bytes() {
        let mut buf = [0xFFu8; 0];
        V::Unit.write_scalar_le(&mut buf, 0, W8, F8);
        assert_eq!(
            V::read_scalar_le(&buf, 0, ScalarKind::Unit, W8, F8),
            V::Unit
        );
    }

    #[cfg(feature = "floats")]
    #[test]
    fn float_roundtrip_f64_and_f32_width() {
        // f64 width is exact.
        assert_eq!(
            roundtrip(V::Float(0.1), ScalarKind::Float, W8, F8, 8),
            V::Float(0.1)
        );
        // 4-byte float width round-trips values exactly representable in f32.
        assert_eq!(
            roundtrip(V::Float(0.5), ScalarKind::Float, W8, 4, 4),
            V::Float(0.5)
        );
    }
}

#[cfg(test)]
mod materialise_kstrings_tests {
    use super::*;
    use crate::kstring::KString;

    type V = Value;

    fn make_arena() -> keleusma_arena::Arena {
        keleusma_arena::Arena::with_capacity(1024)
    }

    #[test]
    fn scalar_values_are_cloned_unchanged() {
        let arena = make_arena();
        assert_eq!(V::Int(42).materialise_kstrings(&arena), V::Int(42));
        assert_eq!(V::Bool(true).materialise_kstrings(&arena), V::Bool(true));
        assert_eq!(V::Unit.materialise_kstrings(&arena), V::Unit);
        assert_eq!(V::None.materialise_kstrings(&arena), V::None);
    }

    #[test]
    fn staticstr_is_cloned_unchanged() {
        let arena = make_arena();
        let v = V::StaticStr(alloc::string::String::from("hello"));
        assert_eq!(v.materialise_kstrings(&arena), v);
    }

    #[test]
    fn kstr_becomes_staticstr_with_arena_contents() {
        let arena = make_arena();
        let handle = KString::alloc(&arena, "the original bytes").expect("alloc");
        let v: V = V::KStr(handle);
        let materialised = v.materialise_kstrings(&arena);
        match materialised {
            V::StaticStr(s) => assert_eq!(s, "the original bytes"),
            other => panic!("expected StaticStr, got {:?}", other),
        }
    }

    #[test]
    fn tuple_walks_recursively() {
        let arena = make_arena();
        let handle = KString::alloc(&arena, "inner").expect("alloc");
        let v = V::tuple(alloc::vec![V::Int(1), V::KStr(handle), V::Bool(false),]);
        let materialised = v.materialise_kstrings(&arena);
        match materialised {
            V::Tuple(items) => {
                let items = items.elements();
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], V::Int(1));
                match &items[1] {
                    V::StaticStr(s) => assert_eq!(s, "inner"),
                    other => panic!("expected StaticStr inside tuple, got {:?}", other),
                }
                assert_eq!(items[2], V::Bool(false));
            }
            other => panic!("expected Tuple, got {:?}", other),
        }
    }

    #[test]
    fn enum_with_kstr_payload_walks_recursively() {
        let arena = make_arena();
        let handle = KString::alloc(&arena, "payload").expect("alloc");
        let v = V::Enum(EnumBody::boxed(
            alloc::string::String::from("Option"),
            alloc::string::String::from("Some"),
            alloc::vec![V::KStr(handle)],
        ));
        let materialised = v.materialise_kstrings(&arena);
        match materialised {
            V::Enum(EnumBody::Boxed(b)) => {
                assert_eq!(b.fields.len(), 1);
                match &b.fields[0] {
                    V::StaticStr(s) => assert_eq!(s, "payload"),
                    other => panic!("expected StaticStr inside enum, got {:?}", other),
                }
            }
            other => panic!("expected Enum, got {:?}", other),
        }
    }

    #[test]
    fn struct_walks_recursively() {
        // Built as an explicit boxed struct: `struct_value` now flattens a
        // struct whose fields are all flat (a `Text`/`KStr` field is a
        // two-word flat reference, B28 P3), and a flat struct's text field
        // is an arena reference reattached at access through the epoch
        // wrapper, not a `KStr` that `materialise_kstrings` converts. The
        // recursive walk over a boxed struct is what this test exercises.
        let arena = make_arena();
        let handle = KString::alloc(&arena, "field-value").expect("alloc");
        let v = V::Struct(StructBody::boxed(
            alloc::string::String::from("Point"),
            alloc::vec![
                (alloc::string::String::from("x"), V::Int(7)),
                (alloc::string::String::from("name"), V::KStr(handle)),
            ],
        ));
        let materialised = v.materialise_kstrings(&arena);
        match materialised {
            V::Struct(StructBody::Boxed(b)) => {
                assert_eq!(b.fields.len(), 2);
                assert_eq!(b.fields[0].1, V::Int(7));
                match &b.fields[1].1 {
                    V::StaticStr(s) => assert_eq!(s, "field-value"),
                    other => panic!("expected StaticStr inside struct, got {:?}", other),
                }
            }
            other => panic!("expected Struct, got {:?}", other),
        }
    }
}
