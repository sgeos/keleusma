//! Flat auxiliary-body encoding (wire format version 2).
//!
//! Replaces the `rkyv`-archived auxiliary body with a byte-addressed format that
//! a Keleusma stage can emit sequentially and that the runtime can read in place.
//! See `docs/decisions/WIRE_FORMAT_V2_FLAT_AUX.md` for the design and its rationale.
//!
//! # Why not rkyv
//!
//! rkyv is a zero-copy archive format built on relative pointers, alignment rules,
//! and padding decided by the derive macro. Reproducing its byte layout from a
//! Keleusma stage would mean reimplementing a third-party archival format exactly,
//! and an rkyv upgrade would silently invalidate that work. This format trades
//! those mechanisms for fixed little-endian fields at computed offsets: every read
//! is a bounds check plus an addition, which is auditable, statically bounded, and
//! emittable by a stage that only appends bytes.
//!
//! # Invariants
//!
//! - **Little-endian, fixed-width scalars.** No varints: the emitter appends bytes
//!   in order and every field's width is known without decoding what precedes it.
//! - **No alignment requirement.** Every field is byte-addressed, which is what
//!   removes the aligned copy the rkyv loader had to make before decoding.
//! - **Offsets are relative to the start of the region that contains them**, so a
//!   region is position-independent within the aux body and the aux body is
//!   position-independent within the module buffer.
//! - **Deterministic.** Equal inputs produce equal bytes; the byte-identical
//!   differential oracle that gates the self-hosted compiler depends on it.
//!
//! Variable-length collections that the runtime indexes (the chunk table, and each
//! chunk's constant pool) carry an offset table so element *i* is reachable in O(1)
//! without scanning. Collections that are only ever iterated are stored packed.

use alloc::string::String;
use alloc::vec::Vec;

use crate::bytecode::{
    BlockType, ChunkSignature, ConstValue, DataLayout, DataSlot, EnumLayout, EnumVariantDisc,
    PrivateCompositeSlot, SharedSlotLayout, SlotVisibility, StructTemplate, TypeTag, WireShape,
};
use crate::wire_format::{WireAuxBody, WireChunk};

/// Magic identifying a flat auxiliary body, so a version-1 rkyv body is rejected
/// on inspection rather than misparsed.
pub const FLAT_AUX_MAGIC: u32 = 0x4B41_5558; // "KAUX"

/// Layout revision of the flat auxiliary body itself, independent of
/// `BYTECODE_VERSION`. Additive regions do not require a bump; a change to an
/// existing region's layout does.
pub const FLAT_AUX_VERSION: u16 = 1;

/// Region identifiers in the aux-body directory. A directory rather than a fixed
/// field order means a later region can be appended without shifting the offsets
/// of existing ones, so the format extends without another `BYTECODE_VERSION` bump.
pub mod region {
    /// Scalar header fields (entry point, widths, WCET/WCMU, flags, data sizes,
    /// schema hash).
    pub const HEADER: u16 = 1;
    /// Chunk table: count, offset table, then per-chunk records.
    pub const CHUNKS: u16 = 2;
    /// Native function names.
    pub const NATIVE_NAMES: u16 = 3;
    /// Data-segment layout, present only when the module declares one.
    pub const DATA_LAYOUT: u16 = 4;
    /// Per-enum-type layout descriptors.
    pub const ENUM_LAYOUTS: u16 = 5;
    /// Per-chunk signature descriptors for the typed verifier pass.
    pub const SIGNATURES: u16 = 6;
    /// Native return-value flat shapes, parallel to `NATIVE_NAMES`.
    pub const NATIVE_RETURN_SHAPES: u16 = 7;
}

/// Bytes in one region-directory entry: kind, reserved, offset, length.
const DIR_ENTRY_BYTES: usize = 12;

/// Errors from decoding a flat auxiliary body.
///
/// Every variant means the buffer is malformed. The decoder is total: it never
/// panics and never reads outside the slice it was given, so a corrupt or hostile
/// buffer produces an error rather than undefined behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlatAuxError {
    /// Buffer too short for the structure being read at that offset.
    Truncated {
        /// Byte offset at which the read was attempted.
        at: usize,
    },
    /// Leading magic did not match [`FLAT_AUX_MAGIC`].
    BadMagic {
        /// The magic actually found.
        found: u32,
    },
    /// Layout revision is not one this build understands.
    BadVersion {
        /// The revision actually found.
        found: u16,
    },
    /// A required region is absent from the directory.
    MissingRegion {
        /// The absent region's identifier.
        kind: u16,
    },
    /// A discriminant byte did not name a known variant.
    BadTag {
        /// Human-readable name of the enum being decoded.
        what: &'static str,
        /// The unrecognised discriminant.
        tag: u8,
    },
    /// A string region did not hold valid UTF-8.
    BadUtf8 {
        /// Byte offset of the offending string.
        at: usize,
    },
    /// Constant nesting exceeded [`MAX_CONST_DEPTH`]. Guards the recursive
    /// constant decoder against stack exhaustion on a hostile buffer.
    DepthExceeded {
        /// Byte offset at which the cap was hit.
        at: usize,
    },
}

// --------------------------------------------------------------------------
// Primitive writers. Every multi-byte field goes through one of these so the
// little-endian convention is stated once rather than at each call site.
// --------------------------------------------------------------------------

fn put_u8(out: &mut Vec<u8>, v: u8) {
    out.push(v);
}

fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_i64(out: &mut Vec<u8>, v: i64) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// A length-prefixed UTF-8 string.
fn put_str(out: &mut Vec<u8>, s: &str) {
    put_u32(out, s.len() as u32);
    out.extend_from_slice(s.as_bytes());
}

/// A length-prefixed byte block.
fn put_bytes(out: &mut Vec<u8>, b: &[u8]) {
    put_u32(out, b.len() as u32);
    out.extend_from_slice(b);
}

// --------------------------------------------------------------------------
// Primitive readers. Each is bounds-checked and returns the next offset, so a
// truncated buffer produces `Truncated` rather than a panic.
// --------------------------------------------------------------------------

fn get_u8(b: &[u8], at: usize) -> Result<(u8, usize), FlatAuxError> {
    if at >= b.len() {
        return Err(FlatAuxError::Truncated { at });
    }
    Ok((b[at], at + 1))
}

fn get_u16(b: &[u8], at: usize) -> Result<(u16, usize), FlatAuxError> {
    if at + 2 > b.len() {
        return Err(FlatAuxError::Truncated { at });
    }
    Ok((u16::from_le_bytes([b[at], b[at + 1]]), at + 2))
}

fn get_u32(b: &[u8], at: usize) -> Result<(u32, usize), FlatAuxError> {
    if at + 4 > b.len() {
        return Err(FlatAuxError::Truncated { at });
    }
    Ok((
        u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]),
        at + 4,
    ))
}

fn get_u64(b: &[u8], at: usize) -> Result<(u64, usize), FlatAuxError> {
    if at + 8 > b.len() {
        return Err(FlatAuxError::Truncated { at });
    }
    let mut w = [0u8; 8];
    w.copy_from_slice(&b[at..at + 8]);
    Ok((u64::from_le_bytes(w), at + 8))
}

fn get_i64(b: &[u8], at: usize) -> Result<(i64, usize), FlatAuxError> {
    let (v, next) = get_u64(b, at)?;
    Ok((v as i64, next))
}

fn get_slice(b: &[u8], at: usize) -> Result<(&[u8], usize), FlatAuxError> {
    let (len, next) = get_u32(b, at)?;
    let len = len as usize;
    if next + len > b.len() {
        return Err(FlatAuxError::Truncated { at: next });
    }
    Ok((&b[next..next + len], next + len))
}

fn get_str(b: &[u8], at: usize) -> Result<(String, usize), FlatAuxError> {
    let (raw, next) = get_slice(b, at)?;
    match core::str::from_utf8(raw) {
        Ok(s) => Ok((String::from(s), next)),
        Err(_) => Err(FlatAuxError::BadUtf8 { at }),
    }
}

// --------------------------------------------------------------------------
// Leaf enums. Each is a single discriminant byte; the payload, where present,
// follows. Tags are assigned explicitly rather than derived from declaration
// order so reordering a Rust enum cannot silently change the wire encoding.
// --------------------------------------------------------------------------

fn put_block_type(out: &mut Vec<u8>, v: BlockType) {
    put_u8(
        out,
        match v {
            BlockType::Func => 0,
            BlockType::Reentrant => 1,
            BlockType::Stream => 2,
        },
    );
}

fn get_block_type(b: &[u8], at: usize) -> Result<(BlockType, usize), FlatAuxError> {
    let (t, next) = get_u8(b, at)?;
    let v = match t {
        0 => BlockType::Func,
        1 => BlockType::Reentrant,
        2 => BlockType::Stream,
        _ => {
            return Err(FlatAuxError::BadTag {
                what: "BlockType",
                tag: t,
            });
        }
    };
    Ok((v, next))
}

fn put_type_tag(out: &mut Vec<u8>, v: TypeTag) {
    put_u8(
        out,
        match v {
            TypeTag::Composite => 0,
            TypeTag::Byte => 1,
            TypeTag::Word => 2,
            TypeTag::Fixed => 3,
            TypeTag::Float => 4,
            TypeTag::Bool => 5,
            TypeTag::Unit => 6,
            TypeTag::Text => 7,
        },
    );
}

fn get_type_tag(b: &[u8], at: usize) -> Result<(TypeTag, usize), FlatAuxError> {
    let (t, next) = get_u8(b, at)?;
    let v = match t {
        0 => TypeTag::Composite,
        1 => TypeTag::Byte,
        2 => TypeTag::Word,
        3 => TypeTag::Fixed,
        4 => TypeTag::Float,
        5 => TypeTag::Bool,
        6 => TypeTag::Unit,
        7 => TypeTag::Text,
        _ => {
            return Err(FlatAuxError::BadTag {
                what: "TypeTag",
                tag: t,
            });
        }
    };
    Ok((v, next))
}

fn put_slot_visibility(out: &mut Vec<u8>, v: SlotVisibility) {
    put_u8(
        out,
        match v {
            SlotVisibility::Shared => 0,
            SlotVisibility::Private => 1,
        },
    );
}

fn get_slot_visibility(b: &[u8], at: usize) -> Result<(SlotVisibility, usize), FlatAuxError> {
    let (t, next) = get_u8(b, at)?;
    let v = match t {
        0 => SlotVisibility::Shared,
        1 => SlotVisibility::Private,
        _ => {
            return Err(FlatAuxError::BadTag {
                what: "SlotVisibility",
                tag: t,
            });
        }
    };
    Ok((v, next))
}

fn put_wire_shape(out: &mut Vec<u8>, v: &WireShape) {
    match v {
        WireShape::Top => put_u8(out, 0),
        WireShape::Scalar { kind } => {
            put_u8(out, 1);
            put_u8(out, *kind);
        }
        WireShape::Flat { kind, size } => {
            put_u8(out, 2);
            put_u8(out, *kind);
            put_u32(out, *size);
        }
    }
}

fn get_wire_shape(b: &[u8], at: usize) -> Result<(WireShape, usize), FlatAuxError> {
    let (t, at) = get_u8(b, at)?;
    match t {
        0 => Ok((WireShape::Top, at)),
        1 => {
            let (kind, at) = get_u8(b, at)?;
            Ok((WireShape::Scalar { kind }, at))
        }
        2 => {
            let (kind, at) = get_u8(b, at)?;
            let (size, at) = get_u32(b, at)?;
            Ok((WireShape::Flat { kind, size }, at))
        }
        _ => Err(FlatAuxError::BadTag {
            what: "WireShape",
            tag: t,
        }),
    }
}

/// Maximum constant nesting the decoder will follow.
///
/// `ConstValue` is recursive, so a hostile buffer could otherwise drive unbounded
/// recursion and exhaust the stack. Real constants nest only as deep as the source
/// literal that produced them; this cap is far above any plausible program and is
/// a denial-of-service guard, not a language limit.
///
/// Note for the eventual Keleusma emitter: the verifier forbids recursion (R4), so
/// the self-hosted encoder must walk this structure with an explicit stack, as the
/// nested-equality drains do.
pub const MAX_CONST_DEPTH: u32 = 32;

/// A constant-pool entry.
///
/// `Float` is behind the `floats` feature. Its tag is reserved unconditionally so
/// a module built with floats and read by a build without them fails with
/// `BadTag` rather than silently shifting every later discriminant.
fn put_const_value(out: &mut Vec<u8>, v: &ConstValue) {
    match v {
        ConstValue::Unit => put_u8(out, 0),
        ConstValue::Bool(x) => {
            put_u8(out, 1);
            put_u8(out, u8::from(*x));
        }
        ConstValue::Int(x) => {
            put_u8(out, 2);
            put_i64(out, *x);
        }
        ConstValue::Byte(x) => {
            put_u8(out, 3);
            put_u8(out, *x);
        }
        ConstValue::Fixed(x) => {
            put_u8(out, 4);
            put_i64(out, *x);
        }
        #[cfg(feature = "floats")]
        ConstValue::Float(x) => {
            put_u8(out, 5);
            put_u64(out, x.to_bits());
        }
        ConstValue::StaticStr(s) => {
            put_u8(out, 6);
            put_str(out, s);
        }
        ConstValue::Tuple(items) => {
            put_u8(out, 7);
            put_u32(out, items.len() as u32);
            for it in items {
                put_const_value(out, it);
            }
        }
        ConstValue::Array(items) => {
            put_u8(out, 8);
            put_u32(out, items.len() as u32);
            for it in items {
                put_const_value(out, it);
            }
        }
        ConstValue::Struct { type_name, fields } => {
            put_u8(out, 9);
            put_str(out, type_name);
            put_u32(out, fields.len() as u32);
            for (name, val) in fields {
                put_str(out, name);
                put_const_value(out, val);
            }
        }
        ConstValue::Enum {
            type_name,
            variant,
            discriminant,
            fields,
        } => {
            put_u8(out, 10);
            put_str(out, type_name);
            put_str(out, variant);
            match discriminant {
                Some(d) => {
                    put_u8(out, 1);
                    put_i64(out, *d);
                }
                None => put_u8(out, 0),
            }
            put_u32(out, fields.len() as u32);
            for f in fields {
                put_const_value(out, f);
            }
        }
        ConstValue::None => put_u8(out, 11),
    }
}

fn get_const_value(b: &[u8], at: usize) -> Result<(ConstValue, usize), FlatAuxError> {
    get_const_value_at(b, at, 0)
}

fn get_const_value_at(
    b: &[u8],
    at: usize,
    depth: u32,
) -> Result<(ConstValue, usize), FlatAuxError> {
    if depth > MAX_CONST_DEPTH {
        return Err(FlatAuxError::DepthExceeded { at });
    }
    let (t, at) = get_u8(b, at)?;
    match t {
        0 => Ok((ConstValue::Unit, at)),
        1 => {
            let (x, at) = get_u8(b, at)?;
            Ok((ConstValue::Bool(x != 0), at))
        }
        2 => {
            let (x, at) = get_i64(b, at)?;
            Ok((ConstValue::Int(x), at))
        }
        3 => {
            let (x, at) = get_u8(b, at)?;
            Ok((ConstValue::Byte(x), at))
        }
        4 => {
            let (x, at) = get_i64(b, at)?;
            Ok((ConstValue::Fixed(x), at))
        }
        #[cfg(feature = "floats")]
        5 => {
            let (bits, at) = get_u64(b, at)?;
            Ok((ConstValue::Float(f64::from_bits(bits)), at))
        }
        6 => {
            let (s, at) = get_str(b, at)?;
            Ok((ConstValue::StaticStr(s), at))
        }
        7 | 8 => {
            let (count, mut at) = get_u32(b, at)?;
            let mut items = Vec::new();
            for _ in 0..count {
                let (v, next) = get_const_value_at(b, at, depth + 1)?;
                items.push(v);
                at = next;
            }
            Ok((
                if t == 7 {
                    ConstValue::Tuple(items)
                } else {
                    ConstValue::Array(items)
                },
                at,
            ))
        }
        9 => {
            let (type_name, at) = get_str(b, at)?;
            let (count, mut at) = get_u32(b, at)?;
            let mut fields = Vec::new();
            for _ in 0..count {
                let (name, next) = get_str(b, at)?;
                let (val, next) = get_const_value_at(b, next, depth + 1)?;
                fields.push((name, val));
                at = next;
            }
            Ok((ConstValue::Struct { type_name, fields }, at))
        }
        10 => {
            let (type_name, at) = get_str(b, at)?;
            let (variant, at) = get_str(b, at)?;
            let (has_disc, at) = get_u8(b, at)?;
            let (discriminant, at) = if has_disc == 1 {
                let (d, at) = get_i64(b, at)?;
                (Some(d), at)
            } else {
                (None, at)
            };
            let (count, mut at) = get_u32(b, at)?;
            let mut fields = Vec::new();
            for _ in 0..count {
                let (v, next) = get_const_value_at(b, at, depth + 1)?;
                fields.push(v);
                at = next;
            }
            Ok((
                ConstValue::Enum {
                    type_name,
                    variant,
                    discriminant,
                    fields,
                },
                at,
            ))
        }
        11 => Ok((ConstValue::None, at)),
        _ => Err(FlatAuxError::BadTag {
            what: "ConstValue",
            tag: t,
        }),
    }
}

fn put_struct_template(out: &mut Vec<u8>, t: &StructTemplate) {
    put_str(out, &t.type_name);
    put_u32(out, t.field_names.len() as u32);
    for n in &t.field_names {
        put_str(out, n);
    }
}

fn get_struct_template(b: &[u8], at: usize) -> Result<(StructTemplate, usize), FlatAuxError> {
    let (type_name, at) = get_str(b, at)?;
    let (count, mut at) = get_u32(b, at)?;
    let mut field_names = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let (n, next) = get_str(b, at)?;
        field_names.push(n);
        at = next;
    }
    Ok((
        StructTemplate {
            type_name,
            field_names,
        },
        at,
    ))
}

// --------------------------------------------------------------------------
// Aggregates. Each is a straightforward field-order encoding; collections are
// count-prefixed. Nothing here is indexed at runtime, so these are packed rather
// than offset-tabled.
// --------------------------------------------------------------------------

fn put_data_slot(out: &mut Vec<u8>, v: &DataSlot) {
    put_str(out, &v.name);
    put_slot_visibility(out, v.visibility);
}

fn get_data_slot(b: &[u8], at: usize) -> Result<(DataSlot, usize), FlatAuxError> {
    let (name, at) = get_str(b, at)?;
    let (visibility, at) = get_slot_visibility(b, at)?;
    Ok((DataSlot { name, visibility }, at))
}

fn put_data_layout(out: &mut Vec<u8>, v: &DataLayout) {
    put_u32(out, v.slots.len() as u32);
    for s in &v.slots {
        put_data_slot(out, s);
    }
    put_u32(out, v.shared_layout.len() as u32);
    for s in &v.shared_layout {
        put_u32(out, s.offset);
        put_u8(out, s.kind);
        put_u16(out, s.len);
    }
    put_u32(out, v.private_composite_layout.len() as u32);
    for s in &v.private_composite_layout {
        put_u16(out, s.slot);
        put_u32(out, s.offset);
    }
    put_u32(out, v.private_init.len() as u32);
    for c in &v.private_init {
        put_const_value(out, c);
    }
}

fn get_data_layout(b: &[u8], at: usize) -> Result<(DataLayout, usize), FlatAuxError> {
    let (n, mut at) = get_u32(b, at)?;
    let mut slots = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (s, next) = get_data_slot(b, at)?;
        slots.push(s);
        at = next;
    }
    let (n, mut at) = get_u32(b, at)?;
    let mut shared_layout = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (offset, next) = get_u32(b, at)?;
        let (kind, next) = get_u8(b, next)?;
        let (len, next) = get_u16(b, next)?;
        shared_layout.push(SharedSlotLayout { offset, kind, len });
        at = next;
    }
    let (n, mut at) = get_u32(b, at)?;
    let mut private_composite_layout = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (slot, next) = get_u16(b, at)?;
        let (offset, next) = get_u32(b, next)?;
        private_composite_layout.push(PrivateCompositeSlot { slot, offset });
        at = next;
    }
    let (n, mut at) = get_u32(b, at)?;
    let mut private_init = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (c, next) = get_const_value(b, at)?;
        private_init.push(c);
        at = next;
    }
    Ok((
        DataLayout {
            slots,
            shared_layout,
            private_composite_layout,
            private_init,
        },
        at,
    ))
}

fn put_enum_layout(out: &mut Vec<u8>, v: &EnumLayout) {
    put_str(out, &v.type_name);
    put_u32(out, v.variants.len() as u32);
    for d in &v.variants {
        put_str(out, &d.name);
        put_i64(out, d.disc);
    }
    put_u32(out, v.min_payload);
}

fn get_enum_layout(b: &[u8], at: usize) -> Result<(EnumLayout, usize), FlatAuxError> {
    let (type_name, at) = get_str(b, at)?;
    let (n, mut at) = get_u32(b, at)?;
    let mut variants = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (name, next) = get_str(b, at)?;
        let (disc, next) = get_i64(b, next)?;
        variants.push(EnumVariantDisc { name, disc });
        at = next;
    }
    let (min_payload, at) = get_u32(b, at)?;
    Ok((
        EnumLayout {
            type_name,
            variants,
            min_payload,
        },
        at,
    ))
}

fn put_chunk_signature(out: &mut Vec<u8>, v: &ChunkSignature) {
    put_u32(out, v.params.len() as u32);
    for p in &v.params {
        put_wire_shape(out, p);
    }
    put_wire_shape(out, &v.ret);
    put_wire_shape(out, &v.resume);
}

fn get_chunk_signature(b: &[u8], at: usize) -> Result<(ChunkSignature, usize), FlatAuxError> {
    let (n, mut at) = get_u32(b, at)?;
    let mut params = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (p, next) = get_wire_shape(b, at)?;
        params.push(p);
        at = next;
    }
    let (ret, at) = get_wire_shape(b, at)?;
    let (resume, at) = get_wire_shape(b, at)?;
    Ok((
        ChunkSignature {
            params,
            ret,
            resume,
        },
        at,
    ))
}

// --------------------------------------------------------------------------
// Chunk records. The constant pool carries an offset table because the runtime
// indexes it by constant id; everything else in a chunk is read as a whole.
// --------------------------------------------------------------------------

fn put_chunk(out: &mut Vec<u8>, c: &WireChunk) {
    put_str(out, &c.name);

    // Constant pool: count, then an offset table, then the entries. Offsets are
    // relative to the start of the entry block so the table can be written before
    // the entries are laid down.
    put_u32(out, c.constants.len() as u32);
    let table_at = out.len();
    for _ in &c.constants {
        put_u32(out, 0);
    }
    let entries_at = out.len();
    for (i, k) in c.constants.iter().enumerate() {
        let rel = (out.len() - entries_at) as u32;
        out[table_at + i * 4..table_at + i * 4 + 4].copy_from_slice(&rel.to_le_bytes());
        put_const_value(out, k);
    }

    put_u32(out, c.struct_templates.len() as u32);
    for t in &c.struct_templates {
        put_struct_template(out, t);
    }
    put_u16(out, c.local_count);
    put_u8(out, c.param_count);
    put_block_type(out, c.block_type);
    put_u32(out, c.param_types.len() as u32);
    for t in &c.param_types {
        put_type_tag(out, *t);
    }
    put_u32(out, c.op_byte_offset);
    put_u32(out, c.op_record_count);
    match &c.debug_pool_bytes {
        Some(bytes) => {
            put_u8(out, 1);
            put_bytes(out, bytes);
        }
        None => put_u8(out, 0),
    }
}

fn get_chunk(b: &[u8], at: usize) -> Result<(WireChunk, usize), FlatAuxError> {
    let (name, at) = get_str(b, at)?;
    let (n, at) = get_u32(b, at)?;
    // Skip the offset table: the sequential decode does not need it, but an
    // in-place reader does, so it is still written.
    let table_bytes = (n as usize)
        .checked_mul(4)
        .ok_or(FlatAuxError::Truncated { at })?;
    if at + table_bytes > b.len() {
        return Err(FlatAuxError::Truncated { at });
    }
    let mut at = at + table_bytes;
    let mut constants = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (k, next) = get_const_value(b, at)?;
        constants.push(k);
        at = next;
    }
    let (n, mut at) = get_u32(b, at)?;
    let mut struct_templates = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (t, next) = get_struct_template(b, at)?;
        struct_templates.push(t);
        at = next;
    }
    let (local_count, at) = get_u16(b, at)?;
    let (param_count, at) = get_u8(b, at)?;
    let (block_type, at) = get_block_type(b, at)?;
    let (n, mut at) = get_u32(b, at)?;
    let mut param_types = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (t, next) = get_type_tag(b, at)?;
        param_types.push(t);
        at = next;
    }
    let (op_byte_offset, at) = get_u32(b, at)?;
    let (op_record_count, at) = get_u32(b, at)?;
    let (has_debug, at) = get_u8(b, at)?;
    let (debug_pool_bytes, at) = if has_debug == 1 {
        let (raw, next) = get_slice(b, at)?;
        (Some(raw.to_vec()), next)
    } else {
        (None, at)
    };
    Ok((
        WireChunk {
            name,
            constants,
            struct_templates,
            local_count,
            param_count,
            block_type,
            param_types,
            op_byte_offset,
            op_record_count,
            debug_pool_bytes,
        },
        at,
    ))
}

// --------------------------------------------------------------------------
// Whole-body encode and decode.
// --------------------------------------------------------------------------

fn region_dir_len(count: usize) -> usize {
    8 + count * DIR_ENTRY_BYTES
}

/// Encode an auxiliary body into the flat format.
///
/// The output is deterministic: equal inputs produce equal bytes.
pub fn encode_aux(aux: &WireAuxBody) -> Vec<u8> {
    // Build each region independently, then lay the directory once the lengths
    // are known. Regions are emitted in ascending kind order for determinism.
    let mut regions: Vec<(u16, Vec<u8>)> = Vec::new();

    let mut header = Vec::new();
    match aux.entry_point {
        Some(e) => {
            put_u8(&mut header, 1);
            put_u32(&mut header, e as u32);
        }
        None => put_u8(&mut header, 0),
    }
    put_u8(&mut header, aux.word_bits_log2);
    put_u8(&mut header, aux.addr_bits_log2);
    put_u8(&mut header, aux.float_bits_log2);
    put_u32(&mut header, aux.wcet_cycles);
    put_u32(&mut header, aux.wcmu_bytes);
    put_u8(&mut header, aux.flags);
    put_u32(&mut header, aux.shared_data_bytes);
    put_u32(&mut header, aux.private_data_bytes);
    put_u32(&mut header, aux.schema_hash);
    regions.push((region::HEADER, header));

    let mut chunks = Vec::new();
    put_u32(&mut chunks, aux.chunks.len() as u32);
    let table_at = chunks.len();
    for _ in &aux.chunks {
        put_u32(&mut chunks, 0);
    }
    let entries_at = chunks.len();
    for (i, c) in aux.chunks.iter().enumerate() {
        let rel = (chunks.len() - entries_at) as u32;
        chunks[table_at + i * 4..table_at + i * 4 + 4].copy_from_slice(&rel.to_le_bytes());
        put_chunk(&mut chunks, c);
    }
    regions.push((region::CHUNKS, chunks));

    let mut names = Vec::new();
    put_u32(&mut names, aux.native_names.len() as u32);
    for n in &aux.native_names {
        put_str(&mut names, n);
    }
    regions.push((region::NATIVE_NAMES, names));

    if let Some(dl) = &aux.data_layout {
        let mut region = Vec::new();
        put_data_layout(&mut region, dl);
        regions.push((region::DATA_LAYOUT, region));
    }

    let mut enums = Vec::new();
    put_u32(&mut enums, aux.enum_layouts.len() as u32);
    for e in &aux.enum_layouts {
        put_enum_layout(&mut enums, e);
    }
    regions.push((region::ENUM_LAYOUTS, enums));

    let mut sigs = Vec::new();
    put_u32(&mut sigs, aux.signatures.len() as u32);
    for sg in &aux.signatures {
        put_chunk_signature(&mut sigs, sg);
    }
    regions.push((region::SIGNATURES, sigs));

    let mut shapes = Vec::new();
    put_u32(&mut shapes, aux.native_return_shapes.len() as u32);
    for sh in &aux.native_return_shapes {
        put_wire_shape(&mut shapes, sh);
    }
    regions.push((region::NATIVE_RETURN_SHAPES, shapes));

    let dir_len = region_dir_len(regions.len());
    let mut out = Vec::with_capacity(dir_len + regions.iter().map(|(_, r)| r.len()).sum::<usize>());
    put_u32(&mut out, FLAT_AUX_MAGIC);
    put_u16(&mut out, FLAT_AUX_VERSION);
    put_u16(&mut out, regions.len() as u16);
    let mut cursor = dir_len as u32;
    for (kind, bytes) in &regions {
        put_u16(&mut out, *kind);
        put_u16(&mut out, 0);
        put_u32(&mut out, cursor);
        put_u32(&mut out, bytes.len() as u32);
        cursor += bytes.len() as u32;
    }
    for (_, bytes) in &regions {
        out.extend_from_slice(bytes);
    }
    out
}

/// Locate a region by kind, returning its byte slice.
fn find_region(b: &[u8], kind: u16) -> Result<Option<&[u8]>, FlatAuxError> {
    let (magic, at) = get_u32(b, 0)?;
    if magic != FLAT_AUX_MAGIC {
        return Err(FlatAuxError::BadMagic { found: magic });
    }
    let (version, at) = get_u16(b, at)?;
    if version != FLAT_AUX_VERSION {
        return Err(FlatAuxError::BadVersion { found: version });
    }
    let (count, mut at) = get_u16(b, at)?;
    for _ in 0..count {
        let (k, next) = get_u16(b, at)?;
        let (_reserved, next) = get_u16(b, next)?;
        let (offset, next) = get_u32(b, next)?;
        let (length, next) = get_u32(b, next)?;
        if k == kind {
            let (o, l) = (offset as usize, length as usize);
            if o + l > b.len() {
                return Err(FlatAuxError::Truncated { at: o });
            }
            return Ok(Some(&b[o..o + l]));
        }
        at = next;
    }
    Ok(None)
}

fn require_region(b: &[u8], kind: u16) -> Result<&[u8], FlatAuxError> {
    find_region(b, kind)?.ok_or(FlatAuxError::MissingRegion { kind })
}

/// Decode a flat auxiliary body.
///
/// Total on arbitrary input: a malformed buffer yields [`FlatAuxError`], never a
/// panic and never a read outside `b`.
pub fn decode_aux(b: &[u8]) -> Result<WireAuxBody, FlatAuxError> {
    let h = require_region(b, region::HEADER)?;
    let (has_entry, at) = get_u8(h, 0)?;
    let (entry_point, at) = if has_entry == 1 {
        let (e, at) = get_u32(h, at)?;
        (Some(e as usize), at)
    } else {
        (None, at)
    };
    let (word_bits_log2, at) = get_u8(h, at)?;
    let (addr_bits_log2, at) = get_u8(h, at)?;
    let (float_bits_log2, at) = get_u8(h, at)?;
    let (wcet_cycles, at) = get_u32(h, at)?;
    let (wcmu_bytes, at) = get_u32(h, at)?;
    let (flags, at) = get_u8(h, at)?;
    let (shared_data_bytes, at) = get_u32(h, at)?;
    let (private_data_bytes, at) = get_u32(h, at)?;
    let (schema_hash, _) = get_u32(h, at)?;

    let cr = require_region(b, region::CHUNKS)?;
    let (n, at) = get_u32(cr, 0)?;
    let table_bytes = (n as usize)
        .checked_mul(4)
        .ok_or(FlatAuxError::Truncated { at })?;
    if at + table_bytes > cr.len() {
        return Err(FlatAuxError::Truncated { at });
    }
    let mut at = at + table_bytes;
    let mut chunks = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (c, next) = get_chunk(cr, at)?;
        chunks.push(c);
        at = next;
    }

    let nr = require_region(b, region::NATIVE_NAMES)?;
    let (n, mut at) = get_u32(nr, 0)?;
    let mut native_names = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (s, next) = get_str(nr, at)?;
        native_names.push(s);
        at = next;
    }

    let data_layout = match find_region(b, region::DATA_LAYOUT)? {
        Some(dr) => Some(get_data_layout(dr, 0)?.0),
        None => None,
    };

    let er = require_region(b, region::ENUM_LAYOUTS)?;
    let (n, mut at) = get_u32(er, 0)?;
    let mut enum_layouts = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (e, next) = get_enum_layout(er, at)?;
        enum_layouts.push(e);
        at = next;
    }

    let sr = require_region(b, region::SIGNATURES)?;
    let (n, mut at) = get_u32(sr, 0)?;
    let mut signatures = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (sg, next) = get_chunk_signature(sr, at)?;
        signatures.push(sg);
        at = next;
    }

    let rr = require_region(b, region::NATIVE_RETURN_SHAPES)?;
    let (n, mut at) = get_u32(rr, 0)?;
    let mut native_return_shapes = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (sh, next) = get_wire_shape(rr, at)?;
        native_return_shapes.push(sh);
        at = next;
    }

    Ok(WireAuxBody {
        chunks,
        native_names,
        entry_point,
        data_layout,
        word_bits_log2,
        addr_bits_log2,
        float_bits_log2,
        wcet_cycles,
        wcmu_bytes,
        flags,
        shared_data_bytes,
        private_data_bytes,
        schema_hash,
        enum_layouts,
        signatures,
        native_return_shapes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_round_trips_are_exact() {
        let mut out = Vec::new();
        put_u8(&mut out, 0xAB);
        put_u16(&mut out, 0xBEEF);
        put_u32(&mut out, 0xDEAD_BEEF);
        put_i64(&mut out, -42);
        put_str(&mut out, "keleusma");
        put_bytes(&mut out, &[1, 2, 3]);

        let (a, at) = get_u8(&out, 0).unwrap();
        let (b, at) = get_u16(&out, at).unwrap();
        let (c, at) = get_u32(&out, at).unwrap();
        let (d, at) = get_i64(&out, at).unwrap();
        let (e, at) = get_str(&out, at).unwrap();
        let (f, at) = get_slice(&out, at).unwrap();
        assert_eq!((a, b, c, d), (0xAB, 0xBEEF, 0xDEAD_BEEF, -42));
        assert_eq!(e, "keleusma");
        assert_eq!(f, &[1, 2, 3]);
        assert_eq!(
            at,
            out.len(),
            "readers must consume exactly what was written"
        );
    }

    #[test]
    fn readers_reject_truncation_rather_than_panicking() {
        // Every reader is bounds-checked, so a hostile or corrupt buffer yields an
        // error. This is the property that lets the decoder run on untrusted bytes.
        let mut out = Vec::new();
        put_str(&mut out, "abcdef");
        for cut in 0..out.len() {
            let short = &out[..cut];
            assert!(
                get_str(short, 0).is_err(),
                "truncation at {cut} must be rejected"
            );
        }
    }

    #[test]
    fn leaf_enums_round_trip_every_variant() {
        for v in [BlockType::Func, BlockType::Reentrant, BlockType::Stream] {
            let mut out = Vec::new();
            put_block_type(&mut out, v);
            assert_eq!(get_block_type(&out, 0).unwrap().0, v);
        }
        for v in [
            TypeTag::Composite,
            TypeTag::Byte,
            TypeTag::Word,
            TypeTag::Fixed,
            TypeTag::Float,
            TypeTag::Bool,
            TypeTag::Unit,
            TypeTag::Text,
        ] {
            let mut out = Vec::new();
            put_type_tag(&mut out, v);
            assert_eq!(get_type_tag(&out, 0).unwrap().0, v);
        }
        for v in [SlotVisibility::Shared, SlotVisibility::Private] {
            let mut out = Vec::new();
            put_slot_visibility(&mut out, v);
            assert_eq!(get_slot_visibility(&out, 0).unwrap().0, v);
        }
    }

    #[test]
    fn unknown_discriminants_are_rejected() {
        // An unrecognised tag must be an error, not a silent default: this is what
        // makes a version-1 body or a corrupt region fail loudly.
        assert!(matches!(
            get_block_type(&[9], 0),
            Err(FlatAuxError::BadTag { .. })
        ));
        assert!(matches!(
            get_type_tag(&[9], 0),
            Err(FlatAuxError::BadTag { .. })
        ));
        assert!(matches!(
            get_const_value(&[99], 0),
            Err(FlatAuxError::BadTag { .. })
        ));
    }

    #[test]
    fn const_values_round_trip() {
        let values = [
            ConstValue::Unit,
            ConstValue::Bool(true),
            ConstValue::Bool(false),
            ConstValue::Int(i64::MIN),
            ConstValue::Int(i64::MAX),
            ConstValue::Byte(255),
            ConstValue::Fixed(-1),
            ConstValue::StaticStr(String::from("hello")),
        ];
        let mut out = Vec::new();
        for v in &values {
            put_const_value(&mut out, v);
        }
        let mut at = 0;
        for want in &values {
            let (got, next) = get_const_value(&out, at).unwrap();
            assert_eq!(&got, want);
            at = next;
        }
        assert_eq!(at, out.len());
    }

    #[test]
    fn struct_templates_round_trip_including_empty() {
        let templates = [
            StructTemplate {
                type_name: String::from("P"),
                field_names: alloc::vec![String::from("x"), String::from("y")],
            },
            StructTemplate {
                type_name: String::from("Empty"),
                field_names: Vec::new(),
            },
        ];
        let mut out = Vec::new();
        for t in &templates {
            put_struct_template(&mut out, t);
        }
        let mut at = 0;
        for want in &templates {
            let (got, next) = get_struct_template(&out, at).unwrap();
            assert_eq!(got.type_name, want.type_name);
            assert_eq!(got.field_names, want.field_names);
            at = next;
        }
        assert_eq!(at, out.len());
    }

    #[test]
    fn deep_constant_nesting_is_rejected_not_overflowed() {
        // A hostile buffer must not be able to drive the recursive constant decoder
        // into stack exhaustion. Build a nesting deeper than the cap by hand: each
        // level is a Tuple tag, a count of 1, then the next level.
        let mut out = Vec::new();
        let levels = (MAX_CONST_DEPTH as usize) + 8;
        for _ in 0..levels {
            put_u8(&mut out, 7);
            put_u32(&mut out, 1);
        }
        put_u8(&mut out, 0); // innermost Unit
        assert!(
            matches!(
                get_const_value(&out, 0),
                Err(FlatAuxError::DepthExceeded { .. })
            ),
            "nesting past the cap must be rejected"
        );
    }

    #[test]
    fn nesting_within_the_cap_still_round_trips() {
        // The guard must not reject legitimate nesting.
        let v = ConstValue::Tuple(alloc::vec![
            ConstValue::Array(alloc::vec![ConstValue::Int(1), ConstValue::Byte(2)]),
            ConstValue::Struct {
                type_name: String::from("P"),
                fields: alloc::vec![(String::from("x"), ConstValue::Bool(true))],
            },
            ConstValue::Enum {
                type_name: String::from("E"),
                variant: String::from("A"),
                discriminant: Some(3),
                fields: alloc::vec![ConstValue::None],
            },
        ]);
        let mut out = Vec::new();
        put_const_value(&mut out, &v);
        let (got, at) = get_const_value(&out, 0).unwrap();
        assert_eq!(got, v);
        assert_eq!(at, out.len());
    }

    #[test]
    fn empty_aux_round_trips() {
        // The degenerate module: no chunks, no natives, no data layout. Exercises
        // the region directory with an absent optional region.
        let aux = WireAuxBody {
            chunks: Vec::new(),
            native_names: Vec::new(),
            entry_point: None,
            data_layout: None,
            word_bits_log2: 6,
            addr_bits_log2: 6,
            float_bits_log2: 6,
            wcet_cycles: 0,
            wcmu_bytes: 0,
            flags: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
            schema_hash: 0,
            enum_layouts: Vec::new(),
            signatures: Vec::new(),
            native_return_shapes: Vec::new(),
        };
        let bytes = encode_aux(&aux);
        let got = decode_aux(&bytes).expect("decode");
        assert!(got.chunks.is_empty());
        assert!(got.data_layout.is_none());
        assert_eq!(got.word_bits_log2, 6);
    }

    #[test]
    fn a_populated_aux_round_trips_every_field() {
        let aux = WireAuxBody {
            chunks: alloc::vec![WireChunk {
                name: String::from("main"),
                constants: alloc::vec![
                    ConstValue::Int(7),
                    ConstValue::StaticStr(String::from("s")),
                    ConstValue::Tuple(alloc::vec![ConstValue::Bool(true)]),
                ],
                struct_templates: alloc::vec![StructTemplate {
                    type_name: String::from("P"),
                    field_names: alloc::vec![String::from("x")],
                }],
                local_count: 9,
                param_count: 2,
                block_type: BlockType::Stream,
                param_types: alloc::vec![TypeTag::Word, TypeTag::Byte],
                op_byte_offset: 16,
                op_record_count: 4,
                debug_pool_bytes: Some(alloc::vec![1, 2, 3]),
            }],
            native_names: alloc::vec![String::from("println")],
            entry_point: Some(3),
            data_layout: Some(DataLayout {
                slots: alloc::vec![DataSlot {
                    name: String::from("d"),
                    visibility: SlotVisibility::Private,
                }],
                shared_layout: alloc::vec![SharedSlotLayout {
                    offset: 8,
                    kind: 3,
                    len: 4,
                }],
                private_composite_layout: alloc::vec![PrivateCompositeSlot { slot: 1, offset: 2 }],
                private_init: alloc::vec![ConstValue::Byte(5)],
            }),
            word_bits_log2: 6,
            addr_bits_log2: 5,
            float_bits_log2: 6,
            wcet_cycles: 1234,
            wcmu_bytes: 5678,
            flags: 0x03,
            shared_data_bytes: 64,
            private_data_bytes: 128,
            schema_hash: 0xDEAD_BEEF,
            enum_layouts: alloc::vec![EnumLayout {
                type_name: String::from("E"),
                variants: alloc::vec![EnumVariantDisc {
                    name: String::from("A"),
                    disc: 0,
                }],
                min_payload: 8,
            }],
            signatures: alloc::vec![ChunkSignature {
                params: alloc::vec![WireShape::Scalar { kind: 3 }],
                ret: WireShape::Flat { kind: 2, size: 16 },
                resume: WireShape::Top,
            }],
            native_return_shapes: alloc::vec![WireShape::Top],
        };

        let bytes = encode_aux(&aux);
        let got = decode_aux(&bytes).expect("decode");

        assert_eq!(got.chunks.len(), 1);
        let c = &got.chunks[0];
        assert_eq!(c.name, "main");
        assert_eq!(c.constants.len(), 3);
        assert_eq!(c.constants[0], ConstValue::Int(7));
        assert_eq!(
            c.constants[2],
            ConstValue::Tuple(alloc::vec![ConstValue::Bool(true)])
        );
        assert_eq!(
            c.struct_templates[0].field_names,
            alloc::vec![String::from("x")]
        );
        assert_eq!((c.local_count, c.param_count), (9, 2));
        assert_eq!(c.block_type, BlockType::Stream);
        assert_eq!(c.param_types, alloc::vec![TypeTag::Word, TypeTag::Byte]);
        assert_eq!((c.op_byte_offset, c.op_record_count), (16, 4));
        assert_eq!(c.debug_pool_bytes.as_deref(), Some(&[1u8, 2, 3][..]));

        assert_eq!(got.native_names, alloc::vec![String::from("println")]);
        assert_eq!(got.entry_point, Some(3));
        let dl = got.data_layout.expect("data layout");
        assert_eq!(dl.slots[0].name, "d");
        assert_eq!(dl.slots[0].visibility, SlotVisibility::Private);
        assert_eq!(dl.shared_layout[0].offset, 8);
        assert_eq!(dl.private_composite_layout[0].slot, 1);
        assert_eq!(dl.private_init[0], ConstValue::Byte(5));
        assert_eq!(
            (got.word_bits_log2, got.addr_bits_log2, got.float_bits_log2),
            (6, 5, 6)
        );
        assert_eq!(
            (got.wcet_cycles, got.wcmu_bytes, got.flags),
            (1234, 5678, 0x03)
        );
        assert_eq!(
            (
                got.shared_data_bytes,
                got.private_data_bytes,
                got.schema_hash
            ),
            (64, 128, 0xDEAD_BEEF)
        );
        assert_eq!(got.enum_layouts[0].type_name, "E");
        assert_eq!(got.enum_layouts[0].variants[0].disc, 0);
        assert_eq!(got.signatures[0].ret, WireShape::Flat { kind: 2, size: 16 });
        assert_eq!(got.native_return_shapes[0], WireShape::Top);
    }

    #[test]
    fn a_truncated_aux_is_rejected_at_every_length() {
        // Totality on malformed input, at whole-body scale rather than per-field.
        let aux = WireAuxBody {
            chunks: alloc::vec![WireChunk {
                name: String::from("f"),
                constants: alloc::vec![ConstValue::Int(1)],
                struct_templates: Vec::new(),
                local_count: 1,
                param_count: 0,
                block_type: BlockType::Func,
                param_types: Vec::new(),
                op_byte_offset: 0,
                op_record_count: 1,
                debug_pool_bytes: None,
            }],
            native_names: Vec::new(),
            entry_point: None,
            data_layout: None,
            word_bits_log2: 6,
            addr_bits_log2: 6,
            float_bits_log2: 6,
            wcet_cycles: 0,
            wcmu_bytes: 0,
            flags: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
            schema_hash: 0,
            enum_layouts: Vec::new(),
            signatures: Vec::new(),
            native_return_shapes: Vec::new(),
        };
        let bytes = encode_aux(&aux);
        for cut in 0..bytes.len() {
            assert!(
                decode_aux(&bytes[..cut]).is_err(),
                "truncation to {cut} bytes must be rejected"
            );
        }
        assert!(decode_aux(&bytes).is_ok(), "the full buffer must decode");
    }

    #[test]
    fn a_version_one_rkyv_body_is_rejected_by_magic() {
        // The magic is what stops a version-1 body being misparsed as flat.
        assert!(matches!(
            decode_aux(&[0, 0, 0, 0, 0, 0, 0, 0]),
            Err(FlatAuxError::BadMagic { .. })
        ));
    }

    #[test]
    fn whole_body_encoding_is_deterministic() {
        let aux = WireAuxBody {
            chunks: alloc::vec![WireChunk {
                name: String::from("f"),
                constants: alloc::vec![ConstValue::Int(1), ConstValue::Unit],
                struct_templates: Vec::new(),
                local_count: 1,
                param_count: 0,
                block_type: BlockType::Func,
                param_types: Vec::new(),
                op_byte_offset: 0,
                op_record_count: 1,
                debug_pool_bytes: None,
            }],
            native_names: alloc::vec![String::from("n")],
            entry_point: Some(0),
            data_layout: None,
            word_bits_log2: 6,
            addr_bits_log2: 6,
            float_bits_log2: 6,
            wcet_cycles: 1,
            wcmu_bytes: 2,
            flags: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
            schema_hash: 7,
            enum_layouts: Vec::new(),
            signatures: Vec::new(),
            native_return_shapes: Vec::new(),
        };
        assert_eq!(encode_aux(&aux), encode_aux(&aux));
    }

    #[test]
    fn encoding_is_deterministic() {
        // The byte-identical oracle that gates the self-hosted compiler depends on
        // equal inputs producing equal bytes.
        let v = ConstValue::StaticStr(String::from("determinism"));
        let mut a = Vec::new();
        let mut b = Vec::new();
        put_const_value(&mut a, &v);
        put_const_value(&mut b, &v);
        assert_eq!(a, b);
    }
}
