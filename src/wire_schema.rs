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

/// Convenience: a record type's stride as a plain const expression.
trait StrideBytes {
    const STRIDE_BYTES: usize;
}
impl<T: WireRecord> StrideBytes for T {
    const STRIDE_BYTES: usize = <T as WireRecord>::STRIDE;
}

use crate::bytecode::{
    BlockType, ChunkSignature, ConstValue, DataLayout, EnumLayout, StructTemplate, TypeTag,
    WireShape,
};

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
    /// Struct construction templates.
    pub const STRUCT_TEMPLATES: u16 = 0x0017;
    /// Enum variant names and discriminants, referenced by range.
    pub const ENUM_VARIANTS: u16 = 0x0018;
    /// Per-enum-type layout descriptors.
    pub const ENUM_LAYOUTS: u16 = 0x0019;
    /// Data-segment slots. **Presence of this region is what distinguishes a
    /// module with a data layout from one without**, so an absent region means
    /// `None` while an empty one means `Some` with no slots.
    pub const DATA_SLOTS: u16 = 0x001A;
    /// Per-shared-slot byte layout in the host buffer.
    pub const SHARED_LAYOUT: u16 = 0x001B;
    /// Persistent-pool placement of private composite slots.
    pub const PRIVATE_COMPOSITE: u16 = 0x001C;
    /// The private-slot initialiser range into the constant table.
    pub const DATA_INIT: u16 = 0x001D;
    /// Flat bytes: every chunk's parameter type tags, concatenated.
    ///
    /// A byte pool rather than a record table because a type tag is one byte and
    /// a whole-word record per tag would waste seven eighths of the region.
    pub const PARAM_TYPES: u16 = 0x001E;
    /// Per-chunk metadata.
    pub const CHUNKS: u16 = 0x001F;
    /// Native function names paired with their return shapes.
    pub const NATIVES: u16 = 0x0020;
    /// The module's scalar header fields.
    pub const HEADER: u16 = 0x0021;
    /// Flat bytes: every chunk's strippable debug metadata, concatenated.
    pub const DEBUG_POOL: u16 = 0x0022;
    /// Native return shapes, as shape indices.
    ///
    /// **Separate from [`NATIVES`] deliberately.** These were first encoded
    /// paired with the names in one record, on the reasoning that two parallel
    /// vectors are exactly the arrangement that falls out of step. That is true
    /// — but they are ALREADY allowed to differ in length, and pairing them
    /// silently DROPPED the surplus rather than preventing it. A round-trip test
    /// with two shapes and no names caught it. Independent regions carry both
    /// lengths, which is what fidelity requires.
    pub const NATIVE_RETURNS: u16 = 0x0023;
}

/// Block-type tags. Numbered from one so a zeroed record is invalid.
pub mod block_tag {
    #![allow(missing_docs)]
    pub const FUNC: u8 = 1;
    pub const REENTRANT: u8 = 2;
    pub const STREAM: u8 = 3;
}

/// Sentinel for an absent optional index or offset.
///
/// Used for `entry_point`, a native's return shape, and a chunk's debug pool.
/// A sentinel rather than a parallel flag because these are indices into tables
/// the container already bounds far below four billion entries, so the value is
/// unreachable in a well-formed artifact — and a flag would have to be kept in
/// step with the field it describes, which is one more thing to get wrong.
pub const ABSENT: u32 = u32::MAX;

/// Parameter type tags. Numbered from one so a zeroed byte is invalid.
pub mod type_tag {
    #![allow(missing_docs)]
    pub const COMPOSITE: u8 = 1;
    pub const BYTE: u8 = 2;
    pub const WORD: u8 = 3;
    pub const FIXED: u8 = 4;
    pub const FLOAT: u8 = 5;
    pub const BOOL: u8 = 6;
    pub const UNIT: u8 = 7;
    pub const TEXT: u8 = 8;
}

/// Encodes a [`TypeTag`] as its wire byte.
pub fn type_tag_byte(t: TypeTag) -> u8 {
    match t {
        TypeTag::Composite => type_tag::COMPOSITE,
        TypeTag::Byte => type_tag::BYTE,
        TypeTag::Word => type_tag::WORD,
        TypeTag::Fixed => type_tag::FIXED,
        TypeTag::Float => type_tag::FLOAT,
        TypeTag::Bool => type_tag::BOOL,
        TypeTag::Unit => type_tag::UNIT,
        TypeTag::Text => type_tag::TEXT,
    }
}

/// Decodes a wire byte back to a [`TypeTag`], or `None` if unrecognised.
pub fn type_tag_from_byte(b: u8) -> Option<TypeTag> {
    Some(match b {
        type_tag::COMPOSITE => TypeTag::Composite,
        type_tag::BYTE => TypeTag::Byte,
        type_tag::WORD => TypeTag::Word,
        type_tag::FIXED => TypeTag::Fixed,
        type_tag::FLOAT => TypeTag::Float,
        type_tag::BOOL => TypeTag::Bool,
        type_tag::UNIT => TypeTag::Unit,
        type_tag::TEXT => TypeTag::Text,
        _ => return None,
    })
}

/// Slot visibility tags. Numbered from one so a zeroed record is invalid.
pub mod visibility_tag {
    #![allow(missing_docs)]
    pub const SHARED: u8 = 1;
    pub const PRIVATE: u8 = 2;
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
///
/// # Why the index exists
///
/// This began as a linear scan, justified in a comment as "the name count per
/// module is small". **That was wrong, and measurement killed it**: the
/// self-hosted stage sources declare thousands of data slots each — 16913 in one
/// case — and every slot name is interned. A scan makes interning quadratic, and
/// encoding a mid-sized stage went from under a second to over nine minutes as
/// the count grew. A `BTreeMap` keeps it logarithmic without pulling in a hasher,
/// which matters for a `no_std` crate.
#[derive(Default)]
struct Names {
    pool: Vec<u8>,
    refs: Vec<NameRef>,
    /// Name bytes to index, for `intern`. `intern_fresh` deliberately bypasses
    /// lookup but still records the entry, so a later `intern` can share it.
    index: alloc::collections::BTreeMap<alloc::vec::Vec<u8>, u32>,
}

impl Names {
    fn intern(&mut self, s: &str) -> u32 {
        if let Some(i) = self.index.get(s.as_bytes()) {
            return *i;
        }
        self.intern_fresh(s)
    }
}

/// The encoded constant table and its side tables.
#[derive(Default)]
struct Tables {
    consts: Vec<ConstRecord>,
    struct_aux: Vec<StructAux>,
    enum_aux: Vec<EnumAux>,
}

/// Flattens a constant forest into the fixed-size tables.
///
/// The roots occupy indices `0..roots.len()` **in order**, because a chunk
/// indexes its constants by position. Children are numbered breadth-first after
/// them, which is what guarantees every range points forward.
fn flatten(roots: &[ConstValue], names: &mut Names) -> Tables {
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
                let idx = names.intern(s);
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
                let type_idx = names.intern(type_name);
                // Field names are interned contiguously so field `i` is at
                // `field_names_first + i` -- one index instead of one per field.
                let names_first = names.refs.len() as u32;
                for (name, _) in fields {
                    names.intern_fresh(name);
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
                let type_idx = names.intern(type_name);
                let variant_idx = names.intern(variant);
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
        let idx = (self.refs.len() - 1) as u32;
        // Record it for `intern` to share later. Overwriting an existing mapping
        // is harmless: any index naming the same bytes is equally correct.
        self.index.insert(bytes.to_vec(), idx);
        idx
    }
}

/// Encodes a constant forest into a container artifact.
///
/// # Errors
///
/// Propagates a [`WireError`] from the container builder.
pub fn encode_constants(roots: &[ConstValue]) -> Result<Vec<u8>, WireError> {
    let mut b = SchemaBuilder::new();
    b.add_constants(roots)?;
    b.finish()
}

/// One struct construction template.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructTemplateRecord {
    /// Name index of the struct's type name.
    pub type_name: u32,
    /// Name index of the first field name; field *i* is at
    /// `field_names_first + i`.
    pub field_names_first: u32,
    /// Number of fields.
    pub field_count: u32,
    /// Reserved; keeps the record two whole words.
    pub reserved: u32,
}

/// One enum variant: its name and discriminant.
///
/// A bare run of names could not carry the discriminants, so variants get their
/// own table rather than riding the name table directly the way struct fields do.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumVariantRecord {
    /// Name index of the variant name.
    pub name: u32,
    /// Reserved; keeps `disc` at a fixed offset.
    pub reserved: u32,
    /// Variant discriminant.
    pub disc: i64,
}

/// One enum type's layout descriptor.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumLayoutRecord {
    /// Name index of the enum's type name.
    pub type_name: u32,
    /// Index of this layout's first variant record.
    pub variants_first: u32,
    /// Number of variants.
    pub variants_count: u32,
    /// Largest-variant payload size in bytes; zero for a non-flat enum.
    pub min_payload: u32,
}

/// Assembles the aux body, owning the state that must be shared across regions.
///
/// # Why this exists rather than free `add_*_regions` functions
///
/// The **name interner is shared**. Constants, struct templates and enum layouts
/// all reference names, but the string pool and its index table are single
/// regions, and the container rejects a duplicate region kind — so a per-concern
/// encoder that declared its own `NAMES` would collide with the first one that
/// ran. Interning centrally also means a type name mentioned by a constant and by
/// a template is stored once and comparable by index.
///
/// Region kinds are declared as each concern is added; the pool and name table
/// are emitted at [`Self::finish`], once every contributor has interned.
#[derive(Default)]
pub struct SchemaBuilder {
    b: WireBuilder,
    names: Names,
    /// Every contributor's constant roots, concatenated. Flattening is deferred
    /// to [`SchemaBuilder::finish`] so all pools share one table.
    const_roots: Vec<ConstValue>,
    /// Set once any contributor asks for a constant pool, so an artifact with no
    /// constants emits no constant regions rather than three empty ones.
    wants_constants: bool,
    /// Every contributor's struct templates, concatenated. Deferred for the same
    /// reason constants are: templates are declared **per chunk**, so the table
    /// serves many contributors and each needs a range rather than the whole of it.
    template_pool: Vec<StructTemplate>,
    wants_templates: bool,
    /// Concatenated parameter type tags, one byte each.
    param_types: Vec<u8>,
    wants_param_types: bool,
    /// Per-chunk metadata records, emitted at finish.
    chunks: Vec<ChunkRecord>,
    wants_chunks: bool,
    /// Concatenated strippable debug metadata.
    debug_pool: Vec<u8>,
    wants_debug: bool,
    /// The shape table, shared by signatures and native return shapes.
    ///
    /// Shared for the same reason the name interner is: `SHAPES` is a single
    /// region and the container rejects a duplicate kind, so two contributors
    /// each declaring their own would collide. That collision was live for one
    /// increment because the only test exercised natives without signatures.
    shapes: Shapes,
    wants_shapes: bool,
}

/// A contributor's slice of the shared constant table: `(first, count)`.
///
/// Roots of every pool occupy the table's prefix in the order they were added,
/// so a pool's constants are a contiguous run indexable as `first + i`.
pub type ConstRange = (u32, u32);

impl SchemaBuilder {
    /// An empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one constant pool and returns its range in the shared table.
    ///
    /// # Why pools share one table
    ///
    /// A module has one constant pool **per chunk**, and `private_init` in the
    /// data layout is a further forest of constant trees. Giving each its own
    /// table would mean a region kind per contributor, which does not scale to a
    /// per-chunk count, and would store a value used by two chunks twice.
    ///
    /// So flattening is **deferred to [`Self::finish`]**: every pool's roots are
    /// concatenated first, then flattened once. Roots occupy the table's prefix
    /// in the order they were added and children are numbered after all of them,
    /// which keeps the forward-ordering invariant intact — a child still lands
    /// strictly after its parent — while letting each contributor address its own
    /// run as `first + i`.
    pub fn add_constant_pool(&mut self, roots: &[ConstValue]) -> ConstRange {
        let first = self.const_roots.len() as u32;
        self.const_roots.extend_from_slice(roots);
        self.wants_constants = true;
        (first, roots.len() as u32)
    }

    /// Adds a single constant pool, discarding its range.
    ///
    /// Convenience for the common single-pool case and for tests.
    ///
    /// # Errors
    ///
    /// Never fails; the `Result` is kept for symmetry with the other `add_*`
    /// methods and so a future capacity check is not a breaking change.
    pub fn add_constants(&mut self, roots: &[ConstValue]) -> Result<(), WireError> {
        self.add_constant_pool(roots);
        Ok(())
    }

    /// Adds the shape and signature tables.
    ///
    /// # Errors
    ///
    /// Propagates a [`WireError`] from the container builder.
    pub fn add_signatures(&mut self, sigs: &[ChunkSignature]) -> Result<(), WireError> {
        let mut records = Vec::with_capacity(sigs.len());

        for sig in sigs {
            // Parameters unshared, so the run stays contiguous.
            let params_first = self.shapes.recs.len() as u32;
            for p in &sig.params {
                self.shapes.append(p);
            }
            // Singles may share. Safe to intern after the run: shapes reference
            // nothing, so unlike the constant table there is no ordering rule.
            let ret = self.shapes.intern(&sig.ret);
            let resume = self.shapes.intern(&sig.resume);

            records.push(SignatureRecord {
                params_first,
                params_count: sig.params.len() as u32,
                ret,
                resume,
            });
        }
        self.wants_shapes = true;

        let sig_region = self.b.region(kind::SIGNATURES, 0)?;
        for r in &records {
            self.b.push_record(sig_region, r);
        }
        Ok(())
    }

    /// Adds the struct-template table.
    ///
    /// Field names are interned **fresh** so each template's run stays
    /// contiguous for `field_names_first + i`, the same reason struct constants
    /// do it. Type names are shared.
    ///
    /// # Errors
    ///
    /// Propagates a [`WireError`] from the container builder.
    pub fn add_struct_templates(&mut self, templates: &[StructTemplate]) -> Result<(), WireError> {
        self.add_struct_template_pool(templates);
        Ok(())
    }

    /// Adds one chunk's struct templates and returns its range in the shared table.
    ///
    /// Templates are declared **per chunk**, so like constants the table serves
    /// many contributors. Emission is deferred to [`Self::finish`] so every
    /// contributor's templates concatenate into one table and each gets a
    /// contiguous run.
    pub fn add_struct_template_pool(&mut self, templates: &[StructTemplate]) -> ConstRange {
        let first = self.template_pool.len() as u32;
        self.template_pool.extend_from_slice(templates);
        self.wants_templates = true;
        (first, templates.len() as u32)
    }

    /// Adds one chunk's parameter type tags, returning their `(offset, count)`
    /// range in the parameter-type byte pool.
    pub fn add_param_types(&mut self, types: &[TypeTag]) -> ConstRange {
        let first = self.param_types.len() as u32;
        for t in types {
            self.param_types.push(type_tag_byte(*t));
        }
        self.wants_param_types = true;
        (first, types.len() as u32)
    }

    /// Adds the enum-layout table and its variant table.
    ///
    /// Variant names are interned **fresh** so a layout's variants form a
    /// contiguous run; the variant records carry the discriminants, which a bare
    /// name run could not.
    ///
    /// # Errors
    ///
    /// Propagates a [`WireError`] from the container builder.
    pub fn add_enum_layouts(&mut self, layouts: &[EnumLayout]) -> Result<(), WireError> {
        let mut variants = Vec::new();
        let mut records = Vec::with_capacity(layouts.len());

        for l in layouts {
            let type_name = self.names.intern(&l.type_name);
            let variants_first = variants.len() as u32;
            for v in &l.variants {
                let name = self.names.intern_fresh(&v.name);
                variants.push(EnumVariantRecord {
                    name,
                    reserved: 0,
                    disc: v.disc,
                });
            }
            records.push(EnumLayoutRecord {
                type_name,
                variants_first,
                variants_count: l.variants.len() as u32,
                min_payload: l.min_payload,
            });
        }

        let vregion = self.b.region(kind::ENUM_VARIANTS, 0)?;
        let lregion = self.b.region(kind::ENUM_LAYOUTS, 0)?;
        for r in &variants {
            self.b.push_record(vregion, r);
        }
        for r in &records {
            self.b.push_record(lregion, r);
        }
        Ok(())
    }

    /// Emits the artifact, appending the shared string pool and name table.
    ///
    /// # Errors
    ///
    /// Propagates a [`WireError`] from the container builder.
    pub fn finish(mut self) -> Result<Vec<u8>, WireError> {
        // Constants are flattened here, once, over every contributor's roots
        // together. Doing it per contributor would give each its own numbering
        // and there is only one table.
        if self.wants_constants {
            let t = flatten(&self.const_roots, &mut self.names);
            let consts = self.b.region(kind::CONSTS, 0)?;
            let saux = self.b.region(kind::STRUCT_AUX, 0)?;
            let eaux = self.b.region(kind::ENUM_AUX, 0)?;
            for r in &t.consts {
                self.b.push_record(consts, r);
            }
            for r in &t.struct_aux {
                self.b.push_record(saux, r);
            }
            for r in &t.enum_aux {
                self.b.push_record(eaux, r);
            }
        }

        // Templates intern names, so they must be emitted before the name table
        // is written out. Each contributor's field-name run stays contiguous
        // because a template's names are interned consecutively.
        if self.wants_templates {
            let mut records = Vec::with_capacity(self.template_pool.len());
            for t in &self.template_pool {
                let type_name = self.names.intern(&t.type_name);
                let field_names_first = self.names.refs.len() as u32;
                for f in &t.field_names {
                    self.names.intern_fresh(f);
                }
                records.push(StructTemplateRecord {
                    type_name,
                    field_names_first,
                    field_count: t.field_names.len() as u32,
                    reserved: 0,
                });
            }
            let region = self.b.region(kind::STRUCT_TEMPLATES, 0)?;
            for r in &records {
                self.b.push_record(region, r);
            }
        }

        if self.wants_param_types {
            let region = self.b.region(kind::PARAM_TYPES, 0)?;
            let bytes = core::mem::take(&mut self.param_types);
            self.b.push(region, &bytes);
        }

        if self.wants_shapes {
            let region = self.b.region(kind::SHAPES, 0)?;
            let recs = core::mem::take(&mut self.shapes.recs);
            for r in &recs {
                self.b.push_record(region, r);
            }
        }

        if self.wants_chunks {
            let region = self.b.region(kind::CHUNKS, 0)?;
            let recs = core::mem::take(&mut self.chunks);
            for r in &recs {
                self.b.push_record(region, r);
            }
        }

        if self.wants_debug {
            let region = self.b.region(kind::DEBUG_POOL, 0)?;
            let bytes = core::mem::take(&mut self.debug_pool);
            self.b.push(region, &bytes);
        }

        let pool = self.b.region(kind::STRING_POOL, 0)?;
        let names = self.b.region(kind::NAMES, 0)?;
        self.b.push(pool, &self.names.pool);
        for r in &self.names.refs {
            self.b.push_record(names, r);
        }
        self.b.finish()
    }
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
    decode_constant_pool(bytes, (0, count as u32))
}

/// Decodes one contributor's pool, given the range [`SchemaBuilder::add_constant_pool`]
/// returned.
///
/// # Errors
///
/// [`SchemaError`] for any malformed artifact, or if the range lies outside the
/// table.
pub fn decode_constant_pool(
    bytes: &[u8],
    range: ConstRange,
) -> Result<Vec<ConstValue>, SchemaError> {
    Ok(decode_constant_pools(bytes, &[range])?
        .pop()
        .unwrap_or_default())
}

/// Decodes several pools in **one pass** over the constant table.
///
/// # Why this exists
///
/// Decoding pools one at a time re-parses and re-materialises the whole table
/// per call, which is quadratic in the number of contributors. A module has one
/// pool per chunk, so a real module with a few hundred chunks made
/// [`decode_aux_body`] pathologically slow — the corpus test found it at a
/// scale hand-built cases never reach.
///
/// The ranges are root indices and are disjoint, so one sweep serves all of them.
///
/// # Errors
///
/// [`SchemaError`] for any malformed artifact, or if a range lies outside the
/// table.
pub fn decode_constant_pools(
    bytes: &[u8],
    ranges: &[ConstRange],
) -> Result<Vec<Vec<ConstValue>>, SchemaError> {
    // One parse-and-validate path, shared with the borrowed accessor. Keeping a
    // second copy here would let the owned and borrowed readers drift apart, and
    // a drift in the ordering check is exactly the silent-wrong-answer class this
    // format is shaped to avoid.
    let t = ConstTable::parse(bytes)?;

    for r in ranges {
        let end = (r.0 as usize)
            .checked_add(r.1 as usize)
            .ok_or(SchemaError::BadIndex)?;
        if end > t.len() {
            return Err(SchemaError::BadIndex);
        }
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

    let mut out = Vec::with_capacity(ranges.len());
    for r in ranges {
        let first = r.0 as usize;
        let end = first + r.1 as usize;
        let mut pool = Vec::with_capacity(r.1 as usize);
        for slot in built.iter_mut().take(end).skip(first) {
            pool.push(slot.take().ok_or(SchemaError::BadIndex)?);
        }
        out.push(pool);
    }
    Ok(out)
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
    let mut b = SchemaBuilder::new();
    b.add_signatures(sigs)?;
    b.finish()
}

/// A **borrowed, allocation-free** view over an encoded signature table.
#[derive(Debug, Clone, Copy)]
pub struct SignatureTable<'a> {
    shapes: RecordTable<'a>,
    /// Absent when the module declares no per-chunk signatures.
    ///
    /// The shape table is shared with native return shapes, so an artifact may
    /// carry shapes with no signatures at all. Demanding the signature region
    /// would make those shapes unreadable — and "no signatures" has one reading,
    /// so absent is simply empty here.
    sigs: Option<RecordTable<'a>>,
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
            sigs: match view.find_region(kind::SIGNATURES) {
                Some(r) => Some(view.typed_records::<SignatureRecord>(&r)?),
                None => None,
            },
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
        self.sigs.map(|t| t.len()).unwrap_or(0)
    }

    /// True when there are no signatures.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The raw signature record for chunk `index`.
    #[inline]
    pub fn record(&self, index: usize) -> Option<SignatureRecord> {
        self.sigs?.get_as::<SignatureRecord>(index)
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

/// A **borrowed, allocation-free** view over the struct-template and enum-layout
/// tables.
///
/// Shares the string pool and name table with the constant table, so a type name
/// mentioned by both is stored once.
#[derive(Debug, Clone, Copy)]
pub struct LayoutTable<'a> {
    pool: Pool<'a>,
    names: RecordTable<'a>,
    /// Absent when the module declares no struct templates.
    ///
    /// Unlike [`DataLayoutTable`], absent and empty mean the **same** thing
    /// here: a module with no templates. `Option<DataLayout>` is semantically
    /// meaningful — a module with no `data` block differs from one whose block
    /// is empty — but "no struct templates" has only one reading, so an absent
    /// region is simply an empty table rather than a distinct state.
    templates: Option<RecordTable<'a>>,
    variants: Option<RecordTable<'a>>,
    layouts: Option<RecordTable<'a>>,
}

impl<'a> LayoutTable<'a> {
    /// Parses and validates the template and layout tables.
    ///
    /// Every name reference and variant range is bounds-checked here, once, so
    /// later accessors are total.
    ///
    /// # Errors
    ///
    /// [`SchemaError`] for any malformed artifact. Never panics.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, SchemaError> {
        let view = WireView::parse(bytes)?;
        let region = |k: u16| view.find_region(k).ok_or(SchemaError::MissingRegion(k));

        let opt = |k: u16| view.find_region(k);
        let t = Self {
            pool: view.pool(&region(kind::STRING_POOL)?)?,
            names: view.typed_records::<NameRef>(&region(kind::NAMES)?)?,
            templates: match opt(kind::STRUCT_TEMPLATES) {
                Some(r) => Some(view.typed_records::<StructTemplateRecord>(&r)?),
                None => None,
            },
            variants: match opt(kind::ENUM_VARIANTS) {
                Some(r) => Some(view.typed_records::<EnumVariantRecord>(&r)?),
                None => None,
            },
            layouts: match opt(kind::ENUM_LAYOUTS) {
                Some(r) => Some(view.typed_records::<EnumLayoutRecord>(&r)?),
                None => None,
            },
        };

        for i in 0..t.template_count() {
            let r = t.template(i).ok_or(SchemaError::BadIndex)?;
            let end = (r.field_names_first as usize)
                .checked_add(r.field_count as usize)
                .ok_or(SchemaError::BadIndex)?;
            if end > t.names.len() || r.type_name as usize >= t.names.len() {
                return Err(SchemaError::BadIndex);
            }
        }
        let variant_len = t.variants.map(|v| v.len()).unwrap_or(0);
        for i in 0..t.layout_count() {
            let r = t.layout(i).ok_or(SchemaError::BadIndex)?;
            let end = (r.variants_first as usize)
                .checked_add(r.variants_count as usize)
                .ok_or(SchemaError::BadIndex)?;
            if end > variant_len || r.type_name as usize >= t.names.len() {
                return Err(SchemaError::BadIndex);
            }
        }
        Ok(t)
    }

    /// Number of struct templates.
    #[inline]
    pub fn template_count(&self) -> usize {
        self.templates.map(|t| t.len()).unwrap_or(0)
    }

    /// Number of enum layouts.
    #[inline]
    pub fn layout_count(&self) -> usize {
        self.layouts.map(|t| t.len()).unwrap_or(0)
    }

    /// Bytes of name `index`, as a slice into the artifact.
    #[inline]
    pub fn name_bytes(&self, index: u32) -> Option<&'a [u8]> {
        let r = self.names.get_as::<NameRef>(index as usize)?;
        self.pool.slice(r.offset, r.length)
    }

    /// The raw template record at `index`.
    #[inline]
    pub fn template(&self, index: usize) -> Option<StructTemplateRecord> {
        self.templates?.get_as::<StructTemplateRecord>(index)
    }

    /// Field `field` of template `index`, as bytes aliasing the artifact.
    #[inline]
    pub fn template_field_name(&self, index: usize, field: usize) -> Option<&'a [u8]> {
        let r = self.template(index)?;
        if field >= r.field_count as usize {
            return None;
        }
        self.name_bytes(r.field_names_first + field as u32)
    }

    /// The raw layout record at `index`.
    #[inline]
    pub fn layout(&self, index: usize) -> Option<EnumLayoutRecord> {
        self.layouts?.get_as::<EnumLayoutRecord>(index)
    }

    /// Variant `variant` of layout `index`: its name bytes and discriminant.
    #[inline]
    pub fn layout_variant(&self, index: usize, variant: usize) -> Option<(&'a [u8], i64)> {
        let r = self.layout(index)?;
        if variant >= r.variants_count as usize {
            return None;
        }
        let v = self
            .variants?
            .get_as::<EnumVariantRecord>(r.variants_first as usize + variant)?;
        Some((self.name_bytes(v.name)?, v.disc))
    }
}

/// Decodes struct templates into owned values.
///
/// # Errors
///
/// [`SchemaError`] for any malformed artifact.
pub fn decode_struct_templates(bytes: &[u8]) -> Result<Vec<StructTemplate>, SchemaError> {
    let t = LayoutTable::parse(bytes)?;
    let name = |b: Option<&[u8]>| -> Result<String, SchemaError> {
        core::str::from_utf8(b.ok_or(SchemaError::BadName)?)
            .map(String::from)
            .map_err(|_| SchemaError::BadName)
    };

    let mut out = Vec::with_capacity(t.template_count());
    for i in 0..t.template_count() {
        let r = t.template(i).ok_or(SchemaError::BadIndex)?;
        let mut field_names = Vec::with_capacity(r.field_count as usize);
        for f in 0..r.field_count as usize {
            field_names.push(name(t.template_field_name(i, f))?);
        }
        out.push(StructTemplate {
            type_name: name(t.name_bytes(r.type_name))?,
            field_names,
        });
    }
    Ok(out)
}

/// Decodes enum layouts into owned values.
///
/// # Errors
///
/// [`SchemaError`] for any malformed artifact.
pub fn decode_enum_layouts(bytes: &[u8]) -> Result<Vec<EnumLayout>, SchemaError> {
    use crate::bytecode::EnumVariantDisc;

    let t = LayoutTable::parse(bytes)?;
    let name = |b: Option<&[u8]>| -> Result<String, SchemaError> {
        core::str::from_utf8(b.ok_or(SchemaError::BadName)?)
            .map(String::from)
            .map_err(|_| SchemaError::BadName)
    };

    let mut out = Vec::with_capacity(t.layout_count());
    for i in 0..t.layout_count() {
        let r = t.layout(i).ok_or(SchemaError::BadIndex)?;
        let mut variants = Vec::with_capacity(r.variants_count as usize);
        for v in 0..r.variants_count as usize {
            let (nb, disc) = t.layout_variant(i, v).ok_or(SchemaError::BadIndex)?;
            variants.push(EnumVariantDisc {
                name: name(Some(nb))?,
                disc,
            });
        }
        out.push(EnumLayout {
            type_name: name(t.name_bytes(r.type_name))?,
            variants,
            min_payload: r.min_payload,
        });
    }
    Ok(out)
}

/// Encodes struct templates and enum layouts into one artifact.
///
/// # Errors
///
/// Propagates a [`WireError`] from the container builder.
pub fn encode_layouts(
    templates: &[StructTemplate],
    layouts: &[EnumLayout],
) -> Result<Vec<u8>, WireError> {
    let mut b = SchemaBuilder::new();
    b.add_struct_templates(templates)?;
    b.add_enum_layouts(layouts)?;
    b.finish()
}

// ---------------------------------------------------------------------------
// Data layout (stage 2b, increment 4)
// ---------------------------------------------------------------------------

/// One named data slot. A single word.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataSlotRecord {
    /// Name index of the slot name.
    pub name: u32,
    /// See [`visibility_tag`].
    pub visibility: u8,
    /// Reserved.
    pub reserved: u8,
    /// Reserved; keeps the record one whole word.
    pub reserved2: u16,
}

/// One shared slot's byte layout in the host buffer. A single word.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedSlotRecord {
    /// Byte offset within the host buffer.
    pub offset: u32,
    /// Scalar or composite kind tag; the high bit marks a composite.
    pub kind: u8,
    /// Reserved; keeps `len` at a fixed offset.
    pub reserved: u8,
    /// Flat composite body length; zero for a scalar slot.
    pub len: u16,
}

/// One private composite slot's placement in the persistent pool. A single word.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivateCompositeRecord {
    /// Unified data-slot index.
    pub slot: u16,
    /// Reserved; keeps `offset` at a fixed offset.
    pub reserved: u16,
    /// Byte offset within the persistent composite pool.
    pub offset: u32,
}

/// The private-slot initialiser range into the shared constant table.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataInitRecord {
    /// First constant index.
    pub first: u32,
    /// Number of initialisers.
    pub count: u32,
}

impl SchemaBuilder {
    /// Adds the data-segment layout.
    ///
    /// `private_init` is a forest of constant trees, so it goes through
    /// [`Self::add_constant_pool`] and is referenced by range — the same shared
    /// table a chunk's constants use, rather than a parallel copy of the
    /// flattening machinery.
    ///
    /// # Errors
    ///
    /// Propagates a [`WireError`] from the container builder.
    pub fn add_data_layout(&mut self, layout: &DataLayout) -> Result<(), WireError> {
        use crate::bytecode::SlotVisibility;

        let (first, count) = self.add_constant_pool(&layout.private_init);

        let slots = self.b.region(kind::DATA_SLOTS, 0)?;
        let shared = self.b.region(kind::SHARED_LAYOUT, 0)?;
        let privcomp = self.b.region(kind::PRIVATE_COMPOSITE, 0)?;
        let init = self.b.region(kind::DATA_INIT, 0)?;

        for s in &layout.slots {
            let name = self.names.intern(&s.name);
            self.b.push_record(
                slots,
                &DataSlotRecord {
                    name,
                    visibility: match s.visibility {
                        SlotVisibility::Shared => visibility_tag::SHARED,
                        SlotVisibility::Private => visibility_tag::PRIVATE,
                    },
                    reserved: 0,
                    reserved2: 0,
                },
            );
        }
        for l in &layout.shared_layout {
            self.b.push_record(
                shared,
                &SharedSlotRecord {
                    offset: l.offset,
                    kind: l.kind,
                    reserved: 0,
                    len: l.len,
                },
            );
        }
        for p in &layout.private_composite_layout {
            self.b.push_record(
                privcomp,
                &PrivateCompositeRecord {
                    slot: p.slot,
                    reserved: 0,
                    offset: p.offset,
                },
            );
        }
        self.b.push_record(init, &DataInitRecord { first, count });
        Ok(())
    }
}

/// A **borrowed, allocation-free** view over the data-segment layout.
#[derive(Debug, Clone, Copy)]
pub struct DataLayoutTable<'a> {
    pool: Pool<'a>,
    names: RecordTable<'a>,
    slots: RecordTable<'a>,
    shared: RecordTable<'a>,
    private_composite: RecordTable<'a>,
    init: DataInitRecord,
}

impl<'a> DataLayoutTable<'a> {
    /// Parses the data layout, or reports it absent.
    ///
    /// Returns `Ok(None)` when the artifact carries no data layout, which is a
    /// module with no `data` block — distinct from a layout with zero slots.
    ///
    /// # Errors
    ///
    /// [`SchemaError`] for a malformed artifact. Never panics.
    pub fn parse(bytes: &'a [u8]) -> Result<Option<Self>, SchemaError> {
        let view = WireView::parse(bytes)?;
        let Some(slots_region) = view.find_region(kind::DATA_SLOTS) else {
            return Ok(None);
        };
        let region = |k: u16| view.find_region(k).ok_or(SchemaError::MissingRegion(k));

        let init_table = view.typed_records::<DataInitRecord>(&region(kind::DATA_INIT)?)?;
        let init = init_table
            .get_as::<DataInitRecord>(0)
            .ok_or(SchemaError::BadIndex)?;

        let t = Self {
            pool: view.pool(&region(kind::STRING_POOL)?)?,
            names: view.typed_records::<NameRef>(&region(kind::NAMES)?)?,
            slots: view.typed_records::<DataSlotRecord>(&slots_region)?,
            shared: view.typed_records::<SharedSlotRecord>(&region(kind::SHARED_LAYOUT)?)?,
            private_composite: view
                .typed_records::<PrivateCompositeRecord>(&region(kind::PRIVATE_COMPOSITE)?)?,
            init,
        };

        for i in 0..t.slot_count() {
            let r = t.slot(i).ok_or(SchemaError::BadIndex)?;
            if r.name as usize >= t.names.len() {
                return Err(SchemaError::BadIndex);
            }
            if r.visibility != visibility_tag::SHARED && r.visibility != visibility_tag::PRIVATE {
                return Err(SchemaError::UnknownTag(r.visibility as u16));
            }
        }
        Ok(Some(t))
    }

    /// Number of declared slots.
    #[inline]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// The slot record at `index`.
    #[inline]
    pub fn slot(&self, index: usize) -> Option<DataSlotRecord> {
        self.slots.get_as::<DataSlotRecord>(index)
    }

    /// Slot `index`'s name, as bytes aliasing the artifact.
    #[inline]
    pub fn slot_name(&self, index: usize) -> Option<&'a [u8]> {
        let r = self.slot(index)?;
        let n = self.names.get_as::<NameRef>(r.name as usize)?;
        self.pool.slice(n.offset, n.length)
    }

    /// The shared-slot layout at `index`.
    #[inline]
    pub fn shared_slot(&self, index: usize) -> Option<SharedSlotRecord> {
        self.shared.get_as::<SharedSlotRecord>(index)
    }

    /// Number of shared-slot layout entries.
    #[inline]
    pub fn shared_count(&self) -> usize {
        self.shared.len()
    }

    /// The private-composite placement at `index`.
    #[inline]
    pub fn private_composite(&self, index: usize) -> Option<PrivateCompositeRecord> {
        self.private_composite
            .get_as::<PrivateCompositeRecord>(index)
    }

    /// Number of private-composite placements.
    #[inline]
    pub fn private_composite_count(&self) -> usize {
        self.private_composite.len()
    }

    /// The constant range holding the private-slot initialisers.
    #[inline]
    pub fn private_init_range(&self) -> ConstRange {
        (self.init.first, self.init.count)
    }
}

/// Decodes the data layout into an owned value, or `None` if absent.
///
/// # Errors
///
/// [`SchemaError`] for any malformed artifact.
pub fn decode_data_layout(bytes: &[u8]) -> Result<Option<DataLayout>, SchemaError> {
    use crate::bytecode::{DataSlot, PrivateCompositeSlot, SharedSlotLayout, SlotVisibility};

    let Some(t) = DataLayoutTable::parse(bytes)? else {
        return Ok(None);
    };

    let mut slots = Vec::with_capacity(t.slot_count());
    for i in 0..t.slot_count() {
        let r = t.slot(i).ok_or(SchemaError::BadIndex)?;
        let name = core::str::from_utf8(t.slot_name(i).ok_or(SchemaError::BadName)?)
            .map(String::from)
            .map_err(|_| SchemaError::BadName)?;
        slots.push(DataSlot {
            name,
            visibility: if r.visibility == visibility_tag::SHARED {
                SlotVisibility::Shared
            } else {
                SlotVisibility::Private
            },
        });
    }

    let mut shared_layout = Vec::with_capacity(t.shared_count());
    for i in 0..t.shared_count() {
        let r = t.shared_slot(i).ok_or(SchemaError::BadIndex)?;
        shared_layout.push(SharedSlotLayout {
            offset: r.offset,
            kind: r.kind,
            len: r.len,
        });
    }

    let mut private_composite_layout = Vec::with_capacity(t.private_composite_count());
    for i in 0..t.private_composite_count() {
        let r = t.private_composite(i).ok_or(SchemaError::BadIndex)?;
        private_composite_layout.push(PrivateCompositeSlot {
            slot: r.slot,
            offset: r.offset,
        });
    }

    let private_init = decode_constant_pool(bytes, t.private_init_range())?;

    Ok(Some(DataLayout {
        slots,
        shared_layout,
        private_composite_layout,
        private_init,
    }))
}

/// A **borrowed** view over the parameter-type byte pool.
///
/// Tags are one byte each, so this is a flat pool rather than a record table —
/// a whole-word record per tag would waste seven eighths of the region.
#[derive(Debug, Clone, Copy)]
pub struct ParamTypeTable<'a> {
    bytes: &'a [u8],
}

impl<'a> ParamTypeTable<'a> {
    /// Wraps an already-resolved parameter-type region.
    #[inline]
    pub fn from_bytes(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// Parses the parameter-type pool, or reports it absent.
    ///
    /// # Errors
    ///
    /// [`SchemaError`] for a malformed container. Never panics.
    pub fn parse(bytes: &'a [u8]) -> Result<Option<Self>, SchemaError> {
        let view = WireView::parse(bytes)?;
        let Some(region) = view.find_region(kind::PARAM_TYPES) else {
            return Ok(None);
        };
        Ok(Some(Self {
            bytes: view.pool(&region)?.bytes(),
        }))
    }

    /// The raw tag bytes for a chunk's range, aliasing the artifact.
    #[inline]
    pub fn tag_bytes(&self, range: ConstRange) -> Option<&'a [u8]> {
        let start = range.0 as usize;
        let end = start.checked_add(range.1 as usize)?;
        self.bytes.get(start..end)
    }

    /// Decodes a chunk's parameter type tags.
    ///
    /// # Errors
    ///
    /// [`SchemaError::BadIndex`] if the range is out of bounds, or
    /// [`SchemaError::UnknownTag`] for an unrecognised tag byte.
    pub fn tags(&self, range: ConstRange) -> Result<Vec<TypeTag>, SchemaError> {
        let raw = self.tag_bytes(range).ok_or(SchemaError::BadIndex)?;
        let mut out = Vec::with_capacity(raw.len());
        for b in raw {
            out.push(type_tag_from_byte(*b).ok_or(SchemaError::UnknownTag(*b as u16))?);
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// The chunk table, natives, header, and debug pool (stage 2b, increment 6)
// ---------------------------------------------------------------------------

/// Per-chunk metadata. Six words.
///
/// Every variable-length part of a chunk lives in a shared table and is
/// referenced here by range, so the record itself is fixed-size.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkRecord {
    /// Name index of the chunk name.
    pub name: u32,
    /// First constant, in the shared constant table.
    pub consts_first: u32,
    /// Number of constants.
    pub consts_count: u32,
    /// First struct template, in the shared template table.
    pub templates_first: u32,
    /// Number of struct templates.
    pub templates_count: u32,
    /// Byte offset of the parameter type tags in the parameter-type pool.
    pub param_types_first: u32,
    /// Number of parameter type tags.
    pub param_types_count: u32,
    /// Byte offset of this chunk's debug metadata, or [`ABSENT`] when the chunk
    /// carries none. `ABSENT` distinguishes `None` from `Some(empty)`.
    pub debug_first: u32,
    /// Byte length of the debug metadata; zero when absent or empty.
    pub debug_len: u32,
    /// Byte offset into the opcode stream where this chunk's records start.
    pub op_byte_offset: u32,
    /// Number of opcode records in the chunk body.
    pub op_record_count: u32,
    /// Total local variable slots.
    pub local_count: u16,
    /// Number of parameters.
    pub param_count: u8,
    /// See [`block_tag`].
    pub block_type: u8,
}

/// One native function's name.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeRecord {
    /// Name index of the native's name.
    pub name: u32,
    /// Reserved; keeps the record one whole word.
    pub reserved: u32,
}

/// One native return shape, as an index into the shape table.
///
/// A separate table from [`NativeRecord`] because the two vectors may legally
/// differ in length; see [`kind::NATIVE_RETURNS`].
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeReturnRecord {
    /// Shape index of the return value.
    pub shape: u32,
    /// Reserved; keeps the record one whole word.
    pub reserved: u32,
}

/// The module's scalar header fields. Four words.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderRecord {
    /// Entry-point chunk index, or [`ABSENT`].
    pub entry_point: u32,
    /// Runtime word width, as log2 of the bit width.
    pub word_bits_log2: u8,
    /// Runtime address width, log2 form.
    pub addr_bits_log2: u8,
    /// Runtime float width, log2 form.
    pub float_bits_log2: u8,
    /// Header flag byte.
    pub flags: u8,
    /// Declared WCET in cycles; zero means auto.
    pub wcet_cycles: u32,
    /// Declared WCMU in bytes; zero means auto.
    pub wcmu_bytes: u32,
    /// Shared-partition byte count.
    pub shared_data_bytes: u32,
    /// Private-partition byte count.
    pub private_data_bytes: u32,
    /// Schema hash for hot-swap compatibility.
    pub schema_hash: u32,
    /// Reserved; keeps the record four whole words.
    pub reserved: u32,
}

/// Everything needed to emit one chunk's metadata record.
///
/// The ranges come from the `add_*` calls that placed the chunk's data, so a
/// caller cannot accidentally describe a range it never wrote.
#[derive(Debug, Clone, Copy)]
pub struct ChunkMeta {
    /// Range returned by [`SchemaBuilder::add_constant_pool`].
    pub constants: ConstRange,
    /// Range returned by [`SchemaBuilder::add_struct_template_pool`].
    pub templates: ConstRange,
    /// Range returned by [`SchemaBuilder::add_param_types`].
    pub param_types: ConstRange,
    /// Total local variable slots.
    pub local_count: u16,
    /// Number of parameters.
    pub param_count: u8,
    /// Block type.
    pub block_type: BlockType,
    /// Byte offset into the opcode stream.
    pub op_byte_offset: u32,
    /// Number of opcode records.
    pub op_record_count: u32,
}

impl SchemaBuilder {
    /// Adds one chunk's metadata record.
    ///
    /// `debug` is the chunk's strippable metadata: `None` for a release build,
    /// which is stored distinctly from `Some(empty)`.
    ///
    /// # Errors
    ///
    /// Propagates a [`WireError`] from the container builder.
    pub fn add_chunk(
        &mut self,
        name: &str,
        meta: &ChunkMeta,
        debug: Option<&[u8]>,
    ) -> Result<(), WireError> {
        let name_idx = self.names.intern(name);
        let (debug_first, debug_len) = match debug {
            Some(bytes) => {
                let at = self.debug_pool.len() as u32;
                self.debug_pool.extend_from_slice(bytes);
                self.wants_debug = true;
                (at, bytes.len() as u32)
            }
            None => (ABSENT, 0),
        };

        self.chunks.push(ChunkRecord {
            name: name_idx,
            consts_first: meta.constants.0,
            consts_count: meta.constants.1,
            templates_first: meta.templates.0,
            templates_count: meta.templates.1,
            param_types_first: meta.param_types.0,
            param_types_count: meta.param_types.1,
            debug_first,
            debug_len,
            op_byte_offset: meta.op_byte_offset,
            op_record_count: meta.op_record_count,
            local_count: meta.local_count,
            param_count: meta.param_count,
            block_type: match meta.block_type {
                BlockType::Func => block_tag::FUNC,
                BlockType::Reentrant => block_tag::REENTRANT,
                BlockType::Stream => block_tag::STREAM,
            },
        });
        self.wants_chunks = true;
        Ok(())
    }

    /// Adds the native-function table.
    ///
    /// `return_shapes` may be shorter than `names`, or empty, which is the
    /// additive case: a native without a described return shape records
    /// [`ABSENT`].
    ///
    /// # Errors
    ///
    /// Propagates a [`WireError`] from the container builder.
    pub fn add_natives(
        &mut self,
        names: &[String],
        return_shapes: &[WireShape],
    ) -> Result<(), WireError> {
        let mut records = Vec::with_capacity(names.len());
        for n in names {
            let name = self.names.intern(n);
            records.push(NativeRecord { name, reserved: 0 });
        }
        // Both vectors are emitted at their own length. They are documented as
        // parallel but are not required to be, and dropping the surplus would be
        // silent data loss.
        let mut shapes = Vec::with_capacity(return_shapes.len());
        for s in return_shapes {
            self.wants_shapes = true;
            shapes.push(NativeReturnRecord {
                shape: self.shapes.intern(s),
                reserved: 0,
            });
        }

        let region = self.b.region(kind::NATIVES, 0)?;
        for r in &records {
            self.b.push_record(region, r);
        }
        let region = self.b.region(kind::NATIVE_RETURNS, 0)?;
        for r in &shapes {
            self.b.push_record(region, r);
        }
        Ok(())
    }

    /// Adds the module's scalar header.
    ///
    /// # Errors
    ///
    /// Propagates a [`WireError`] from the container builder.
    #[allow(clippy::too_many_arguments)]
    pub fn add_header(&mut self, header: &HeaderRecord) -> Result<(), WireError> {
        let region = self.b.region(kind::HEADER, 0)?;
        self.b.push_record(region, header);
        Ok(())
    }
}

/// A **borrowed, allocation-free** view over the chunk table, natives, and header.
#[derive(Debug, Clone, Copy)]
pub struct ModuleTable<'a> {
    pool: Pool<'a>,
    names: RecordTable<'a>,
    chunks: Option<RecordTable<'a>>,
    natives: Option<RecordTable<'a>>,
    native_returns: Option<RecordTable<'a>>,
    header: Option<HeaderRecord>,
    debug: Option<Pool<'a>>,
}

impl<'a> ModuleTable<'a> {
    /// Parses the module-level tables.
    ///
    /// # Errors
    ///
    /// [`SchemaError`] for any malformed artifact. Never panics.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, SchemaError> {
        let view = WireView::parse(bytes)?;
        let region = |k: u16| view.find_region(k).ok_or(SchemaError::MissingRegion(k));
        let opt = |k: u16| view.find_region(k);

        let header = match opt(kind::HEADER) {
            Some(r) => view
                .typed_records::<HeaderRecord>(&r)?
                .get_as::<HeaderRecord>(0),
            None => None,
        };

        let t = Self {
            pool: view.pool(&region(kind::STRING_POOL)?)?,
            names: view.typed_records::<NameRef>(&region(kind::NAMES)?)?,
            chunks: match opt(kind::CHUNKS) {
                Some(r) => Some(view.typed_records::<ChunkRecord>(&r)?),
                None => None,
            },
            natives: match opt(kind::NATIVES) {
                Some(r) => Some(view.typed_records::<NativeRecord>(&r)?),
                None => None,
            },
            native_returns: match opt(kind::NATIVE_RETURNS) {
                Some(r) => Some(view.typed_records::<NativeReturnRecord>(&r)?),
                None => None,
            },
            header,
            debug: match opt(kind::DEBUG_POOL) {
                Some(r) => Some(view.pool(&r)?),
                None => None,
            },
        };

        for i in 0..t.chunk_count() {
            let c = t.chunk(i).ok_or(SchemaError::BadIndex)?;
            if c.name as usize >= t.names.len() {
                return Err(SchemaError::BadIndex);
            }
            if c.block_type != block_tag::FUNC
                && c.block_type != block_tag::REENTRANT
                && c.block_type != block_tag::STREAM
            {
                return Err(SchemaError::UnknownTag(c.block_type as u16));
            }
        }
        for i in 0..t.native_count() {
            let n = t.native(i).ok_or(SchemaError::BadIndex)?;
            if n.name as usize >= t.names.len() {
                return Err(SchemaError::BadIndex);
            }
        }
        Ok(t)
    }

    /// Number of chunks.
    #[inline]
    pub fn chunk_count(&self) -> usize {
        self.chunks.map(|t| t.len()).unwrap_or(0)
    }

    /// Number of natives.
    #[inline]
    pub fn native_count(&self) -> usize {
        self.natives.map(|t| t.len()).unwrap_or(0)
    }

    /// The chunk record at `index`.
    #[inline]
    pub fn chunk(&self, index: usize) -> Option<ChunkRecord> {
        self.chunks?.get_as::<ChunkRecord>(index)
    }

    /// The native record at `index`.
    #[inline]
    pub fn native(&self, index: usize) -> Option<NativeRecord> {
        self.natives?.get_as::<NativeRecord>(index)
    }

    /// The module header, if present.
    #[inline]
    pub fn header(&self) -> Option<HeaderRecord> {
        self.header
    }

    /// Bytes of name `index`, aliasing the artifact.
    #[inline]
    pub fn name_bytes(&self, index: u32) -> Option<&'a [u8]> {
        let r = self.names.get_as::<NameRef>(index as usize)?;
        self.pool.slice(r.offset, r.length)
    }

    /// Chunk `index`'s name, aliasing the artifact.
    #[inline]
    pub fn chunk_name(&self, index: usize) -> Option<&'a [u8]> {
        self.name_bytes(self.chunk(index)?.name)
    }

    /// Native `index`'s name, aliasing the artifact.
    #[inline]
    pub fn native_name(&self, index: usize) -> Option<&'a [u8]> {
        self.name_bytes(self.native(index)?.name)
    }

    /// Number of declared native return shapes.
    #[inline]
    pub fn native_return_count(&self) -> usize {
        self.native_returns.map(|t| t.len()).unwrap_or(0)
    }

    /// Shape index of native return `index`.
    #[inline]
    pub fn native_return_shape(&self, index: usize) -> Option<u32> {
        Some(
            self.native_returns?
                .get_as::<NativeReturnRecord>(index)?
                .shape,
        )
    }

    /// Chunk `index`'s block type.
    #[inline]
    pub fn chunk_block_type(&self, index: usize) -> Option<BlockType> {
        Some(match self.chunk(index)?.block_type {
            block_tag::FUNC => BlockType::Func,
            block_tag::REENTRANT => BlockType::Reentrant,
            block_tag::STREAM => BlockType::Stream,
            _ => return None,
        })
    }

    /// Chunk `index`'s debug metadata, aliasing the artifact.
    ///
    /// `None` means the chunk carries none, which is distinct from `Some(&[])`.
    #[inline]
    pub fn chunk_debug_bytes(&self, index: usize) -> Option<&'a [u8]> {
        let c = self.chunk(index)?;
        if c.debug_first == ABSENT {
            return None;
        }
        self.debug?.slice(c.debug_first, c.debug_len)
    }

    /// The entry-point chunk index, if the module declares one.
    #[inline]
    pub fn entry_point(&self) -> Option<usize> {
        let h = self.header?;
        if h.entry_point == ABSENT {
            None
        } else {
            Some(h.entry_point as usize)
        }
    }
}

// ---------------------------------------------------------------------------
// The whole auxiliary body (stage 2, first real consumer)
// ---------------------------------------------------------------------------

/// Encodes a complete [`WireAuxBody`](crate::wire_format::WireAuxBody) into one artifact.
///
/// This is the first consumer that drives every `add_*` method together, and so
/// the first thing that exercises the shared-state design end to end rather than
/// one table at a time.
///
/// # Errors
///
/// Propagates a [`WireError`] from the container builder.
pub fn encode_aux_body(aux: &crate::wire_format::WireAuxBody) -> Result<Vec<u8>, WireError> {
    let mut b = SchemaBuilder::new();

    // Per-chunk data first, so each chunk record can carry the ranges the
    // contributions returned. A chunk cannot describe a range it did not write.
    let mut metas = Vec::with_capacity(aux.chunks.len());
    for c in &aux.chunks {
        let constants = b.add_constant_pool(&c.constants);
        let templates = b.add_struct_template_pool(&c.struct_templates);
        let param_types = b.add_param_types(&c.param_types);
        metas.push(ChunkMeta {
            constants,
            templates,
            param_types,
            local_count: c.local_count,
            param_count: c.param_count,
            block_type: c.block_type,
            op_byte_offset: c.op_byte_offset,
            op_record_count: c.op_record_count,
        });
    }
    for (c, m) in aux.chunks.iter().zip(&metas) {
        b.add_chunk(&c.name, m, c.debug_pool_bytes.as_deref())?;
    }

    b.add_signatures(&aux.signatures)?;
    b.add_enum_layouts(&aux.enum_layouts)?;
    b.add_natives(&aux.native_names, &aux.native_return_shapes)?;
    if let Some(dl) = &aux.data_layout {
        b.add_data_layout(dl)?;
    }
    b.add_header(&HeaderRecord {
        entry_point: aux.entry_point.map_or(ABSENT, |e| e as u32),
        word_bits_log2: aux.word_bits_log2,
        addr_bits_log2: aux.addr_bits_log2,
        float_bits_log2: aux.float_bits_log2,
        flags: aux.flags,
        wcet_cycles: aux.wcet_cycles,
        wcmu_bytes: aux.wcmu_bytes,
        shared_data_bytes: aux.shared_data_bytes,
        private_data_bytes: aux.private_data_bytes,
        schema_hash: aux.schema_hash,
        reserved: 0,
    })?;

    b.finish()
}

/// Decodes a complete [`WireAuxBody`](crate::wire_format::WireAuxBody) from an artifact.
///
/// # Errors
///
/// [`SchemaError`] for any malformed artifact. Never panics.
pub fn decode_aux_body(bytes: &[u8]) -> Result<crate::wire_format::WireAuxBody, SchemaError> {
    use crate::wire_format::{WireAuxBody, WireChunk};

    let m = ModuleTable::parse(bytes)?;
    let header = m.header().ok_or(SchemaError::MissingRegion(kind::HEADER))?;
    let params = ParamTypeTable::parse(bytes)?;

    let name_of = |b: Option<&[u8]>| -> Result<String, SchemaError> {
        core::str::from_utf8(b.ok_or(SchemaError::BadName)?)
            .map(String::from)
            .map_err(|_| SchemaError::BadName)
    };

    let templates = decode_struct_templates(bytes).unwrap_or_default();

    // One sweep for every chunk's pool. Decoding them individually re-walks the
    // whole table per chunk, which is quadratic in chunk count.
    let mut ranges = Vec::with_capacity(m.chunk_count());
    for i in 0..m.chunk_count() {
        let c = m.chunk(i).ok_or(SchemaError::BadIndex)?;
        ranges.push((c.consts_first, c.consts_count));
    }
    let mut pools = if ranges.is_empty() {
        Vec::new()
    } else {
        decode_constant_pools(bytes, &ranges)?
    };

    let mut chunks = Vec::with_capacity(m.chunk_count());
    for (i, pool) in pools.iter_mut().enumerate() {
        let c = m.chunk(i).ok_or(SchemaError::BadIndex)?;
        let constants = core::mem::take(pool);
        let first = c.templates_first as usize;
        let end = first
            .checked_add(c.templates_count as usize)
            .ok_or(SchemaError::BadIndex)?;
        let struct_templates = templates
            .get(first..end)
            .ok_or(SchemaError::BadIndex)?
            .to_vec();
        let param_types = match &params {
            Some(p) => p.tags((c.param_types_first, c.param_types_count))?,
            None => Vec::new(),
        };

        chunks.push(WireChunk {
            name: name_of(m.chunk_name(i))?,
            constants,
            struct_templates,
            local_count: c.local_count,
            param_count: c.param_count,
            block_type: m.chunk_block_type(i).ok_or(SchemaError::BadIndex)?,
            param_types,
            op_byte_offset: c.op_byte_offset,
            op_record_count: c.op_record_count,
            debug_pool_bytes: m.chunk_debug_bytes(i).map(<[u8]>::to_vec),
        });
    }

    let sigs = SignatureTable::parse(bytes)
        .ok()
        .map(|_| decode_signatures(bytes))
        .transpose()?
        .unwrap_or_default();

    let mut native_names = Vec::with_capacity(m.native_count());
    for i in 0..m.native_count() {
        native_names.push(name_of(m.native_name(i))?);
    }
    let shapes = SignatureTable::parse(bytes).ok();
    let mut native_return_shapes = Vec::with_capacity(m.native_return_count());
    for i in 0..m.native_return_count() {
        let idx = m.native_return_shape(i).ok_or(SchemaError::BadIndex)?;
        native_return_shapes.push(
            shapes
                .as_ref()
                .and_then(|t| t.shape(idx))
                .ok_or(SchemaError::BadIndex)?,
        );
    }

    Ok(WireAuxBody {
        chunks,
        native_names,
        entry_point: if header.entry_point == ABSENT {
            None
        } else {
            Some(header.entry_point as usize)
        },
        data_layout: decode_data_layout(bytes)?,
        word_bits_log2: header.word_bits_log2,
        addr_bits_log2: header.addr_bits_log2,
        float_bits_log2: header.float_bits_log2,
        wcet_cycles: header.wcet_cycles,
        wcmu_bytes: header.wcmu_bytes,
        flags: header.flags,
        shared_data_bytes: header.shared_data_bytes,
        private_data_bytes: header.private_data_bytes,
        schema_hash: header.schema_hash,
        enum_layouts: decode_enum_layouts(bytes).unwrap_or_default(),
        signatures: sigs,
        native_return_shapes,
    })
}

// ---------------------------------------------------------------------------
// The runtime's read surface (step 5, increment 1)
// ---------------------------------------------------------------------------

/// A **single-parse, borrowed** view exposing exactly what the runtime reads.
///
/// # Why this exists rather than using the tables directly
///
/// Each individual table (`ConstTable`, `ModuleTable`, …) calls
/// [`WireView::parse`] itself. That is right for tooling, which touches one table
/// once, and wrong for the runtime, which reads constants and templates
/// repeatedly during execution — it would re-walk the directory and re-validate
/// on every access.
///
/// `AuxView` parses once and holds the sub-tables, so a read is an index
/// operation. It also presents indices at the granularity the VM actually uses:
/// **chunk-relative**, not table-global, because a chunk addresses its own
/// constant pool from zero.
///
/// # The property that must not be lost
///
/// [`Self::chunk_const_str_bytes`] returns bytes **aliasing the artifact**, which
/// is what allows the runtime to mint a handle over a string constant rather than
/// copying it. A probe of the live runtime established that this is the one read
/// where aliasing is load-bearing: an empty string is deliberately not aliased,
/// and a composite's string leaves are already copied today.
#[derive(Debug, Clone, Copy)]
pub struct AuxView<'a> {
    module: ModuleTable<'a>,
    consts: Option<ConstTable<'a>>,
    layouts: LayoutTable<'a>,
    data: Option<DataLayoutTable<'a>>,
    param_types: Option<ParamTypeTable<'a>>,
}

impl<'a> AuxView<'a> {
    /// Parses an artifact once, validating every table it exposes.
    ///
    /// # Errors
    ///
    /// [`SchemaError`] for any malformed artifact. Never panics.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, SchemaError> {
        Ok(Self {
            module: ModuleTable::parse(bytes)?,
            // A module with no constants carries no constant regions, which is
            // absence rather than an error.
            consts: match ConstTable::parse(bytes) {
                Ok(t) => Some(t),
                Err(SchemaError::MissingRegion(_)) => None,
                Err(e) => return Err(e),
            },
            layouts: LayoutTable::parse(bytes)?,
            data: DataLayoutTable::parse(bytes)?,
            param_types: ParamTypeTable::parse(bytes)?,
        })
    }

    /// Number of chunks.
    #[inline]
    pub fn chunk_count(&self) -> usize {
        self.module.chunk_count()
    }

    /// Chunk `chunk`'s local-slot count.
    #[inline]
    pub fn local_count(&self, chunk: usize) -> Option<u16> {
        Some(self.module.chunk(chunk)?.local_count)
    }

    /// Number of constants in chunk `chunk`'s pool.
    #[inline]
    pub fn const_count(&self, chunk: usize) -> Option<u32> {
        Some(self.module.chunk(chunk)?.consts_count)
    }

    /// Maps a chunk-relative constant index to its index in the shared table.
    ///
    /// Returns `None` when `index` is past the chunk's pool, so a chunk cannot
    /// read another chunk's constants by overrunning its own range.
    #[inline]
    fn global_const(&self, chunk: usize, index: usize) -> Option<usize> {
        let c = self.module.chunk(chunk)?;
        if index >= c.consts_count as usize {
            return None;
        }
        Some(c.consts_first as usize + index)
    }

    /// The constant record for chunk `chunk`'s constant `index`.
    #[inline]
    pub fn const_record(&self, chunk: usize, index: usize) -> Option<ConstRecord> {
        self.consts?.record(self.global_const(chunk, index)?)
    }

    /// **The image-aliasing accessor.** Bytes of a string constant in chunk
    /// `chunk`'s pool, as a slice into the artifact.
    ///
    /// `None` when the index is out of range or the constant is not a string.
    #[inline]
    pub fn chunk_const_str_bytes(&self, chunk: usize, index: usize) -> Option<&'a [u8]> {
        self.consts?.str_bytes(self.global_const(chunk, index)?)
    }

    /// Number of struct templates in chunk `chunk`.
    #[inline]
    pub fn template_count(&self, chunk: usize) -> Option<u32> {
        Some(self.module.chunk(chunk)?.templates_count)
    }

    /// Chunk `chunk`'s struct template `index`: its type name, aliasing the
    /// artifact.
    #[inline]
    pub fn template_type_name(&self, chunk: usize, index: usize) -> Option<&'a [u8]> {
        let c = self.module.chunk(chunk)?;
        if index >= c.templates_count as usize {
            return None;
        }
        let t = self.layouts.template(c.templates_first as usize + index)?;
        self.layouts.name_bytes(t.type_name)
    }

    /// Field `field` of chunk `chunk`'s struct template `index`, aliasing the
    /// artifact.
    #[inline]
    pub fn template_field_name(
        &self,
        chunk: usize,
        index: usize,
        field: usize,
    ) -> Option<&'a [u8]> {
        let c = self.module.chunk(chunk)?;
        if index >= c.templates_count as usize {
            return None;
        }
        self.layouts
            .template_field_name(c.templates_first as usize + index, field)
    }

    /// Runtime word width, as log2 of the bit width.
    #[inline]
    pub fn word_bits_log2(&self) -> Option<u8> {
        Some(self.module.header()?.word_bits_log2)
    }

    /// Runtime float width, log2 form.
    #[inline]
    pub fn float_bits_log2(&self) -> Option<u8> {
        Some(self.module.header()?.float_bits_log2)
    }

    /// Schema hash, used to reject an incompatible hot swap.
    #[inline]
    pub fn schema_hash(&self) -> Option<u32> {
        Some(self.module.header()?.schema_hash)
    }

    /// Shared-partition byte count.
    #[inline]
    pub fn shared_data_bytes(&self) -> Option<u32> {
        Some(self.module.header()?.shared_data_bytes)
    }

    /// Number of enum layouts.
    #[inline]
    pub fn enum_layout_count(&self) -> usize {
        self.layouts.layout_count()
    }

    /// Enum layout `index`: its type name, aliasing the artifact.
    #[inline]
    pub fn enum_type_name(&self, index: usize) -> Option<&'a [u8]> {
        self.layouts
            .name_bytes(self.layouts.layout(index)?.type_name)
    }

    /// Variant `variant` of enum layout `index`: name bytes and discriminant.
    #[inline]
    pub fn enum_variant(&self, index: usize, variant: usize) -> Option<(&'a [u8], i64)> {
        self.layouts.layout_variant(index, variant)
    }

    /// The data-segment layout, if the module declares one.
    #[inline]
    pub fn data_layout(&self) -> Option<DataLayoutTable<'a>> {
        self.data
    }

    /// Number of opcode records in chunk `chunk`'s body.
    #[inline]
    pub fn op_record_count(&self, chunk: usize) -> Option<u32> {
        Some(self.module.chunk(chunk)?.op_record_count)
    }

    /// Number of native functions the module references.
    #[inline]
    pub fn native_count(&self) -> usize {
        self.module.native_count()
    }

    /// Native `index`'s name, aliasing the artifact.
    #[inline]
    pub fn native_name_bytes(&self, index: usize) -> Option<&'a [u8]> {
        self.module.native_name(index)
    }

    /// Number of fields in chunk `chunk`'s struct template `index`.
    #[inline]
    pub fn template_field_count(&self, chunk: usize, index: usize) -> Option<u32> {
        let c = self.module.chunk(chunk)?;
        if index >= c.templates_count as usize {
            return None;
        }
        Some(
            self.layouts
                .template(c.templates_first as usize + index)?
                .field_count,
        )
    }

    /// Padded-body payload size for enum layout `index`; zero for a non-flat enum.
    #[inline]
    pub fn enum_min_payload(&self, index: usize) -> Option<u32> {
        Some(self.layouts.layout(index)?.min_payload)
    }

    /// Number of variants in enum layout `index`.
    #[inline]
    pub fn enum_variant_count(&self, index: usize) -> Option<u32> {
        Some(self.layouts.layout(index)?.variants_count)
    }

    /// Parameter `index`'s type tag for chunk `chunk`.
    #[inline]
    pub fn param_type(&self, chunk: usize, index: usize) -> Option<TypeTag> {
        let c = self.module.chunk(chunk)?;
        if index >= c.param_types_count as usize {
            return None;
        }
        let raw = self
            .param_types?
            .tag_bytes((c.param_types_first, c.param_types_count))?;
        type_tag_from_byte(*raw.get(index)?)
    }

    /// The underlying module table, for reads this view does not wrap.
    #[inline]
    pub fn module(&self) -> ModuleTable<'a> {
        self.module
    }
}

/// Byte ranges of every region [`AuxView`] needs, resolved once.
///
/// # Why this exists
///
/// [`AuxView::parse`] walks the region directory and validates every table. That
/// is right once per module load and wrong per access — the runtime reads a
/// constant on every `LoadConst`, and today's `rkyv` accessor is an unchecked
/// pointer cast, so a directory walk per read would be a real regression on the
/// hot path.
///
/// Resolving to plain byte ranges separates the two costs: validate once at load
/// producing an `AuxOffsets`, then rebuild the view per access by slicing, which
/// is a handful of bounds checks and no directory walk.
///
/// Carries no borrow, so a caller can store it beside the bytecode image without
/// a self-referential struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuxOffsets {
    pool: (usize, usize),
    names: (usize, usize),
    consts: Option<(usize, usize)>,
    struct_aux: Option<(usize, usize)>,
    enum_aux: Option<(usize, usize)>,
    templates: Option<(usize, usize)>,
    variants: Option<(usize, usize)>,
    layouts: Option<(usize, usize)>,
    chunks: Option<(usize, usize)>,
    natives: Option<(usize, usize)>,
    header: Option<(usize, usize)>,
    debug: Option<(usize, usize)>,
    native_returns: Option<(usize, usize)>,
    data_slots: Option<(usize, usize)>,
    shared_layout: Option<(usize, usize)>,
    private_composite: Option<(usize, usize)>,
    data_init: Option<(usize, usize)>,
    param_types: Option<(usize, usize)>,
}

fn region_span(view: &WireView<'_>, kind: u16) -> Result<Option<(usize, usize)>, SchemaError> {
    let Some(r) = view.find_region(kind) else {
        return Ok(None);
    };
    let start = r.byte_offset().ok_or(SchemaError::BadIndex)?;
    let len = r.byte_length().ok_or(SchemaError::BadIndex)?;
    Ok(Some((start, len)))
}

impl AuxOffsets {
    /// Validates an artifact and records where every region lives.
    ///
    /// Run this once per module load. The returned offsets are only meaningful
    /// for the exact bytes they were resolved from.
    ///
    /// # Errors
    ///
    /// [`SchemaError`] for any malformed artifact.
    pub fn resolve(bytes: &[u8]) -> Result<Self, SchemaError> {
        // Full validation happens here, so the per-access path can skip it.
        AuxView::parse(bytes)?;

        let view = WireView::parse(bytes)?;
        let required = |k: u16| -> Result<(usize, usize), SchemaError> {
            region_span(&view, k)?.ok_or(SchemaError::MissingRegion(k))
        };
        Ok(Self {
            pool: required(kind::STRING_POOL)?,
            names: required(kind::NAMES)?,
            consts: region_span(&view, kind::CONSTS)?,
            struct_aux: region_span(&view, kind::STRUCT_AUX)?,
            enum_aux: region_span(&view, kind::ENUM_AUX)?,
            templates: region_span(&view, kind::STRUCT_TEMPLATES)?,
            variants: region_span(&view, kind::ENUM_VARIANTS)?,
            layouts: region_span(&view, kind::ENUM_LAYOUTS)?,
            chunks: region_span(&view, kind::CHUNKS)?,
            natives: region_span(&view, kind::NATIVES)?,
            header: region_span(&view, kind::HEADER)?,
            debug: region_span(&view, kind::DEBUG_POOL)?,
            native_returns: region_span(&view, kind::NATIVE_RETURNS)?,
            data_slots: region_span(&view, kind::DATA_SLOTS)?,
            shared_layout: region_span(&view, kind::SHARED_LAYOUT)?,
            private_composite: region_span(&view, kind::PRIVATE_COMPOSITE)?,
            data_init: region_span(&view, kind::DATA_INIT)?,
            param_types: region_span(&view, kind::PARAM_TYPES)?,
        })
    }
}

impl<'a> AuxView<'a> {
    /// Rebuilds a view from already-resolved offsets, **without** re-validating.
    ///
    /// The fast path. [`AuxOffsets::resolve`] does the directory walk and the
    /// validation once; this slices the recorded ranges, which is a handful of
    /// bounds checks.
    ///
    /// # Correctness
    ///
    /// `offsets` must have been resolved from these exact `bytes`. Passing
    /// offsets from a different artifact is not unsafe — every slice is
    /// bounds-checked and this returns `None` on any mismatch — but the result
    /// would be meaningless, so the caller is expected to keep the two together.
    pub fn from_offsets(bytes: &'a [u8], offsets: &AuxOffsets) -> Option<Self> {
        let span = |r: (usize, usize)| bytes.get(r.0..r.0.checked_add(r.1)?);
        let opt_span = |r: Option<(usize, usize)>| -> Option<Option<&'a [u8]>> {
            match r {
                None => Some(None),
                Some(x) => span(x).map(Some),
            }
        };
        let table = |b: Option<&'a [u8]>, stride: usize| -> Option<Option<RecordTable<'a>>> {
            match b {
                None => Some(None),
                Some(x) => RecordTable::from_bytes(x, stride).map(Some),
            }
        };

        let pool = Pool::from_bytes(span(offsets.pool)?);
        let names = RecordTable::from_bytes(span(offsets.names)?, NameRef::STRIDE_BYTES)?;

        let consts_b = opt_span(offsets.consts)?;
        let struct_aux_b = opt_span(offsets.struct_aux)?;
        let enum_aux_b = opt_span(offsets.enum_aux)?;

        // The constant tables travel together: a module either carries all three
        // or none, which is what `add_constants` emits.
        let consts = match (consts_b, struct_aux_b, enum_aux_b) {
            (Some(c), Some(s), Some(e)) => Some(ConstTable {
                pool,
                names,
                consts: RecordTable::from_bytes(c, ConstRecord::STRIDE_BYTES)?,
                struct_aux: RecordTable::from_bytes(s, StructAux::STRIDE_BYTES)?,
                enum_aux: RecordTable::from_bytes(e, EnumAux::STRIDE_BYTES)?,
            }),
            _ => None,
        };

        let layouts = LayoutTable {
            pool,
            names,
            templates: table(
                opt_span(offsets.templates)?,
                StructTemplateRecord::STRIDE_BYTES,
            )?,
            variants: table(opt_span(offsets.variants)?, EnumVariantRecord::STRIDE_BYTES)?,
            layouts: table(opt_span(offsets.layouts)?, EnumLayoutRecord::STRIDE_BYTES)?,
        };

        let module = ModuleTable {
            pool,
            names,
            chunks: table(opt_span(offsets.chunks)?, ChunkRecord::STRIDE_BYTES)?,
            natives: table(opt_span(offsets.natives)?, NativeRecord::STRIDE_BYTES)?,
            native_returns: table(
                opt_span(offsets.native_returns)?,
                NativeReturnRecord::STRIDE_BYTES,
            )?,
            header: match opt_span(offsets.header)? {
                Some(h) => RecordTable::from_bytes(h, HeaderRecord::STRIDE_BYTES)?
                    .get_as::<HeaderRecord>(0),
                None => None,
            },
            debug: opt_span(offsets.debug)?.map(Pool::from_bytes),
        };

        // The data layout is present only when the module declares one; region
        // presence is what carries that distinction, so all four spans travel
        // together exactly as `add_data_layout` emits them.
        let data = match (
            opt_span(offsets.data_slots)?,
            opt_span(offsets.shared_layout)?,
            opt_span(offsets.private_composite)?,
            opt_span(offsets.data_init)?,
        ) {
            (Some(slots), Some(shared), Some(pc), Some(init_b)) => {
                let init_table = RecordTable::from_bytes(init_b, DataInitRecord::STRIDE_BYTES)?;
                Some(DataLayoutTable {
                    pool,
                    names,
                    slots: RecordTable::from_bytes(slots, DataSlotRecord::STRIDE_BYTES)?,
                    shared: RecordTable::from_bytes(shared, SharedSlotRecord::STRIDE_BYTES)?,
                    private_composite: RecordTable::from_bytes(
                        pc,
                        PrivateCompositeRecord::STRIDE_BYTES,
                    )?,
                    init: init_table.get_as::<DataInitRecord>(0)?,
                })
            }
            _ => None,
        };

        Some(Self {
            module,
            consts,
            layouts,
            data,
            param_types: opt_span(offsets.param_types)?.map(ParamTypeTable::from_bytes),
        })
    }
}
