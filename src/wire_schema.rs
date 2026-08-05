//! Keleusma's schema on top of the [`keleusma_wire`] container (wire format v2,
//! stage 1: the constant table).
//!
//! The container knows nothing about Keleusma. This module supplies the part it
//! deliberately omits: which region kinds exist, what each record means, and how a
//! [`ConstValue`](crate::bytecode::ConstValue) tree becomes fixed-size records.
//!
//! # What this stage covers
//!
//! The **constant table** and its supporting pools, which is the part of the
//! auxiliary body carrying the real design content. The remaining aux-body
//! fields — struct templates, param types, enum layouts, signatures, native
//! return shapes, the scalar header block — are flat vectors of scalars that
//! follow the same mechanical pattern and land in later stages. The `rkyv` path
//! is untouched; nothing here is wired into the loader yet.
//!
//! # The design point: recursion is removed, not merely bounded
//!
//! [`ConstValue`](crate::bytecode::ConstValue) is a tree. The superseded encoding nested
//! children inline, which forced a recursive decoder and a depth cap to stop hostile input
//! exhausting the stack. Here a composite instead references a **range** of
//! entries in the same table, and the flattening guarantees that range lies
//! strictly **after** the composite itself.
//!
//! That ordering is what makes the table walkable by a single reverse linear
//! sweep with no stack at all — and it is checked rather than assumed, because
//! its violation is silent: a backwards range makes a reverse sweep read entries
//! it has not computed yet, producing a wrong answer rather than a fault. The
//! encoder produces the ordering by construction (breadth-first numbering) and
//! [`decode_constants`](crate::wire_schema::decode_constants) re-validates it on the way
//! back in, since a decoder must not trust the encoder that produced its input.
//!
//! # Why side tables instead of wider records
//!
//! A struct constant needs a type name, field names, and field values; an enum
//! needs a type name, a variant name, an optional discriminant, and payload
//! values. Widening every constant record to fit the worst case would cost 32
//! bytes for an `Int` that needs 8. Instead the two composite kinds reference
//! small side tables, so the constant record stays two words and the space is
//! paid only by the constants that need it.

use alloc::string::String;
use alloc::vec::Vec;

use keleusma_wire::{Pool, RecordTable, WireBuilder, WireError, WireRecord, WireView};

use crate::bytecode::{ChunkSignature, ConstValue, WireShape};

/// Region kinds. Assigned explicitly rather than by declaration order, so
/// reordering this list cannot silently change an artifact's meaning.
pub mod kind {
    /// Flat bytes: every name and string constant, concatenated.
    pub const STRING_POOL: u16 = 0x0010;
    /// `(offset, length)` slices into [`STRING_POOL`].
    pub const NAMES: u16 = 0x0011;
    /// The flattened constant table.
    pub const CONSTS: u16 = 0x0012;
    /// Per-struct-constant type and field-name references.
    pub const STRUCT_AUX: u16 = 0x0013;
    /// Per-enum-constant type, variant, and discriminant.
    pub const ENUM_AUX: u16 = 0x0014;
    /// Flat operand shapes, referenced by index.
    pub const SHAPES: u16 = 0x0015;
    /// Per-chunk signature descriptors.
    pub const SIGNATURES: u16 = 0x0016;
}

/// Shape tags. Numbered from one so an all-zero record is invalid rather than
/// decoding as a valid `Top` — a zeroed region should not read as a well-formed
/// table of "shape unknown" entries.
pub mod shape_tag {
    #![allow(missing_docs)]
    pub const TOP: u16 = 1;
    pub const SCALAR: u16 = 2;
    pub const FLAT: u16 = 3;
}

/// Constant tags. Explicit, and never reordered: the numbering is the wire
/// contract. The float tag is reserved unconditionally so an artifact written by
/// a floats build fails loudly on a no-floats build rather than being misread.
pub mod tag {
    #![allow(missing_docs)]
    pub const UNIT: u16 = 1;
    pub const BOOL: u16 = 2;
    pub const INT: u16 = 3;
    pub const BYTE: u16 = 4;
    pub const FIXED: u16 = 5;
    pub const FLOAT: u16 = 6;
    pub const STATIC_STR: u16 = 7;
    pub const TUPLE: u16 = 8;
    pub const ARRAY: u16 = 9;
    pub const STRUCT: u16 = 10;
    pub const ENUM: u16 = 11;
    pub const NONE: u16 = 12;
}

/// Constant-record flag: an enum constant carries a resolved discriminant.
pub const FLAG_HAS_DISCRIMINANT: u16 = 1 << 0;

/// A slice of the string pool.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameRef {
    /// Byte offset into the string pool.
    pub offset: u32,
    /// Byte length.
    pub length: u32,
}

/// One constant. Two words: a tag word and a payload word.
///
/// The payload is read according to the tag — a scalar's bits, or a
/// `(first, count)` range into this same table for a composite.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstRecord {
    /// See [`tag`].
    pub tag: u16,
    /// See [`FLAG_HAS_DISCRIMINANT`].
    pub flags: u16,
    /// Name index for a string constant, or side-table index for a struct or
    /// enum. Zero and unused otherwise.
    pub aux: u32,
    /// Scalar bits, or a `(first, count)` range packed as two `u32`s.
    pub payload: u64,
}

/// Type and field names for one struct constant.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructAux {
    /// Name index of the struct's type name.
    pub type_name: u32,
    /// Name index of this struct's first field name; field *i* is at
    /// `field_names_first + i`.
    pub field_names_first: u32,
}

/// Type, variant, and discriminant for one enum constant.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumAux {
    /// Name index of the enum's type name.
    pub type_name: u32,
    /// Name index of the variant name.
    pub variant: u32,
    /// Resolved discriminant. Meaningful only when the constant record carries
    /// [`FLAG_HAS_DISCRIMINANT`]; a bare zero here is not the same as `Some(0)`.
    pub discriminant: i64,
}

impl ConstRecord {
    /// Packs a `(first, count)` range into the payload word.
    #[inline]
    fn range(first: u32, count: u32) -> u64 {
        first as u64 | ((count as u64) << 32)
    }

    /// Unpacks the payload as a `(first, count)` range.
    #[inline]
    pub fn as_range(&self) -> (u32, u32) {
        (self.payload as u32, (self.payload >> 32) as u32)
    }

    /// True when this constant's payload is a range into the constant table.
    #[inline]
    pub fn is_composite(&self) -> bool {
        matches!(self.tag, tag::TUPLE | tag::ARRAY | tag::STRUCT | tag::ENUM)
    }
}

/// Accumulates the string pool and its name table, sharing repeated names.
///
/// Sharing is not merely a size optimisation: type and field names repeat
/// heavily across a module's constants, and a shared pool is what lets a name be
/// compared by index rather than by bytes.
#[derive(Default)]
struct Names {
    pool: Vec<u8>,
    refs: Vec<NameRef>,
}

impl Names {
    fn intern(&mut self, s: &str) -> u32 {
        let bytes = s.as_bytes();
        // Linear scan. The name count per module is small, and a map would pull
        // in hashing for no measurable benefit at this size.
        for (i, r) in self.refs.iter().enumerate() {
            let at = r.offset as usize;
            if r.length as usize == bytes.len() && &self.pool[at..at + bytes.len()] == bytes {
                return i as u32;
            }
        }
        let offset = self.pool.len() as u32;
        self.pool.extend_from_slice(bytes);
        self.refs.push(NameRef {
            offset,
            length: bytes.len() as u32,
        });
        (self.refs.len() - 1) as u32
    }
}

/// The encoded constant table and its side tables.
#[derive(Default)]
struct Tables {
    consts: Vec<ConstRecord>,
    struct_aux: Vec<StructAux>,
    enum_aux: Vec<EnumAux>,
    names: Names,
}

/// Flattens a constant forest into the fixed-size tables.
///
/// The roots occupy indices `0..roots.len()` **in order**, because a chunk
/// indexes its constants by position. Children are numbered breadth-first after
/// them, which is what guarantees every range points forward.
fn flatten(roots: &[ConstValue]) -> Tables {
    let mut t = Tables::default();

    // Breadth-first. `queue` holds nodes whose records are not yet written;
    // `next_index` is the index the next unallocated child will take.
    let mut queue: Vec<&ConstValue> = roots.iter().collect();
    let mut next_index = roots.len() as u32;
    let mut head = 0usize;

    while head < queue.len() {
        let node = queue[head];
        head += 1;

        let record = match node {
            ConstValue::Unit => ConstRecord {
                tag: tag::UNIT,
                flags: 0,
                aux: 0,
                payload: 0,
            },
            ConstValue::None => ConstRecord {
                tag: tag::NONE,
                flags: 0,
                aux: 0,
                payload: 0,
            },
            ConstValue::Bool(b) => ConstRecord {
                tag: tag::BOOL,
                flags: 0,
                aux: 0,
                payload: *b as u64,
            },
            ConstValue::Int(v) => ConstRecord {
                tag: tag::INT,
                flags: 0,
                aux: 0,
                payload: *v as u64,
            },
            ConstValue::Byte(v) => ConstRecord {
                tag: tag::BYTE,
                flags: 0,
                aux: 0,
                payload: *v as u64,
            },
            ConstValue::Fixed(v) => ConstRecord {
                tag: tag::FIXED,
                flags: 0,
                aux: 0,
                payload: *v as u64,
            },
            #[cfg(feature = "floats")]
            ConstValue::Float(v) => ConstRecord {
                tag: tag::FLOAT,
                flags: 0,
                aux: 0,
                payload: v.to_bits(),
            },
            ConstValue::StaticStr(s) => {
                let idx = t.names.intern(s);
                ConstRecord {
                    tag: tag::STATIC_STR,
                    flags: 0,
                    aux: idx,
                    payload: 0,
                }
            }
            ConstValue::Tuple(items) | ConstValue::Array(items) => {
                let first = next_index;
                next_index += items.len() as u32;
                queue.extend(items.iter());
                ConstRecord {
                    tag: if matches!(node, ConstValue::Tuple(_)) {
                        tag::TUPLE
                    } else {
                        tag::ARRAY
                    },
                    flags: 0,
                    aux: 0,
                    payload: ConstRecord::range(first, items.len() as u32),
                }
            }
            ConstValue::Struct { type_name, fields } => {
                let type_idx = t.names.intern(type_name);
                // Field names are interned contiguously so field `i` is at
                // `field_names_first + i` -- one index instead of one per field.
                let names_first = t.names.refs.len() as u32;
                for (name, _) in fields {
                    t.names.intern_fresh(name);
                }
                let aux_idx = t.struct_aux.len() as u32;
                t.struct_aux.push(StructAux {
                    type_name: type_idx,
                    field_names_first: names_first,
                });

                let first = next_index;
                next_index += fields.len() as u32;
                queue.extend(fields.iter().map(|(_, v)| v));
                ConstRecord {
                    tag: tag::STRUCT,
                    flags: 0,
                    aux: aux_idx,
                    payload: ConstRecord::range(first, fields.len() as u32),
                }
            }
            ConstValue::Enum {
                type_name,
                variant,
                discriminant,
                fields,
            } => {
                let type_idx = t.names.intern(type_name);
                let variant_idx = t.names.intern(variant);
                let aux_idx = t.enum_aux.len() as u32;
                t.enum_aux.push(EnumAux {
                    type_name: type_idx,
                    variant: variant_idx,
                    discriminant: discriminant.unwrap_or(0),
                });

                let first = next_index;
                next_index += fields.len() as u32;
                queue.extend(fields.iter());
                ConstRecord {
                    tag: tag::ENUM,
                    flags: if discriminant.is_some() {
                        FLAG_HAS_DISCRIMINANT
                    } else {
                        0
                    },
                    aux: aux_idx,
                    payload: ConstRecord::range(first, fields.len() as u32),
                }
            }
        };

        t.consts.push(record);
    }

    t
}

impl Names {
    /// Interns without sharing, so a run of field names stays contiguous.
    ///
    /// Sharing would be correct for the bytes but would break the
    /// `field_names_first + i` addressing, since a repeated name would return an
    /// earlier index and interrupt the run.
    fn intern_fresh(&mut self, s: &str) -> u32 {
        let bytes = s.as_bytes();
        let offset = self.pool.len() as u32;
        self.pool.extend_from_slice(bytes);
        self.refs.push(NameRef {
            offset,
            length: bytes.len() as u32,
        });
        (self.refs.len() - 1) as u32
    }
}

/// Encodes a constant forest into a container artifact.
///
/// # Errors
///
/// Propagates a [`WireError`] from the container builder.
pub fn encode_constants(roots: &[ConstValue]) -> Result<Vec<u8>, WireError> {
    let mut b = WireBuilder::new();
    add_constant_regions(&mut b, roots)?;
    b.finish()
}

/// Adds the constant-table regions to an existing builder.
///
/// Separate from [`encode_constants`] so the aux body can eventually be one
/// artifact carrying every region, rather than several artifacts stitched
/// together later. Building the composability in now costs nothing; retrofitting
/// it would mean rewriting each encoder.
///
/// # Errors
///
/// Propagates a [`WireError`] from the builder.
pub fn add_constant_regions(b: &mut WireBuilder, roots: &[ConstValue]) -> Result<(), WireError> {
    let t = flatten(roots);

    let pool = b.region(kind::STRING_POOL, 0)?;
    let names = b.region(kind::NAMES, 0)?;
    let consts = b.region(kind::CONSTS, 0)?;
    let saux = b.region(kind::STRUCT_AUX, 0)?;
    let eaux = b.region(kind::ENUM_AUX, 0)?;

    b.push(pool, &t.names.pool);
    for r in &t.names.refs {
        b.push_record(names, r);
    }
    for r in &t.consts {
        b.push_record(consts, r);
    }
    for r in &t.struct_aux {
        b.push_record(saux, r);
    }
    for r in &t.enum_aux {
        b.push_record(eaux, r);
    }

    Ok(())
}

/// Why a constant table could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SchemaError {
    /// The container itself was malformed.
    Container(WireError),
    /// A required region is absent.
    MissingRegion(u16),
    /// A record index lies outside its table.
    BadIndex,
    /// A composite's range does not lie strictly after it, or overruns the
    /// table. **Silent if unchecked** — see the module documentation.
    BadRange,
    /// A tag is not one this build understands. A floats-built artifact read by
    /// a no-floats build lands here rather than being misread.
    UnknownTag(u16),
    /// A name slice lies outside the string pool, or is not valid UTF-8.
    BadName,
}

impl From<WireError> for SchemaError {
    fn from(e: WireError) -> Self {
        Self::Container(e)
    }
}

/// A **borrowed, allocation-free** view over an encoded constant table.
///
/// This is the accessor the runtime wants, as distinct from [`decode_constants`],
/// which materialises owned values for tooling and tests.
///
/// # The property this exists to preserve
///
/// A probe of the live runtime (2026-08-04) established what actually needs to be
/// borrowed, which is narrower than "everything":
///
/// - A **non-empty top-level string constant** is loaded by minting a `KString`
///   directly over the bytecode image's bytes — zero-copy, no per-load
///   allocation. [`Self::str_bytes`] is the accessor that keeps that possible: it
///   returns a slice **into the artifact**, so the pointer stays mintable.
/// - An **empty** string is deliberately *not* aliased by the runtime, so that it
///   need not rest on a non-null guarantee for a zero-length pointer.
/// - A **composite's** string leaves are already copied today, materialising as
///   owned values before the flat packer moves them into the arena. Borrowing
///   them buys nothing the runtime uses.
///
/// So the hard requirement is exactly one accessor returning image-aliasing
/// bytes; the rest may return values by copy, because scalars are registers and
/// composites already copy. Stating it this precisely matters — over-constraining
/// the accessor would have complicated it for no gain, and under-constraining it
/// would have silently cost the one property that is load-bearing.
///
/// Validation happens once in [`Self::parse`], so every accessor afterwards is
/// total and needs no further checking.
#[derive(Debug, Clone, Copy)]
pub struct ConstTable<'a> {
    pool: Pool<'a>,
    names: RecordTable<'a>,
    consts: RecordTable<'a>,
    struct_aux: RecordTable<'a>,
    enum_aux: RecordTable<'a>,
}

impl<'a> ConstTable<'a> {
    /// Parses and validates an encoded constant table.
    ///
    /// The ordering invariant is checked here, once, rather than on each access:
    /// a composite's range must lie strictly after it. See the module docs for
    /// why an unchecked violation is a wrong answer rather than a fault.
    ///
    /// # Errors
    ///
    /// [`SchemaError`] for any malformed artifact. Never panics.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, SchemaError> {
        let view = WireView::parse(bytes)?;

        let region = |k: u16| view.find_region(k).ok_or(SchemaError::MissingRegion(k));

        let table = Self {
            pool: view.pool(&region(kind::STRING_POOL)?)?,
            names: view.typed_records::<NameRef>(&region(kind::NAMES)?)?,
            consts: view.typed_records::<ConstRecord>(&region(kind::CONSTS)?)?,
            struct_aux: view.typed_records::<StructAux>(&region(kind::STRUCT_AUX)?)?,
            enum_aux: view.typed_records::<EnumAux>(&region(kind::ENUM_AUX)?)?,
        };
        table.validate_ordering()?;
        Ok(table)
    }

    /// Rejects any composite whose range is not strictly forward or overruns.
    fn validate_ordering(&self) -> Result<(), SchemaError> {
        for i in 0..self.len() {
            let rec = self.record(i).ok_or(SchemaError::BadIndex)?;
            if rec.is_composite() {
                let (first, n) = rec.as_range();
                if !self.consts.range_is_forward(i, first, n) {
                    return Err(SchemaError::BadRange);
                }
            }
        }
        Ok(())
    }

    /// Number of entries, roots and children together.
    #[inline]
    pub fn len(&self) -> usize {
        self.consts.len()
    }

    /// True when the table holds no constants.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The raw record at `index`.
    #[inline]
    pub fn record(&self, index: usize) -> Option<ConstRecord> {
        self.consts.get_as::<ConstRecord>(index)
    }

    /// The tag at `index`. See [`tag`].
    #[inline]
    pub fn tag(&self, index: usize) -> Option<u16> {
        Some(self.record(index)?.tag)
    }

    /// **The image-aliasing accessor.** Bytes of a string constant, as a slice
    /// into the artifact.
    ///
    /// Returns `None` when `index` is out of range or is not a string. The
    /// returned slice aliases the caller's buffer, which is what allows the
    /// runtime to mint a handle over it rather than copying.
    #[inline]
    pub fn str_bytes(&self, index: usize) -> Option<&'a [u8]> {
        let rec = self.record(index)?;
        if rec.tag != tag::STATIC_STR {
            return None;
        }
        self.name_bytes(rec.aux)
    }

    /// [`Self::str_bytes`] validated as UTF-8. Still borrowed; still no copy.
    #[inline]
    pub fn str(&self, index: usize) -> Option<&'a str> {
        core::str::from_utf8(self.str_bytes(index)?).ok()
    }

    /// Bytes of name `index`, as a slice into the artifact.
    #[inline]
    pub fn name_bytes(&self, index: u32) -> Option<&'a [u8]> {
        let r = self.names.get_as::<NameRef>(index as usize)?;
        self.pool.slice(r.offset, r.length)
    }

    /// A scalar constant's bits, whatever its width. The caller interprets them
    /// by tag, exactly as the record does.
    #[inline]
    pub fn payload(&self, index: usize) -> Option<u64> {
        Some(self.record(index)?.payload)
    }

    /// The `(first, count)` range of a composite, without materialising it.
    ///
    /// Validated forward at parse time, so a caller may index the range
    /// directly.
    #[inline]
    pub fn range(&self, index: usize) -> Option<(u32, u32)> {
        let rec = self.record(index)?;
        if !rec.is_composite() {
            return None;
        }
        Some(rec.as_range())
    }

    /// Type and field-name references for a struct constant.
    #[inline]
    pub fn struct_aux(&self, index: usize) -> Option<StructAux> {
        let rec = self.record(index)?;
        if rec.tag != tag::STRUCT {
            return None;
        }
        self.struct_aux.get_as::<StructAux>(rec.aux as usize)
    }

    /// Type, variant, and discriminant for an enum constant.
    ///
    /// The discriminant is `None` unless the record carries
    /// [`FLAG_HAS_DISCRIMINANT`]; a stored zero is not the same as `Some(0)`.
    #[inline]
    pub fn enum_aux(&self, index: usize) -> Option<(EnumAux, Option<i64>)> {
        let rec = self.record(index)?;
        if rec.tag != tag::ENUM {
            return None;
        }
        let aux = self.enum_aux.get_as::<EnumAux>(rec.aux as usize)?;
        let disc = if rec.flags & FLAG_HAS_DISCRIMINANT != 0 {
            Some(aux.discriminant)
        } else {
            None
        };
        Some((aux, disc))
    }
}

/// Decodes a constant forest, re-validating the ordering invariant.
///
/// `count` is the number of roots, which the caller knows from the chunk
/// metadata. Roots occupy indices `0..count`.
///
/// # Errors
///
/// [`SchemaError`] for any malformed artifact. This function never panics.
pub fn decode_constants(bytes: &[u8], count: usize) -> Result<Vec<ConstValue>, SchemaError> {
    // One parse-and-validate path, shared with the borrowed accessor. Keeping a
    // second copy here would let the owned and borrowed readers drift apart, and
    // a drift in the ordering check is exactly the silent-wrong-answer class this
    // format is shaped to avoid.
    let t = ConstTable::parse(bytes)?;

    if count > t.len() {
        return Err(SchemaError::BadIndex);
    }

    let name_of = |idx: u32| -> Result<String, SchemaError> {
        let slice = t.name_bytes(idx).ok_or(SchemaError::BadName)?;
        core::str::from_utf8(slice)
            .map(String::from)
            .map_err(|_| SchemaError::BadName)
    };

    // Bottom-up by a single REVERSE LINEAR SWEEP. Every child has a higher index
    // than its parent -- validated by `ConstTable::parse` -- so by the time index
    // `i` is reached, everything it references is already built. No stack, no
    // recursion, and the trip count is the table length.
    let mut built: Vec<Option<ConstValue>> = (0..t.len()).map(|_| None).collect();
    for i in (0..t.len()).rev() {
        let rec = t.record(i).ok_or(SchemaError::BadIndex)?;
        let (first, n) = rec.as_range();

        let children = |b: &mut Vec<Option<ConstValue>>| -> Result<Vec<ConstValue>, SchemaError> {
            let mut out = Vec::with_capacity(n as usize);
            for k in 0..n as usize {
                out.push(b[first as usize + k].take().ok_or(SchemaError::BadRange)?);
            }
            Ok(out)
        };

        let value = match rec.tag {
            tag::UNIT => ConstValue::Unit,
            tag::NONE => ConstValue::None,
            tag::BOOL => ConstValue::Bool(rec.payload != 0),
            tag::INT => ConstValue::Int(rec.payload as i64),
            tag::BYTE => ConstValue::Byte(rec.payload as u8),
            tag::FIXED => ConstValue::Fixed(rec.payload as i64),
            #[cfg(feature = "floats")]
            tag::FLOAT => ConstValue::Float(f64::from_bits(rec.payload)),
            tag::STATIC_STR => ConstValue::StaticStr(name_of(rec.aux)?),
            tag::TUPLE => ConstValue::Tuple(children(&mut built)?),
            tag::ARRAY => ConstValue::Array(children(&mut built)?),
            tag::STRUCT => {
                let aux = t.struct_aux(i).ok_or(SchemaError::BadIndex)?;
                let values = children(&mut built)?;
                let mut fields = Vec::with_capacity(values.len());
                for (k, v) in values.into_iter().enumerate() {
                    fields.push((name_of(aux.field_names_first + k as u32)?, v));
                }
                ConstValue::Struct {
                    type_name: name_of(aux.type_name)?,
                    fields,
                }
            }
            tag::ENUM => {
                let (aux, discriminant) = t.enum_aux(i).ok_or(SchemaError::BadIndex)?;
                ConstValue::Enum {
                    type_name: name_of(aux.type_name)?,
                    variant: name_of(aux.variant)?,
                    discriminant,
                    fields: children(&mut built)?,
                }
            }
            other => return Err(SchemaError::UnknownTag(other)),
        };

        built[i] = Some(value);
    }

    let mut roots = Vec::with_capacity(count);
    for slot in built.iter_mut().take(count) {
        roots.push(slot.take().ok_or(SchemaError::BadIndex)?);
    }
    Ok(roots)
}

// ---------------------------------------------------------------------------
// Shapes and signatures (stage 2b, increment 1)
// ---------------------------------------------------------------------------

/// One flat operand shape. A single word.
///
/// [`WireShape`] is a tagged union whose widest variant carries a `u8` and a
/// `u32`, so the whole thing fits one word with room to spare — no side table
/// needed, unlike the struct and enum constants.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeRecord {
    /// See [`shape_tag`].
    pub tag: u16,
    /// `ScalarKind::to_tag` or `CompositeKind::to_tag` code. Zero for `Top`.
    pub kind: u8,
    /// Reserved; keeps `size` at a fixed offset.
    pub reserved: u8,
    /// Flat body byte length for `Flat`. Zero otherwise.
    pub size: u32,
}

/// One chunk's signature: a range of parameter shapes plus two single shapes.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureRecord {
    /// Index of this chunk's first parameter shape.
    pub params_first: u32,
    /// Number of parameter shapes.
    pub params_count: u32,
    /// Shape index of the return value.
    pub ret: u32,
    /// Shape index of what a `Yield`/resume pushes.
    pub resume: u32,
}

impl ShapeRecord {
    fn of(s: &WireShape) -> Self {
        match s {
            WireShape::Top => Self {
                tag: shape_tag::TOP,
                kind: 0,
                reserved: 0,
                size: 0,
            },
            WireShape::Scalar { kind } => Self {
                tag: shape_tag::SCALAR,
                kind: *kind,
                reserved: 0,
                size: 0,
            },
            WireShape::Flat { kind, size } => Self {
                tag: shape_tag::FLAT,
                kind: *kind,
                reserved: 0,
                size: *size,
            },
        }
    }

    /// Decodes back to a [`WireShape`], or `None` for an unrecognised tag.
    pub fn to_shape(self) -> Option<WireShape> {
        match self.tag {
            shape_tag::TOP => Some(WireShape::Top),
            shape_tag::SCALAR => Some(WireShape::Scalar { kind: self.kind }),
            shape_tag::FLAT => Some(WireShape::Flat {
                kind: self.kind,
                size: self.size,
            }),
            _ => None,
        }
    }
}

/// Accumulates the shape table.
///
/// Two admission modes, mirroring the names table and for the same reason: a
/// parameter run must be **contiguous** so `params_first + i` addresses it, while
/// a single `ret` or `resume` reference may be **shared**. `Top` dominates real
/// modules — every non-Stream chunk resumes with it — so sharing the singles is
/// worth having.
#[derive(Default)]
struct Shapes {
    recs: Vec<ShapeRecord>,
}

impl Shapes {
    /// Appends without sharing, keeping a run contiguous.
    fn append(&mut self, s: &WireShape) -> u32 {
        self.recs.push(ShapeRecord::of(s));
        (self.recs.len() - 1) as u32
    }

    /// Reuses an identical entry if one exists.
    fn intern(&mut self, s: &WireShape) -> u32 {
        let want = ShapeRecord::of(s);
        for (i, r) in self.recs.iter().enumerate() {
            if *r == want {
                return i as u32;
            }
        }
        self.append(s)
    }
}

/// Encodes per-chunk signatures into an artifact.
///
/// # Errors
///
/// Propagates a [`WireError`] from the builder.
pub fn encode_signatures(sigs: &[ChunkSignature]) -> Result<Vec<u8>, WireError> {
    let mut b = WireBuilder::new();
    add_signature_regions(&mut b, sigs)?;
    b.finish()
}

/// Adds the shape and signature regions to an existing builder.
///
/// # Errors
///
/// Propagates a [`WireError`] from the builder.
pub fn add_signature_regions(
    b: &mut WireBuilder,
    sigs: &[ChunkSignature],
) -> Result<(), WireError> {
    let mut shapes = Shapes::default();
    let mut records = Vec::with_capacity(sigs.len());

    for sig in sigs {
        // Parameters first and unshared, so the run stays contiguous.
        let params_first = shapes.recs.len() as u32;
        for p in &sig.params {
            shapes.append(p);
        }
        // Singles may share. Interning after the run is safe: shapes reference
        // nothing, so unlike the constant table there is no ordering invariant.
        let ret = shapes.intern(&sig.ret);
        let resume = shapes.intern(&sig.resume);

        records.push(SignatureRecord {
            params_first,
            params_count: sig.params.len() as u32,
            ret,
            resume,
        });
    }

    let shape_region = b.region(kind::SHAPES, 0)?;
    let sig_region = b.region(kind::SIGNATURES, 0)?;
    for r in &shapes.recs {
        b.push_record(shape_region, r);
    }
    for r in &records {
        b.push_record(sig_region, r);
    }
    Ok(())
}

/// A **borrowed, allocation-free** view over an encoded signature table.
#[derive(Debug, Clone, Copy)]
pub struct SignatureTable<'a> {
    shapes: RecordTable<'a>,
    sigs: RecordTable<'a>,
}

impl<'a> SignatureTable<'a> {
    /// Parses and validates a signature artifact.
    ///
    /// Every parameter range is checked in bounds here, once, so later accessors
    /// are total. Note there is **no forward-ordering rule** to enforce: a shape
    /// references no other shape, so the recursion the constant table had to
    /// linearise simply does not arise.
    ///
    /// # Errors
    ///
    /// [`SchemaError`] for any malformed artifact. Never panics.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, SchemaError> {
        let view = WireView::parse(bytes)?;
        let region = |k: u16| view.find_region(k).ok_or(SchemaError::MissingRegion(k));

        let t = Self {
            shapes: view.typed_records::<ShapeRecord>(&region(kind::SHAPES)?)?,
            sigs: view.typed_records::<SignatureRecord>(&region(kind::SIGNATURES)?)?,
        };

        for i in 0..t.len() {
            let rec = t.record(i).ok_or(SchemaError::BadIndex)?;
            let end = (rec.params_first as usize)
                .checked_add(rec.params_count as usize)
                .ok_or(SchemaError::BadIndex)?;
            // Plain bounds, no `max(1)` fudge: with an empty shape table a
            // signature referencing shape 0 is malformed, and letting it through
            // would leave the accessors returning `None` instead of being total.
            if end > t.shapes.len()
                || rec.ret as usize >= t.shapes.len()
                || rec.resume as usize >= t.shapes.len()
            {
                return Err(SchemaError::BadIndex);
            }
        }
        Ok(t)
    }

    /// Number of chunk signatures.
    #[inline]
    pub fn len(&self) -> usize {
        self.sigs.len()
    }

    /// True when there are no signatures.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The raw signature record for chunk `index`.
    #[inline]
    pub fn record(&self, index: usize) -> Option<SignatureRecord> {
        self.sigs.get_as::<SignatureRecord>(index)
    }

    /// The shape at `index` in the shape table.
    #[inline]
    pub fn shape(&self, index: u32) -> Option<WireShape> {
        self.shapes
            .get_as::<ShapeRecord>(index as usize)?
            .to_shape()
    }

    /// Parameter `param` of chunk `index`.
    #[inline]
    pub fn param_shape(&self, index: usize, param: usize) -> Option<WireShape> {
        let rec = self.record(index)?;
        if param >= rec.params_count as usize {
            return None;
        }
        self.shape(rec.params_first + param as u32)
    }

    /// The return shape of chunk `index`.
    #[inline]
    pub fn ret_shape(&self, index: usize) -> Option<WireShape> {
        self.shape(self.record(index)?.ret)
    }

    /// The resume shape of chunk `index`.
    #[inline]
    pub fn resume_shape(&self, index: usize) -> Option<WireShape> {
        self.shape(self.record(index)?.resume)
    }
}

/// Decodes per-chunk signatures into owned values.
///
/// The tooling counterpart to [`SignatureTable`], as [`decode_constants`] is to
/// [`ConstTable`]. Built on the same parse path so the two cannot drift.
///
/// # Errors
///
/// [`SchemaError`] for any malformed artifact.
pub fn decode_signatures(bytes: &[u8]) -> Result<Vec<ChunkSignature>, SchemaError> {
    let t = SignatureTable::parse(bytes)?;
    let mut out = Vec::with_capacity(t.len());
    for i in 0..t.len() {
        let rec = t.record(i).ok_or(SchemaError::BadIndex)?;
        let mut params = Vec::with_capacity(rec.params_count as usize);
        for p in 0..rec.params_count as usize {
            params.push(t.param_shape(i, p).ok_or(SchemaError::UnknownTag(0))?);
        }
        out.push(ChunkSignature {
            params,
            ret: t.ret_shape(i).ok_or(SchemaError::UnknownTag(0))?,
            resume: t.resume_shape(i).ok_or(SchemaError::UnknownTag(0))?,
        });
    }
    Ok(out)
}
