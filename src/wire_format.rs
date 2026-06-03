//! V0.2.0 wire format types and codec.
//!
//! This module ships the wire-format types specified in
//! [`docs/architecture/WIRE_FORMAT.md`](../../docs/architecture/WIRE_FORMAT.md):
//! the sixty-four-byte framing header, the fixed-size four-byte
//! opcode records with their parity bit, and the eight-byte
//! operand pool entries that carry compound operands referenced
//! from the opcode stream.
//!
//! V0.2.0 Phase 7a publishes the types and the per-op codec; the
//! execution loop, [`crate::bytecode::Module::to_bytes`], and
//! [`crate::bytecode::Module::from_bytes`] continue to round-trip
//! through rkyv until Phase 7b cuts the producer and consumer over
//! to the section-partitioned body. The phased cutover preserves
//! the existing test surface while the new format gains coverage
//! through the round-trip tests in this module.
//!
//! The encoder and decoder operate on individual `Op` values. A
//! producer that wants a full module wire-format buffer assembles
//! the opcode records and operand pool entries side by side, then
//! frames them with the sixty-four-byte header and the CRC-32
//! trailer. The full module producer is the load-bearing piece of
//! Phase 7b.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use rkyv::{Archive, Deserialize, Serialize};

use crate::bytecode::{
    BYTECODE_MAGIC, BYTECODE_VERSION, BlockType, Chunk, ConstValue, DataLayout, LoadError, Module,
    Op, StructTemplate, TypeTag, crc32,
};

/// Error surfaced by the wire-format decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireFormatError {
    /// The record's parity bit did not match the parity of its
    /// payload. Detected single-bit corruption of the four-byte
    /// record.
    OpcodeRecordParityMismatch,
    /// The opcode identifier in the record did not match any
    /// known [`Op`] variant. The runtime treats this as a
    /// corrupted record rather than a valid opcode whose
    /// dispatch is not implemented.
    UnknownOpcodeId(u8),
    /// The pool index encoded in the opcode record exceeded the
    /// supplied pool length. The record references an entry that
    /// does not exist.
    OperandPoolIndexOutOfBounds(usize),
    /// The operand pool entry's parity byte did not match the
    /// parity of its payload. Detected single-bit corruption of
    /// the eight-byte entry.
    OperandPoolParityMismatch,
    /// The operand pool entry's type tag did not match the
    /// expected tag for the consuming opcode. Either the
    /// producer assembled the pool incorrectly or the entry was
    /// corrupted.
    OperandPoolTagMismatch {
        /// Tag observed in the pool entry.
        observed: u8,
        /// Tag the consuming opcode requires.
        expected: u8,
    },
    /// The operand pool index did not fit in the three inline
    /// bytes of the opcode record. The pool exceeds 16,777,216
    /// entries which is well beyond any observed program; the
    /// producer rejects such modules at encode time.
    OperandPoolIndexOverflow,
    /// The scalar-kind tag in a baked flat-composite access operand
    /// (`Op::GetTupleField` or `Op::GetIndex`) did not map to a known
    /// [`crate::value_layout::ScalarKind`]. Either the producer
    /// assembled the operand incorrectly or the record was corrupted.
    /// The boxed sentinel (`255`) is handled separately and is not
    /// reported here.
    TupleFieldKindUnknown(u8),
}

/// Opcode identifier as it appears in the seven low bits of byte
/// zero of a [`OpcodeRecord`]. The mapping is fixed at version 1
/// of the wire format. Adding a new opcode appends to the table;
/// removing an opcode keeps the identifier vacant rather than
/// shifting later identifiers, preserving wire-format stability
/// for unaffected opcodes.
///
/// V0.2.0 ISA uses identifiers in the range `[0, 68]`; the upper
/// bound is one less than the number of variants. The seven-bit
/// field admits up to `128` identifiers before the version field
/// would need to bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcodeId(pub u8);

/// Pool entry type tag for the `(u16, u16)` shape. Used by
/// `Op::GetDataIndexed`, `Op::SetDataIndexed`, and
/// `Op::IsEnum`.
pub const POOL_TAG_U16_U16: u8 = 0x01;

/// Pool entry type tag for the `(u16, u16, u8)` shape. Used by
/// `Op::NewEnum`.
pub const POOL_TAG_U16_U16_U8: u8 = 0x02;

/// Byte-three sentinel in a baked `Op::GetTupleField` record marking
/// the boxed (positional-index) form. Distinguished from a flat
/// scalar-kind tag because [`crate::value_layout::ScalarKind::to_tag`]
/// returns values in `0..=7`, well below `255`. For the flat form
/// byte three holds the scalar-kind tag and bytes one and two hold
/// the little-endian offset; for the boxed form byte one holds the
/// index.
pub const TUPLE_FIELD_BOXED_SENTINEL: u8 = 0xFF;

/// Width of an opcode record in bytes.
pub const OPCODE_RECORD_BYTES: usize = 4;

/// Width of an operand pool entry in bytes.
pub const OPERAND_POOL_ENTRY_BYTES: usize = 8;

/// Width of the framing header in bytes for unsigned modules.
/// Signed modules grow the header by a signature-extension block;
/// the decoder reads the actual header length from bytes 6..8.
pub const WIRE_FORMAT_HEADER_BYTES: usize = 64;

/// Width of the signature-extension metadata block that follows
/// the base header on signed modules. Bytes are laid out as:
///
/// - byte 0: `scheme_id` (u8). `0` is reserved-invalid; `1` is
///   Ed25519. Other values are reserved for future schemes
///   tracked in `secret/SIGNATURE_SCHEME_MIGRATION.md`.
/// - byte 1: reserved (must be `0`).
/// - bytes 2..4: `signature_length` (u16 little-endian).
/// - bytes 4..8: reserved (must be `0`).
pub const SIGNATURE_METADATA_BYTES: usize = 8;

/// Bit in the framing header's `flags: u8` byte (offset 15) that
/// signals "this module must be loaded only after a successful
/// signature verification." The flag is set by the compiler when
/// the entry function carries the `signed` modifier and is
/// enforced by the load-time runtime against the host-supplied
/// trust matrix.
pub const FLAG_REQUIRES_SIGNATURE: u8 = 0x02;

/// Bit in the framing header's `flags: u8` byte (offset 15) that
/// signals "this module's body is encrypted." The flag is set by
/// the compiler when producing an encrypted artefact and is
/// enforced by the load-time runtime which decrypts the body
/// before structural verification. Encrypted modules are also
/// signed because the signature covers the encrypted body and is
/// what authenticates the artefact's origin against tampering on
/// the delivery channel.
pub const FLAG_ENCRYPTED: u8 = 0x04;

/// Signature scheme identifier for the Ed25519 algorithm. The
/// only scheme V0.2.0 implements; the byte exists to make the
/// wire format scheme-agnostic so future migrations do not
/// require an ABI break.
pub const SIGNATURE_SCHEME_ED25519: u8 = 0x01;

/// Length of an Ed25519 signature in bytes.
pub const ED25519_SIGNATURE_BYTES: usize = 64;

/// Length of an Ed25519 verifying key in bytes.
pub const ED25519_VERIFYING_KEY_BYTES: usize = 32;

/// Width of the encryption-metadata block appended after the
/// signature extension on encrypted modules. The layout is
/// documented in [`crate::encryption::EncryptionMetadata`] and in
/// `tmp/encrypted_signed_modules.md`. Always 88 bytes for the
/// V0.2.1 X25519+AES-256-GCM scheme; future schemes may use the
/// same constant or extend it through a scheme-specific
/// negotiation.
pub const ENCRYPTION_METADATA_BYTES: usize = 88;

/// Maximum operand pool size addressable by the twenty-four-bit
/// inline pool index. `16_777_216`.
pub const MAX_POOL_ENTRIES: usize = 1 << 24;

/// A four-byte opcode record. Byte zero carries the parity bit
/// (high) and the opcode identifier (low seven bits). Bytes one
/// through three carry either the inline operand or the
/// little-endian operand-pool index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcodeRecord(pub [u8; OPCODE_RECORD_BYTES]);

/// An eight-byte operand pool entry. Byte zero carries the type
/// tag; byte one carries the parity byte; bytes two through seven
/// carry the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperandPoolEntry(pub [u8; OPERAND_POOL_ENTRY_BYTES]);

impl OpcodeRecord {
    /// Construct a record from its raw four bytes. The caller is
    /// responsible for the parity bit; see [`Self::from_id_and_operand`]
    /// for the parity-computing constructor used by the encoder.
    pub fn from_raw(bytes: [u8; OPCODE_RECORD_BYTES]) -> Self {
        OpcodeRecord(bytes)
    }

    /// Construct a record from an opcode identifier and three
    /// inline operand bytes. Computes the parity bit and writes
    /// it to the high bit of byte zero.
    pub fn from_id_and_operand(id: OpcodeId, operand: [u8; 3]) -> Self {
        let raw = [id.0 & 0x7F, operand[0], operand[1], operand[2]];
        let parity = compute_parity(&raw);
        let mut bytes = raw;
        bytes[0] |= parity << 7;
        OpcodeRecord(bytes)
    }

    /// Extract the opcode identifier from byte zero, ignoring the
    /// parity bit.
    pub fn opcode_id(&self) -> OpcodeId {
        OpcodeId(self.0[0] & 0x7F)
    }

    /// Extract the three inline operand bytes (bytes one through
    /// three).
    pub fn operand_bytes(&self) -> [u8; 3] {
        [self.0[1], self.0[2], self.0[3]]
    }

    /// Verify that the parity bit matches the parity of the rest
    /// of the record. Returns `Ok` on match, `Err` on mismatch.
    pub fn check_parity(&self) -> Result<(), WireFormatError> {
        let masked = [self.0[0] & 0x7F, self.0[1], self.0[2], self.0[3]];
        let expected = (self.0[0] >> 7) & 1;
        if compute_parity(&masked) == expected {
            Ok(())
        } else {
            Err(WireFormatError::OpcodeRecordParityMismatch)
        }
    }

    /// Decode the inline operand as a little-endian `u8` in byte
    /// one. Bytes two and three are not consulted.
    pub fn operand_u8(&self) -> u8 {
        self.0[1]
    }

    /// Decode the inline operand as a little-endian `u16` from
    /// bytes one through two.
    pub fn operand_u16(&self) -> u16 {
        u16::from_le_bytes([self.0[1], self.0[2]])
    }

    /// Decode the inline operand as `(u16, u8)` where the `u16`
    /// is little-endian in bytes one through two and the `u8` is
    /// byte three.
    pub fn operand_u16_u8(&self) -> (u16, u8) {
        (u16::from_le_bytes([self.0[1], self.0[2]]), self.0[3])
    }

    /// Decode the inline operand as a twenty-four-bit
    /// little-endian pool index. The producer guarantees the
    /// pool fits in twenty-four bits.
    pub fn operand_pool_index(&self) -> u32 {
        u32::from_le_bytes([self.0[1], self.0[2], self.0[3], 0])
    }
}

impl OperandPoolEntry {
    /// Construct a `(u16, u16)` pool entry. Computes the parity
    /// byte over the type tag and the payload.
    pub fn from_u16_u16(a: u16, b: u16) -> Self {
        let mut bytes = [0u8; OPERAND_POOL_ENTRY_BYTES];
        bytes[0] = POOL_TAG_U16_U16;
        // Byte 1 is the parity; computed below.
        bytes[2..4].copy_from_slice(&a.to_le_bytes());
        bytes[4..6].copy_from_slice(&b.to_le_bytes());
        // Bytes 6 and 7 stay zero for the `(u16, u16)` shape.
        let parity_payload = [
            bytes[0], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ];
        bytes[1] = pool_entry_parity(&parity_payload);
        OperandPoolEntry(bytes)
    }

    /// Construct a `(u16, u16, u8)` pool entry. Computes the
    /// parity byte over the type tag and the payload.
    pub fn from_u16_u16_u8(a: u16, b: u16, c: u8) -> Self {
        let mut bytes = [0u8; OPERAND_POOL_ENTRY_BYTES];
        bytes[0] = POOL_TAG_U16_U16_U8;
        bytes[2..4].copy_from_slice(&a.to_le_bytes());
        bytes[4..6].copy_from_slice(&b.to_le_bytes());
        bytes[6] = c;
        // Byte 7 stays zero (reserved).
        let parity_payload = [
            bytes[0], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ];
        bytes[1] = pool_entry_parity(&parity_payload);
        OperandPoolEntry(bytes)
    }

    /// Verify the parity byte against the rest of the entry.
    pub fn check_parity(&self) -> Result<(), WireFormatError> {
        let parity_payload = [
            self.0[0], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7],
        ];
        if pool_entry_parity(&parity_payload) == self.0[1] {
            Ok(())
        } else {
            Err(WireFormatError::OperandPoolParityMismatch)
        }
    }

    /// Return the type tag (byte zero).
    pub fn tag(&self) -> u8 {
        self.0[0]
    }

    /// Decode the `(u16, u16)` payload. Caller verifies the tag
    /// and parity before invoking.
    pub fn as_u16_u16(&self) -> (u16, u16) {
        (
            u16::from_le_bytes([self.0[2], self.0[3]]),
            u16::from_le_bytes([self.0[4], self.0[5]]),
        )
    }

    /// Decode the `(u16, u16, u8)` payload. Caller verifies the
    /// tag and parity before invoking.
    pub fn as_u16_u16_u8(&self) -> (u16, u16, u8) {
        let (a, b) = self.as_u16_u16();
        (a, b, self.0[6])
    }
}

/// Compute the parity bit over a four-byte slice. Returns `0` or
/// `1`. The high bit of byte zero must be cleared before calling
/// when computing the parity of an opcode record.
fn compute_parity(bytes: &[u8; OPCODE_RECORD_BYTES]) -> u8 {
    let total = bytes.iter().map(|b| b.count_ones()).sum::<u32>();
    (total & 1) as u8
}

/// Compute the parity byte over a seven-byte slice covering the
/// type tag and the payload of an [`OperandPoolEntry`]. Returns
/// the XOR of the seven bytes; a consumer compares this against
/// the parity byte stored in byte one of the entry.
fn pool_entry_parity(bytes: &[u8; 7]) -> u8 {
    bytes.iter().fold(0u8, |acc, b| acc ^ b)
}

/// Canonical opcode identifier table. The mapping is fixed at
/// version 1 of the wire format. The list mirrors the
/// declaration order of the `Op` enum in `src/bytecode.rs` and is
/// part of the wire-format ABI: adding a new opcode appends to
/// the tail of the list, and removing an opcode keeps the
/// vacated identifier reserved rather than shifting later
/// identifiers.
///
/// The Phase 7a release ships the table as a `const` array
/// indexed by an internal discriminator computed by [`opcode_id_of`].
/// Phase 7b transitions to a derived mapping that the compile-
/// pipeline runs once at module load.
const OPCODE_ID_TABLE: &[(&str, u8)] = &[
    ("Const", 0),
    ("GetLocal", 1),
    ("SetLocal", 2),
    ("GetData", 3),
    ("SetData", 4),
    ("GetDataIndexed", 5),
    ("SetDataIndexed", 6),
    ("BoundsCheck", 7),
    ("Add", 8),
    ("Sub", 9),
    ("Mul", 10),
    ("Div", 11),
    ("Mod", 12),
    ("Neg", 13),
    ("CmpEq", 14),
    ("CmpNe", 15),
    ("CmpLt", 16),
    ("CmpGt", 17),
    ("CmpLe", 18),
    ("CmpGe", 19),
    ("Not", 20),
    ("If", 21),
    ("Else", 22),
    ("EndIf", 23),
    ("Loop", 24),
    ("EndLoop", 25),
    ("Break", 26),
    ("BreakIf", 27),
    ("Stream", 28),
    ("Reset", 29),
    ("Call", 30),
    ("Return", 31),
    ("Yield", 32),
    ("Dup", 33),
    ("NewStruct", 34),
    ("NewEnum", 35),
    ("NewArray", 36),
    ("NewTuple", 37),
    ("GetField", 38),
    ("GetIndex", 39),
    ("GetTupleField", 40),
    ("GetEnumField", 41),
    ("Len", 42),
    ("IsEnum", 43),
    ("IsStruct", 44),
    ("IntToFloat", 45),
    ("FloatToInt", 46),
    ("WordToByte", 47),
    ("ByteToWord", 48),
    ("WordToFixed", 49),
    ("FixedToWord", 50),
    ("FixedMul", 51),
    ("FixedDiv", 52),
    ("Trap", 53),
    ("CheckedAdd", 54),
    ("CheckedSub", 55),
    ("CheckedMul", 56),
    ("CheckedNeg", 57),
    ("CheckedDiv", 58),
    ("CheckedMod", 59),
    ("PushImmediate", 60),
    ("PopN", 61),
    ("BitAnd", 62),
    ("BitOr", 63),
    ("BitXor", 64),
    ("Shl", 65),
    ("Shr", 66),
    ("CallVerifiedNative", 67),
    ("CallExternalNative", 68),
];

/// Return the wire-format identifier for an `Op` variant.
pub fn opcode_id_of(op: &Op) -> OpcodeId {
    let id = match op {
        Op::Const(_) => 0,
        Op::GetLocal(_) => 1,
        Op::SetLocal(_) => 2,
        Op::GetData(_) => 3,
        Op::SetData(_) => 4,
        Op::GetDataIndexed(_, _) => 5,
        Op::SetDataIndexed(_, _) => 6,
        Op::BoundsCheck(_) => 7,
        Op::Add => 8,
        Op::Sub => 9,
        Op::Mul => 10,
        Op::Div => 11,
        Op::Mod => 12,
        Op::Neg => 13,
        Op::CmpEq => 14,
        Op::CmpNe => 15,
        Op::CmpLt => 16,
        Op::CmpGt => 17,
        Op::CmpLe => 18,
        Op::CmpGe => 19,
        Op::Not => 20,
        Op::If(_) => 21,
        Op::Else(_) => 22,
        Op::EndIf => 23,
        Op::Loop(_) => 24,
        Op::EndLoop(_) => 25,
        Op::Break(_) => 26,
        Op::BreakIf(_) => 27,
        Op::Stream => 28,
        Op::Reset => 29,
        Op::Call(_, _) => 30,
        Op::Return => 31,
        Op::Yield => 32,
        Op::Dup => 33,
        Op::NewStruct(_) => 34,
        Op::NewEnum(_, _, _) => 35,
        Op::NewArray(_) => 36,
        Op::NewTuple(_) => 37,
        Op::GetField(_) => 38,
        Op::GetIndex(_) => 39,
        Op::GetTupleField(_) => 40,
        Op::GetEnumField(_) => 41,
        Op::Len => 42,
        Op::IsEnum(_, _) => 43,
        Op::IsStruct(_) => 44,
        Op::IntToFloat => 45,
        Op::FloatToInt => 46,
        Op::WordToByte => 47,
        Op::ByteToWord => 48,
        Op::WordToFixed(_) => 49,
        Op::FixedToWord(_) => 50,
        Op::FixedMul(_) => 51,
        Op::FixedDiv(_) => 52,
        Op::Trap(_) => 53,
        Op::CheckedAdd => 54,
        Op::CheckedSub => 55,
        Op::CheckedMul(_) => 56,
        Op::CheckedNeg => 57,
        Op::CheckedDiv(_) => 58,
        Op::CheckedMod => 59,
        Op::PushImmediate(_) => 60,
        Op::PopN(_) => 61,
        Op::BitAnd => 62,
        Op::BitOr => 63,
        Op::BitXor => 64,
        Op::Shl => 65,
        Op::Shr => 66,
        Op::CallVerifiedNative(_, _) => 67,
        Op::CallExternalNative(_, _) => 68,
    };
    OpcodeId(id)
}

/// Look up the variant name for an opcode identifier. Used by
/// the decoder's error path to surface readable diagnostics.
fn opcode_name_for_id(id: u8) -> Option<&'static str> {
    OPCODE_ID_TABLE
        .iter()
        .find_map(|(name, code)| if *code == id { Some(*name) } else { None })
}

/// Encode an [`Op`] into an [`OpcodeRecord`], appending an entry
/// to `pool` for the four compound-operand opcodes
/// (`GetDataIndexed`, `SetDataIndexed`, `IsEnum`, `NewEnum`).
///
/// The pool index of an appended entry is the entry's position
/// in `pool` before the append. The encoder rejects modules
/// whose pool exceeds [`MAX_POOL_ENTRIES`] entries with
/// `WireFormatError::OperandPoolIndexOverflow`.
pub fn encode_op(
    op: &Op,
    pool: &mut Vec<OperandPoolEntry>,
) -> Result<OpcodeRecord, WireFormatError> {
    let id = opcode_id_of(op);
    let operand = match op {
        // No operand. All three operand bytes are zero.
        Op::Add
        | Op::Sub
        | Op::Mul
        | Op::Div
        | Op::Mod
        | Op::Neg
        | Op::CmpEq
        | Op::CmpNe
        | Op::CmpLt
        | Op::CmpGt
        | Op::CmpLe
        | Op::CmpGe
        | Op::Not
        | Op::EndIf
        | Op::Stream
        | Op::Reset
        | Op::Return
        | Op::Yield
        | Op::Dup
        | Op::Len
        | Op::IntToFloat
        | Op::FloatToInt
        | Op::WordToByte
        | Op::ByteToWord
        | Op::CheckedAdd
        | Op::CheckedSub
        | Op::CheckedNeg
        | Op::CheckedMod
        | Op::BitAnd
        | Op::BitOr
        | Op::BitXor
        | Op::Shl
        | Op::Shr => [0, 0, 0],

        // Baked enum-payload access (B28 P2). Same inline layout as the
        // struct/tuple field forms: the flat form stores the offset
        // little-endian in bytes one and two and the scalar-kind tag in
        // byte three; the boxed form stores the index in byte one and the
        // boxed sentinel in byte three.
        Op::GetEnumField(crate::bytecode::EnumField::Flat { offset, kind }) => {
            let b = offset.to_le_bytes();
            [b[0], b[1], kind.to_tag()]
        }
        Op::GetEnumField(crate::bytecode::EnumField::Boxed { index }) => {
            [*index, 0, TUPLE_FIELD_BOXED_SENTINEL]
        }

        // `u8` operand carried inline in byte one.
        Op::NewTuple(n)
        | Op::WordToFixed(n)
        | Op::FixedToWord(n)
        | Op::FixedMul(n)
        | Op::FixedDiv(n)
        | Op::CheckedMul(n)
        | Op::CheckedDiv(n)
        | Op::PushImmediate(n)
        | Op::PopN(n) => [*n, 0, 0],

        // `u16` operand carried little-endian in bytes one and two.
        Op::Const(v)
        | Op::GetLocal(v)
        | Op::SetLocal(v)
        | Op::GetData(v)
        | Op::SetData(v)
        | Op::BoundsCheck(v)
        | Op::If(v)
        | Op::Else(v)
        | Op::Loop(v)
        | Op::EndLoop(v)
        | Op::Break(v)
        | Op::BreakIf(v)
        | Op::NewStruct(v)
        | Op::NewArray(v)
        | Op::IsStruct(v)
        | Op::Trap(v) => {
            let b = v.to_le_bytes();
            [b[0], b[1], 0]
        }

        // `(u16, u8)` operand carried inline: u16 little-endian in
        // bytes one and two, u8 in byte three.
        Op::Call(c, n) | Op::CallVerifiedNative(c, n) | Op::CallExternalNative(c, n) => {
            let b = c.to_le_bytes();
            [b[0], b[1], *n]
        }

        // Baked tuple-field access (B28 P2). The flat form stores the
        // little-endian offset in bytes one and two and the scalar-kind
        // tag in byte three. The boxed form stores the index in byte
        // one and the boxed sentinel in byte three.
        Op::GetTupleField(crate::bytecode::TupleField::Flat { offset, kind }) => {
            let b = offset.to_le_bytes();
            [b[0], b[1], kind.to_tag()]
        }
        Op::GetTupleField(crate::bytecode::TupleField::Boxed { index }) => {
            [*index, 0, TUPLE_FIELD_BOXED_SENTINEL]
        }

        // Baked struct-field access (B28 P2). Both forms carry a u16 in
        // bytes one and two; byte three discriminates: a scalar-kind tag
        // for the flat read, or the boxed sentinel for the by-name lookup.
        Op::GetField(crate::bytecode::StructField::Flat { offset, kind }) => {
            let b = offset.to_le_bytes();
            [b[0], b[1], kind.to_tag()]
        }
        Op::GetField(crate::bytecode::StructField::Boxed { name_const }) => {
            let b = name_const.to_le_bytes();
            [b[0], b[1], TUPLE_FIELD_BOXED_SENTINEL]
        }

        // Baked array element access (B28 P2). The flat form stores the
        // element scalar-kind tag in byte one; the runtime derives the
        // element size and the offset from the index. The boxed form
        // stores the boxed sentinel in byte one.
        Op::GetIndex(crate::bytecode::ArrayElem::Flat { kind }) => [kind.to_tag(), 0, 0],
        Op::GetIndex(crate::bytecode::ArrayElem::Boxed) => [TUPLE_FIELD_BOXED_SENTINEL, 0, 0],

        // Pool-using shapes. Append the entry and store the index
        // in the inline operand bytes.
        Op::GetDataIndexed(a, b) | Op::SetDataIndexed(a, b) | Op::IsEnum(a, b) => {
            let idx = pool.len();
            if idx >= MAX_POOL_ENTRIES {
                return Err(WireFormatError::OperandPoolIndexOverflow);
            }
            pool.push(OperandPoolEntry::from_u16_u16(*a, *b));
            let idx_bytes = (idx as u32).to_le_bytes();
            [idx_bytes[0], idx_bytes[1], idx_bytes[2]]
        }
        Op::NewEnum(a, b, c) => {
            let idx = pool.len();
            if idx >= MAX_POOL_ENTRIES {
                return Err(WireFormatError::OperandPoolIndexOverflow);
            }
            pool.push(OperandPoolEntry::from_u16_u16_u8(*a, *b, *c));
            let idx_bytes = (idx as u32).to_le_bytes();
            [idx_bytes[0], idx_bytes[1], idx_bytes[2]]
        }
    };
    Ok(OpcodeRecord::from_id_and_operand(id, operand))
}

/// Decode an [`OpcodeRecord`] into an [`Op`], consulting the
/// supplied operand pool for the four compound-operand opcodes.
///
/// Verifies the record's parity before dispatching. Pool-using
/// opcodes additionally verify the pool entry's parity and type
/// tag.
pub fn decode_op(record: OpcodeRecord, pool: &[OperandPoolEntry]) -> Result<Op, WireFormatError> {
    record.check_parity()?;
    let id = record.opcode_id().0;
    let op = match id {
        0 => Op::Const(record.operand_u16()),
        1 => Op::GetLocal(record.operand_u16()),
        2 => Op::SetLocal(record.operand_u16()),
        3 => Op::GetData(record.operand_u16()),
        4 => Op::SetData(record.operand_u16()),
        5 => {
            let (a, b) = decode_pool_u16_u16(record, pool)?;
            Op::GetDataIndexed(a, b)
        }
        6 => {
            let (a, b) = decode_pool_u16_u16(record, pool)?;
            Op::SetDataIndexed(a, b)
        }
        7 => Op::BoundsCheck(record.operand_u16()),
        8 => Op::Add,
        9 => Op::Sub,
        10 => Op::Mul,
        11 => Op::Div,
        12 => Op::Mod,
        13 => Op::Neg,
        14 => Op::CmpEq,
        15 => Op::CmpNe,
        16 => Op::CmpLt,
        17 => Op::CmpGt,
        18 => Op::CmpLe,
        19 => Op::CmpGe,
        20 => Op::Not,
        21 => Op::If(record.operand_u16()),
        22 => Op::Else(record.operand_u16()),
        23 => Op::EndIf,
        24 => Op::Loop(record.operand_u16()),
        25 => Op::EndLoop(record.operand_u16()),
        26 => Op::Break(record.operand_u16()),
        27 => Op::BreakIf(record.operand_u16()),
        28 => Op::Stream,
        29 => Op::Reset,
        30 => {
            let (c, n) = record.operand_u16_u8();
            Op::Call(c, n)
        }
        31 => Op::Return,
        32 => Op::Yield,
        33 => Op::Dup,
        34 => Op::NewStruct(record.operand_u16()),
        35 => {
            let (a, b, c) = decode_pool_u16_u16_u8(record, pool)?;
            Op::NewEnum(a, b, c)
        }
        36 => Op::NewArray(record.operand_u16()),
        37 => Op::NewTuple(record.operand_u8()),
        38 => {
            let bytes = record.operand_bytes();
            if bytes[2] == TUPLE_FIELD_BOXED_SENTINEL {
                let name_const = u16::from_le_bytes([bytes[0], bytes[1]]);
                Op::GetField(crate::bytecode::StructField::Boxed { name_const })
            } else {
                let offset = u16::from_le_bytes([bytes[0], bytes[1]]);
                let kind = crate::value_layout::ScalarKind::from_tag(bytes[2])
                    .ok_or(WireFormatError::TupleFieldKindUnknown(bytes[2]))?;
                Op::GetField(crate::bytecode::StructField::Flat { offset, kind })
            }
        }
        39 => {
            let b0 = record.operand_bytes()[0];
            if b0 == TUPLE_FIELD_BOXED_SENTINEL {
                Op::GetIndex(crate::bytecode::ArrayElem::Boxed)
            } else {
                let kind = crate::value_layout::ScalarKind::from_tag(b0)
                    .ok_or(WireFormatError::TupleFieldKindUnknown(b0))?;
                Op::GetIndex(crate::bytecode::ArrayElem::Flat { kind })
            }
        }
        40 => {
            let bytes = record.operand_bytes();
            if bytes[2] == TUPLE_FIELD_BOXED_SENTINEL {
                Op::GetTupleField(crate::bytecode::TupleField::Boxed { index: bytes[0] })
            } else {
                let offset = u16::from_le_bytes([bytes[0], bytes[1]]);
                let kind = crate::value_layout::ScalarKind::from_tag(bytes[2])
                    .ok_or(WireFormatError::TupleFieldKindUnknown(bytes[2]))?;
                Op::GetTupleField(crate::bytecode::TupleField::Flat { offset, kind })
            }
        }
        41 => {
            let bytes = record.operand_bytes();
            if bytes[2] == TUPLE_FIELD_BOXED_SENTINEL {
                Op::GetEnumField(crate::bytecode::EnumField::Boxed { index: bytes[0] })
            } else {
                let offset = u16::from_le_bytes([bytes[0], bytes[1]]);
                let kind = crate::value_layout::ScalarKind::from_tag(bytes[2])
                    .ok_or(WireFormatError::TupleFieldKindUnknown(bytes[2]))?;
                Op::GetEnumField(crate::bytecode::EnumField::Flat { offset, kind })
            }
        }
        42 => Op::Len,
        43 => {
            let (a, b) = decode_pool_u16_u16(record, pool)?;
            Op::IsEnum(a, b)
        }
        44 => Op::IsStruct(record.operand_u16()),
        45 => Op::IntToFloat,
        46 => Op::FloatToInt,
        47 => Op::WordToByte,
        48 => Op::ByteToWord,
        49 => Op::WordToFixed(record.operand_u8()),
        50 => Op::FixedToWord(record.operand_u8()),
        51 => Op::FixedMul(record.operand_u8()),
        52 => Op::FixedDiv(record.operand_u8()),
        53 => Op::Trap(record.operand_u16()),
        54 => Op::CheckedAdd,
        55 => Op::CheckedSub,
        56 => Op::CheckedMul(record.operand_u8()),
        57 => Op::CheckedNeg,
        58 => Op::CheckedDiv(record.operand_u8()),
        59 => Op::CheckedMod,
        60 => Op::PushImmediate(record.operand_u8()),
        61 => Op::PopN(record.operand_u8()),
        62 => Op::BitAnd,
        63 => Op::BitOr,
        64 => Op::BitXor,
        65 => Op::Shl,
        66 => Op::Shr,
        67 => {
            let (c, n) = record.operand_u16_u8();
            Op::CallVerifiedNative(c, n)
        }
        68 => {
            let (c, n) = record.operand_u16_u8();
            Op::CallExternalNative(c, n)
        }
        other => return Err(WireFormatError::UnknownOpcodeId(other)),
    };
    let _ = opcode_name_for_id(id);
    Ok(op)
}

/// Helper: fetch a `(u16, u16)` operand pool entry, validating
/// the index, the parity, and the type tag.
fn decode_pool_u16_u16(
    record: OpcodeRecord,
    pool: &[OperandPoolEntry],
) -> Result<(u16, u16), WireFormatError> {
    let idx = record.operand_pool_index() as usize;
    let entry = pool
        .get(idx)
        .copied()
        .ok_or(WireFormatError::OperandPoolIndexOutOfBounds(idx))?;
    entry.check_parity()?;
    if entry.tag() != POOL_TAG_U16_U16 {
        return Err(WireFormatError::OperandPoolTagMismatch {
            observed: entry.tag(),
            expected: POOL_TAG_U16_U16,
        });
    }
    Ok(entry.as_u16_u16())
}

/// Helper: fetch a `(u16, u16, u8)` operand pool entry,
/// validating the index, the parity, and the type tag.
fn decode_pool_u16_u16_u8(
    record: OpcodeRecord,
    pool: &[OperandPoolEntry],
) -> Result<(u16, u16, u8), WireFormatError> {
    let idx = record.operand_pool_index() as usize;
    let entry = pool
        .get(idx)
        .copied()
        .ok_or(WireFormatError::OperandPoolIndexOutOfBounds(idx))?;
    entry.check_parity()?;
    if entry.tag() != POOL_TAG_U16_U16_U8 {
        return Err(WireFormatError::OperandPoolTagMismatch {
            observed: entry.tag(),
            expected: POOL_TAG_U16_U16_U8,
        });
    }
    Ok(entry.as_u16_u16_u8())
}

/// Footer length: the trailing CRC-32 over the entire framed
/// bytecode.
pub const WIRE_FORMAT_FOOTER_BYTES: usize = 4;

/// Information about a module's signature, extracted from the
/// framing header's signature-extension block. Used both by the
/// decoder (to validate framing) and by the signature-verification
/// path (to locate the signature bytes and construct the message
/// view).
#[derive(Debug, Clone, Copy)]
pub struct SignatureInfo {
    /// Scheme identifier at byte 64 of the header. The only V0.2.0
    /// scheme is `SIGNATURE_SCHEME_ED25519 = 1`.
    pub scheme_id: u8,
    /// Byte offset within the framed buffer where the raw
    /// signature bytes begin. Always `WIRE_FORMAT_HEADER_BYTES +
    /// SIGNATURE_METADATA_BYTES = 72` for the V0.2.0 layout.
    pub signature_offset: usize,
    /// Length of the signature in bytes. For Ed25519: 64.
    pub signature_length: usize,
}

/// Compute the framing header length for a signed module given
/// the chosen signature scheme. Pads to an 8-byte boundary so the
/// body that follows starts aligned.
pub const fn signed_header_length(signature_length: usize) -> usize {
    let unpadded = WIRE_FORMAT_HEADER_BYTES + SIGNATURE_METADATA_BYTES + signature_length;
    // Round up to multiple of 8 for body alignment.
    (unpadded + 7) & !7
}

/// Parse the optional signature-extension metadata block at the
/// tail of the framing header. Returns `Ok(None)` for an unsigned
/// module (header_length exactly `WIRE_FORMAT_HEADER_BYTES`).
/// Returns `Ok(Some(info))` for a well-formed signed module.
/// Returns `Err` if the extension is malformed, claims an unknown
/// scheme, or carries an unexpected signature length.
///
/// `bytes` must point at the start of the framed buffer (after
/// any shebang strip). `header_length` must already be validated
/// to fall within the buffer.
pub fn parse_signature_metadata(
    bytes: &[u8],
    header_length: usize,
) -> Result<Option<SignatureInfo>, LoadError> {
    if header_length == WIRE_FORMAT_HEADER_BYTES {
        return Ok(None);
    }
    if header_length < WIRE_FORMAT_HEADER_BYTES + SIGNATURE_METADATA_BYTES {
        return Err(LoadError::Codec(format!(
            "header_length {} is less than the minimum {} required for a signature extension",
            header_length,
            WIRE_FORMAT_HEADER_BYTES + SIGNATURE_METADATA_BYTES,
        )));
    }
    let scheme_id = bytes[64];
    let reserved_byte = bytes[65];
    let signature_length = u16::from_le_bytes([bytes[66], bytes[67]]) as usize;
    let reserved_word = u32::from_le_bytes([bytes[68], bytes[69], bytes[70], bytes[71]]);
    if reserved_byte != 0 || reserved_word != 0 {
        return Err(LoadError::Codec(String::from(
            "signature metadata reserved fields must be zero",
        )));
    }
    if scheme_id == 0 {
        return Err(LoadError::Codec(String::from(
            "signature metadata scheme_id 0 is reserved; signed modules must use scheme_id >= 1",
        )));
    }
    if scheme_id != SIGNATURE_SCHEME_ED25519 {
        return Err(LoadError::Codec(format!(
            "signature scheme_id {} is not supported in this build (only Ed25519 = {} is implemented in V0.2.0)",
            scheme_id, SIGNATURE_SCHEME_ED25519,
        )));
    }
    if signature_length != ED25519_SIGNATURE_BYTES {
        return Err(LoadError::Codec(format!(
            "Ed25519 signature_length must be {}; got {}",
            ED25519_SIGNATURE_BYTES, signature_length,
        )));
    }
    let expected_header_length = signed_header_length(signature_length);
    let expected_encrypted_header_length = expected_header_length + ENCRYPTION_METADATA_BYTES;
    let encrypted = bytes[15] & FLAG_ENCRYPTED != 0;
    let acceptable = if encrypted {
        header_length == expected_encrypted_header_length
    } else {
        header_length == expected_header_length
    };
    if !acceptable {
        let expected = if encrypted {
            expected_encrypted_header_length
        } else {
            expected_header_length
        };
        return Err(LoadError::Codec(format!(
            "header_length {} does not match expected {} (sig metadata {} + sig {}{} + padding to 8-byte multiple)",
            header_length,
            expected,
            SIGNATURE_METADATA_BYTES,
            signature_length,
            if encrypted {
                " + encryption metadata"
            } else {
                ""
            },
        )));
    }
    Ok(Some(SignatureInfo {
        scheme_id,
        signature_offset: WIRE_FORMAT_HEADER_BYTES + SIGNATURE_METADATA_BYTES,
        signature_length,
    }))
}

/// Reflected polynomial residue produced after concatenating any
/// byte sequence with the little-endian encoding of its CRC-32.
/// The wire-format reader verifies the bytecode through this
/// residue equality.
const WIRE_FORMAT_CRC32_RESIDUE: u32 = 0x2144DF1C;

/// Wire-format chunk metadata. Mirrors [`Chunk`] but moves the
/// `ops` vector out of the rkyv-archived body and into the
/// opcode stream section. Carries the byte offset and record
/// count needed to recover the chunk's opcode sequence from the
/// section-partitioned body.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct WireChunk {
    /// Function name; matches `Chunk::name`.
    pub name: String,
    /// Constant pool; matches `Chunk::constants`.
    pub constants: Vec<ConstValue>,
    /// Struct templates; matches `Chunk::struct_templates`.
    pub struct_templates: Vec<StructTemplate>,
    /// Total local variable slots; matches `Chunk::local_count`.
    pub local_count: u16,
    /// Number of parameters; matches `Chunk::param_count`.
    pub param_count: u8,
    /// Block type classification; matches `Chunk::block_type`.
    pub block_type: BlockType,
    /// Parameter type tags; matches `Chunk::param_types`.
    pub param_types: Vec<TypeTag>,
    /// Byte offset into the opcode stream section where this
    /// chunk's opcode records start. Always a multiple of
    /// [`OPCODE_RECORD_BYTES`].
    pub op_byte_offset: u32,
    /// Number of opcode records that compose this chunk's body.
    /// The chunk's total byte span in the opcode stream is
    /// `op_record_count * OPCODE_RECORD_BYTES`.
    pub op_record_count: u32,
    /// Optional strippable debug metadata (B29), carried as the
    /// canonical bytes produced by
    /// [`crate::debug_meta::DebugPool::encode`]. `None` for a release
    /// build or a stripped artefact. Held as opaque bytes so the
    /// section is parseable by external tooling without rkyv and so
    /// dropping it (strip) is a single `None` assignment. The debug
    /// metadata lives only here in the auxiliary body, never in the
    /// opcode stream, which keeps the opcode stream byte-identical
    /// between debug and release builds.
    pub debug_pool_bytes: Option<Vec<u8>>,
}

/// Wire-format auxiliary body. Mirrors [`Module`] but carries
/// [`WireChunk`] metadata (ops live in the opcode stream
/// section).
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct WireAuxBody {
    /// Chunk metadata for the module's chunks.
    pub chunks: Vec<WireChunk>,
    /// Native function names referenced by the module.
    pub native_names: Vec<String>,
    /// Entry-point chunk index, if the module declares one.
    pub entry_point: Option<usize>,
    /// Data-segment layout, if the module declares any data fields.
    pub data_layout: Option<DataLayout>,
    /// Runtime word width declared by the module, encoded as the
    /// base-2 logarithm of the bit width.
    pub word_bits_log2: u8,
    /// Runtime address width declared by the module, log2 form.
    pub addr_bits_log2: u8,
    /// Runtime float width declared by the module, log2 form.
    pub float_bits_log2: u8,
    /// Declared WCET in pipelined cycles per Stream-to-Reset slice.
    /// `0` means auto; `u32::MAX` means overflow.
    pub wcet_cycles: u32,
    /// Declared WCMU in bytes per Stream-to-Reset slice. `0` means
    /// auto; `u32::MAX` means overflow.
    pub wcmu_bytes: u32,
    /// Header flag byte (e.g. `FLAG_EPHEMERAL`, `FLAG_REQUIRES_SIGNATURE`).
    pub flags: u8,
    /// Verifier-populated byte count for the shared partition of
    /// the data segment.
    pub shared_data_bytes: u32,
    /// Verifier-populated byte count for the private partition of
    /// the data segment.
    pub private_data_bytes: u32,
    /// CRC-32 of the canonical serialisation of
    /// `(slot_name, visibility)` per slot in declaration order.
    /// Used by `Vm::replace_module` to reject schema-incompatible
    /// hot swaps.
    pub schema_hash: u32,
}

/// Strip a `#!` shebang prefix from a byte slice. Wire-format
/// bytes that begin with `#!` are produced when a host appends
/// the bytecode after an executable shebang; the strip returns
/// the slice past the first `\n`. Bytes without the prefix pass
/// through unchanged.
fn strip_shebang_prefix(bytes: &[u8]) -> &[u8] {
    if bytes.starts_with(b"#!") {
        if let Some(newline_pos) = bytes.iter().position(|&b| b == b'\n') {
            &bytes[newline_pos + 1..]
        } else {
            bytes
        }
    } else {
        bytes
    }
}

/// Borrowed view of the three body sections inside a framed
/// V0.2.0 wire-format buffer. Callers obtain this through
/// [`parse_wire_sections`] after framing-level validation has
/// run.
#[derive(Debug, Clone, Copy)]
pub struct WireSections<'a> {
    /// Bytes of the opcode stream section. Length is a multiple
    /// of [`OPCODE_RECORD_BYTES`].
    pub opcode_stream: &'a [u8],
    /// Bytes of the operand pool section. Length is a multiple
    /// of [`OPERAND_POOL_ENTRY_BYTES`].
    pub operand_pool: &'a [u8],
    /// Bytes of the rkyv-archived auxiliary body.
    pub aux_body: &'a [u8],
}

/// Header-mirrored fields exposed by [`read_header_fields`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderFields {
    /// Declared word width, encoded as the base-2 logarithm of the
    /// bit width.
    pub word_bits_log2: u8,
    /// Declared address width, log2 form.
    pub addr_bits_log2: u8,
    /// Declared float width, log2 form.
    pub float_bits_log2: u8,
    /// Header flag byte.
    pub flags: u8,
    /// Declared WCET in pipelined cycles. `0` means auto;
    /// `u32::MAX` means overflow.
    pub wcet_cycles: u32,
    /// Declared WCMU in bytes. Same convention as `wcet_cycles`.
    pub wcmu_bytes: u32,
    /// Verifier-populated byte count for the shared data partition.
    pub shared_data_bytes: u32,
    /// Verifier-populated byte count for the private data partition.
    pub private_data_bytes: u32,
}

/// Parse a framed V0.2.0 wire-format buffer and return slices
/// for the three body sections. Validates the framing header
/// (magic, version, header length, total length), the CRC-32
/// residue, and the section bounds. Does not deserialize the
/// auxiliary body; callers feed `aux_body` to
/// `rkyv::access::<ArchivedWireAuxBody, _>` or to
/// `rkyv::from_bytes::<WireAuxBody, _>`.
///
/// The returned slices borrow from the input. Used by both
/// [`module_from_wire_bytes`] and the VM's zero-copy view path.
pub fn parse_wire_sections(bytes: &[u8]) -> Result<WireSections<'_>, LoadError> {
    let bytes = strip_shebang_prefix(bytes);
    if bytes.len() < WIRE_FORMAT_HEADER_BYTES + WIRE_FORMAT_FOOTER_BYTES {
        return Err(LoadError::Truncated);
    }
    if bytes[0..4] != BYTECODE_MAGIC {
        return Err(LoadError::BadMagic);
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != BYTECODE_VERSION {
        return Err(LoadError::UnsupportedVersion {
            got: version,
            expected: BYTECODE_VERSION,
        });
    }
    let header_length = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
    if header_length < WIRE_FORMAT_HEADER_BYTES {
        return Err(LoadError::Codec(format!(
            "wire format header_length {} is below the minimum {}",
            header_length, WIRE_FORMAT_HEADER_BYTES
        )));
    }
    if header_length > bytes.len() {
        return Err(LoadError::Truncated);
    }
    let total_length = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    if total_length < header_length + WIRE_FORMAT_FOOTER_BYTES || total_length > bytes.len() {
        return Err(LoadError::Truncated);
    }
    let bytes = &bytes[..total_length];
    if crc32(bytes) != WIRE_FORMAT_CRC32_RESIDUE {
        return Err(LoadError::BadChecksum);
    }
    // Signature-extension consistency. If the flag is set, the
    // header must carry a signature metadata block; if the flag
    // is unset, the header must be exactly the base size.
    let flags = bytes[15];
    let signed = (flags & FLAG_REQUIRES_SIGNATURE) != 0;
    let sig_info = parse_signature_metadata(bytes, header_length)?;
    match (signed, sig_info) {
        (true, None) => {
            return Err(LoadError::Codec(String::from(
                "FLAG_REQUIRES_SIGNATURE is set but the header carries no signature extension",
            )));
        }
        (false, Some(_)) => {
            return Err(LoadError::Codec(String::from(
                "header carries a signature extension but FLAG_REQUIRES_SIGNATURE is not set; V0.2.0 does not admit audit-only signatures",
            )));
        }
        _ => {}
    }
    let opcode_stream_offset =
        u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]) as usize;
    let opcode_stream_length =
        u32::from_le_bytes([bytes[36], bytes[37], bytes[38], bytes[39]]) as usize;
    let operand_pool_offset =
        u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]) as usize;
    let operand_pool_length =
        u32::from_le_bytes([bytes[44], bytes[45], bytes[46], bytes[47]]) as usize;
    let aux_body_offset = u32::from_le_bytes([bytes[48], bytes[49], bytes[50], bytes[51]]) as usize;
    let aux_body_length = u32::from_le_bytes([bytes[52], bytes[53], bytes[54], bytes[55]]) as usize;
    let body_end = total_length - WIRE_FORMAT_FOOTER_BYTES;
    let in_body = |off: usize, len: usize| -> bool {
        off >= header_length && off.checked_add(len).is_some_and(|end| end <= body_end)
    };
    if !in_body(opcode_stream_offset, opcode_stream_length)
        || !in_body(operand_pool_offset, operand_pool_length)
        || !in_body(aux_body_offset, aux_body_length)
    {
        return Err(LoadError::Truncated);
    }
    if !opcode_stream_length.is_multiple_of(OPCODE_RECORD_BYTES) {
        return Err(LoadError::Codec(format!(
            "opcode stream length {} is not a multiple of the record size {}",
            opcode_stream_length, OPCODE_RECORD_BYTES,
        )));
    }
    if !operand_pool_length.is_multiple_of(OPERAND_POOL_ENTRY_BYTES) {
        return Err(LoadError::Codec(format!(
            "operand pool length {} is not a multiple of the entry size {}",
            operand_pool_length, OPERAND_POOL_ENTRY_BYTES,
        )));
    }
    Ok(WireSections {
        opcode_stream: &bytes[opcode_stream_offset..opcode_stream_offset + opcode_stream_length],
        operand_pool: &bytes[operand_pool_offset..operand_pool_offset + operand_pool_length],
        aux_body: &bytes[aux_body_offset..aux_body_offset + aux_body_length],
    })
}

/// Read header-mirrored target widths and declared WCET / WCMU
/// fields from a framed V0.2.0 wire-format buffer. Used by
/// [`crate::bytecode::Module::access_bytes`] (and the VM's
/// zero-copy path) to surface the fast-path metadata without
/// deserializing the auxiliary body.
pub fn read_header_fields(bytes: &[u8]) -> Result<HeaderFields, LoadError> {
    // The parse_wire_sections call validates framing and CRC;
    // reuse it so the header read participates in the same
    // validation pass.
    let _ = parse_wire_sections(bytes)?;
    let bytes = strip_shebang_prefix(bytes);
    Ok(HeaderFields {
        word_bits_log2: bytes[12],
        addr_bits_log2: bytes[13],
        float_bits_log2: bytes[14],
        flags: bytes[15],
        wcet_cycles: u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
        wcmu_bytes: u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
        shared_data_bytes: u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
        private_data_bytes: u32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]),
    })
}

/// Decode every opcode record in `opcode_stream` against
/// `operand_pool` and return the resulting `Vec<Op>`. Used by
/// the VM at construction time to populate `decoded_ops` from
/// the wire-format sections.
pub fn decode_op_stream(
    opcode_stream: &[u8],
    operand_pool_bytes: &[u8],
) -> Result<Vec<Op>, LoadError> {
    if !opcode_stream.len().is_multiple_of(OPCODE_RECORD_BYTES) {
        return Err(LoadError::Codec(format!(
            "opcode stream length {} is not a multiple of {}",
            opcode_stream.len(),
            OPCODE_RECORD_BYTES,
        )));
    }
    if !operand_pool_bytes
        .len()
        .is_multiple_of(OPERAND_POOL_ENTRY_BYTES)
    {
        return Err(LoadError::Codec(format!(
            "operand pool length {} is not a multiple of {}",
            operand_pool_bytes.len(),
            OPERAND_POOL_ENTRY_BYTES,
        )));
    }
    let mut pool: Vec<OperandPoolEntry> =
        Vec::with_capacity(operand_pool_bytes.len() / OPERAND_POOL_ENTRY_BYTES);
    for chunk_off in (0..operand_pool_bytes.len()).step_by(OPERAND_POOL_ENTRY_BYTES) {
        let mut entry = [0u8; OPERAND_POOL_ENTRY_BYTES];
        entry.copy_from_slice(&operand_pool_bytes[chunk_off..chunk_off + OPERAND_POOL_ENTRY_BYTES]);
        let entry = OperandPoolEntry(entry);
        entry
            .check_parity()
            .map_err(|e| LoadError::Codec(format!("operand pool entry corruption: {:?}", e)))?;
        pool.push(entry);
    }
    let mut ops: Vec<Op> = Vec::with_capacity(opcode_stream.len() / OPCODE_RECORD_BYTES);
    for off in (0..opcode_stream.len()).step_by(OPCODE_RECORD_BYTES) {
        let mut rec = [0u8; OPCODE_RECORD_BYTES];
        rec.copy_from_slice(&opcode_stream[off..off + OPCODE_RECORD_BYTES]);
        let op = decode_op(OpcodeRecord(rec), &pool)
            .map_err(|e| LoadError::Codec(format!("opcode decode failed: {:?}", e)))?;
        ops.push(op);
    }
    Ok(ops)
}

/// Encode a [`Module`] into the V0.2.0 wire format: 64-byte
/// framing header, opcode stream, operand pool, rkyv-archived
/// auxiliary body, 4-byte CRC-32 trailer.
///
/// The opcode stream packs every chunk's opcodes as 4-byte
/// records in chunk declaration order. The auxiliary body's
/// [`WireChunk`] entries point into the opcode stream through
/// `op_byte_offset` and `op_record_count`. Compound operands
/// (`(u16, u16)` for `Op::GetDataIndexed` / `Op::SetDataIndexed`
/// / `Op::IsEnum` and `(u16, u16, u8)` for `Op::NewEnum`) flow
/// into the operand pool as 8-byte entries indexed by the
/// inline 24-bit pool index in the opcode record.
pub fn module_to_wire_bytes(module: &Module) -> Result<Vec<u8>, LoadError> {
    // Build the opcode stream and operand pool. Track per-chunk
    // byte offsets and record counts for the WireChunk metadata.
    let mut opcode_stream: Vec<u8> = Vec::new();
    let mut operand_pool: Vec<OperandPoolEntry> = Vec::new();
    let mut wire_chunks: Vec<WireChunk> = Vec::with_capacity(module.chunks.len());
    for chunk in &module.chunks {
        let op_byte_offset = opcode_stream.len() as u32;
        let op_record_count = chunk.ops.len() as u32;
        for op in &chunk.ops {
            let record = encode_op(op, &mut operand_pool)
                .map_err(|e| LoadError::Codec(format!("opcode encode failed: {:?}", e)))?;
            opcode_stream.extend_from_slice(&record.0);
        }
        wire_chunks.push(WireChunk {
            name: chunk.name.clone(),
            constants: chunk.constants.clone(),
            struct_templates: chunk.struct_templates.clone(),
            local_count: chunk.local_count,
            param_count: chunk.param_count,
            block_type: chunk.block_type,
            param_types: chunk.param_types.clone(),
            op_byte_offset,
            op_record_count,
            debug_pool_bytes: chunk.debug_pool.as_ref().map(|p| p.encode()),
        });
    }
    let aux = WireAuxBody {
        chunks: wire_chunks,
        native_names: module.native_names.clone(),
        entry_point: module.entry_point,
        data_layout: module.data_layout.clone(),
        word_bits_log2: module.word_bits_log2,
        addr_bits_log2: module.addr_bits_log2,
        float_bits_log2: module.float_bits_log2,
        wcet_cycles: module.wcet_cycles,
        wcmu_bytes: module.wcmu_bytes,
        flags: module.flags,
        shared_data_bytes: module.shared_data_bytes,
        private_data_bytes: module.private_data_bytes,
        schema_hash: module.schema_hash,
    };
    let aux_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&aux)
        .map_err(|e| LoadError::Codec(format!("aux body encode failed: {}", e)))?;

    // Flatten operand pool to bytes.
    let mut operand_pool_bytes: Vec<u8> =
        Vec::with_capacity(operand_pool.len() * OPERAND_POOL_ENTRY_BYTES);
    for entry in &operand_pool {
        operand_pool_bytes.extend_from_slice(&entry.0);
    }

    assemble_wire_bytes(
        module,
        &opcode_stream,
        &operand_pool_bytes,
        &aux_bytes,
        None,
    )
}

/// Shared buffer assembly for both signed and unsigned encoder
/// paths. The `signature` parameter encodes whether the output
/// should include a signature-extension block:
///
/// - `None`: produces an unsigned buffer with a 64-byte framing
///   header. The buffer is fully framed including the CRC trailer.
/// - `Some(scheme_id)`: produces a signed buffer with an extended
///   header that holds the signature metadata and a *zeroed*
///   signature payload. The CRC trailer is also zeroed. The
///   caller (`module_to_signed_wire_bytes`) is responsible for
///   computing the signature over the zeroed buffer, patching the
///   real signature bytes into the header, and finally computing
///   and patching the real CRC.
fn assemble_wire_bytes(
    module: &Module,
    opcode_stream: &[u8],
    operand_pool_bytes: &[u8],
    aux_bytes: &[u8],
    signature: Option<u8>,
) -> Result<Vec<u8>, LoadError> {
    // Determine header_length and the flag state of the module.
    let (header_length, effective_flags) = match signature {
        None => (WIRE_FORMAT_HEADER_BYTES, module.flags),
        Some(SIGNATURE_SCHEME_ED25519) => (
            signed_header_length(ED25519_SIGNATURE_BYTES),
            module.flags | FLAG_REQUIRES_SIGNATURE,
        ),
        Some(other) => {
            return Err(LoadError::Codec(format!(
                "unsupported signature scheme_id {} (only Ed25519 = {} ships in V0.2.0)",
                other, SIGNATURE_SCHEME_ED25519
            )));
        }
    };
    // Compute section offsets. The opcode stream begins
    // immediately after the framing header. Each section is then
    // padded to an 8-byte boundary so the next section starts
    // aligned. The aux body is rkyv-archived and requires 8-byte
    // alignment for in-place access.
    let opcode_stream_offset = header_length as u32;
    let opcode_stream_length = opcode_stream.len() as u32;
    let mut after_opcodes = opcode_stream_offset + opcode_stream_length;
    let opcode_padding = ((8 - (after_opcodes as usize % 8)) % 8) as u32;
    after_opcodes += opcode_padding;
    let operand_pool_offset = after_opcodes;
    let operand_pool_length = operand_pool_bytes.len() as u32;
    let mut after_pool = operand_pool_offset + operand_pool_length;
    let pool_padding = ((8 - (after_pool as usize % 8)) % 8) as u32;
    after_pool += pool_padding;
    let aux_body_offset = after_pool;
    let aux_body_length = aux_bytes.len() as u32;
    let after_aux = aux_body_offset + aux_body_length;
    let total_length = after_aux + WIRE_FORMAT_FOOTER_BYTES as u32;

    // Assemble the buffer.
    let mut buf: Vec<u8> = Vec::with_capacity(total_length as usize);
    // Framing header (base 64 bytes).
    buf.extend_from_slice(&BYTECODE_MAGIC);
    buf.extend_from_slice(&BYTECODE_VERSION.to_le_bytes());
    buf.extend_from_slice(&(header_length as u16).to_le_bytes());
    buf.extend_from_slice(&total_length.to_le_bytes());
    buf.push(module.word_bits_log2);
    buf.push(module.addr_bits_log2);
    buf.push(module.float_bits_log2);
    buf.push(effective_flags);
    buf.extend_from_slice(&module.wcet_cycles.to_le_bytes());
    buf.extend_from_slice(&module.wcmu_bytes.to_le_bytes());
    buf.extend_from_slice(&module.shared_data_bytes.to_le_bytes());
    buf.extend_from_slice(&module.private_data_bytes.to_le_bytes());
    buf.extend_from_slice(&opcode_stream_offset.to_le_bytes());
    buf.extend_from_slice(&opcode_stream_length.to_le_bytes());
    buf.extend_from_slice(&operand_pool_offset.to_le_bytes());
    buf.extend_from_slice(&operand_pool_length.to_le_bytes());
    buf.extend_from_slice(&aux_body_offset.to_le_bytes());
    buf.extend_from_slice(&aux_body_length.to_le_bytes());
    buf.extend_from_slice(&[0u8; 4]); // reserved at offsets 56..60
    buf.extend_from_slice(&[0u8; 4]); // reserved at offsets 60..64
    debug_assert_eq!(buf.len(), WIRE_FORMAT_HEADER_BYTES);

    // Signature extension. The signature payload is zero-filled
    // here; the caller patches the real signature in after signing
    // the zero-filled buffer.
    if let Some(scheme_id) = signature {
        // 8-byte signature metadata block.
        buf.push(scheme_id);
        buf.push(0); // reserved
        let signature_length = match scheme_id {
            SIGNATURE_SCHEME_ED25519 => ED25519_SIGNATURE_BYTES,
            _ => unreachable!("validated above"),
        };
        buf.extend_from_slice(&(signature_length as u16).to_le_bytes());
        buf.extend_from_slice(&[0u8; 4]); // reserved
        // Signature payload (zero placeholder).
        buf.resize(buf.len() + signature_length, 0);
        // Pad to 8-byte boundary so the body starts aligned.
        while buf.len() < header_length {
            buf.push(0);
        }
        debug_assert_eq!(buf.len(), header_length);
    }

    // Opcode stream + alignment padding.
    buf.extend_from_slice(opcode_stream);
    buf.resize(buf.len() + opcode_padding as usize, 0);

    // Operand pool + alignment padding.
    buf.extend_from_slice(operand_pool_bytes);
    buf.resize(buf.len() + pool_padding as usize, 0);

    // Auxiliary body.
    buf.extend_from_slice(aux_bytes);

    // CRC trailer. Compute over the buffer so far (signed and
    // unsigned paths both produce a valid CRC at this point).
    let crc = crc32(&buf);
    buf.extend_from_slice(&crc.to_le_bytes());
    debug_assert_eq!(buf.len(), total_length as usize);
    Ok(buf)
}

/// Encode a [`Module`] into the V0.2.0 wire format with an
/// Ed25519 signature appended to the framing header. Sets
/// `FLAG_REQUIRES_SIGNATURE` in the header's flags byte.
///
/// The signature is computed over the entire framed buffer with
/// the signature payload bytes and the CRC trailer bytes zeroed.
/// The verifier reconstructs the same view by copying the
/// received buffer and zeroing those two regions before calling
/// [`verify_module_signature`].
///
/// Requires the `signatures` cargo feature. Without it, the
/// `signed` surface keyword still parses (for source-file
/// portability) but no host can produce signed bytecode.
#[cfg(feature = "signatures")]
pub fn module_to_signed_wire_bytes(
    module: &Module,
    signing_key: &ed25519_dalek::SigningKey,
) -> Result<Vec<u8>, LoadError> {
    use ed25519_dalek::Signer;

    // Re-run the per-chunk encoding so we have the opcode stream,
    // operand pool, and aux body bytes to hand to the shared
    // assembler. This is a small duplication of `module_to_wire_bytes`
    // but keeps the signed and unsigned encode paths uniform.
    let mut opcode_stream: Vec<u8> = Vec::new();
    let mut operand_pool: Vec<OperandPoolEntry> = Vec::new();
    let mut wire_chunks: Vec<WireChunk> = Vec::with_capacity(module.chunks.len());
    for chunk in &module.chunks {
        let op_byte_offset = opcode_stream.len() as u32;
        let op_record_count = chunk.ops.len() as u32;
        for op in &chunk.ops {
            let record = encode_op(op, &mut operand_pool)
                .map_err(|e| LoadError::Codec(format!("opcode encode failed: {:?}", e)))?;
            opcode_stream.extend_from_slice(&record.0);
        }
        wire_chunks.push(WireChunk {
            name: chunk.name.clone(),
            constants: chunk.constants.clone(),
            struct_templates: chunk.struct_templates.clone(),
            local_count: chunk.local_count,
            param_count: chunk.param_count,
            block_type: chunk.block_type,
            param_types: chunk.param_types.clone(),
            op_byte_offset,
            op_record_count,
            debug_pool_bytes: chunk.debug_pool.as_ref().map(|p| p.encode()),
        });
    }
    let aux = WireAuxBody {
        chunks: wire_chunks,
        native_names: module.native_names.clone(),
        entry_point: module.entry_point,
        data_layout: module.data_layout.clone(),
        word_bits_log2: module.word_bits_log2,
        addr_bits_log2: module.addr_bits_log2,
        float_bits_log2: module.float_bits_log2,
        wcet_cycles: module.wcet_cycles,
        wcmu_bytes: module.wcmu_bytes,
        flags: module.flags | FLAG_REQUIRES_SIGNATURE,
        shared_data_bytes: module.shared_data_bytes,
        private_data_bytes: module.private_data_bytes,
        schema_hash: module.schema_hash,
    };
    let aux_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&aux)
        .map_err(|e| LoadError::Codec(format!("aux body encode failed: {}", e)))?;
    let mut operand_pool_bytes: Vec<u8> =
        Vec::with_capacity(operand_pool.len() * OPERAND_POOL_ENTRY_BYTES);
    for entry in &operand_pool {
        operand_pool_bytes.extend_from_slice(&entry.0);
    }

    // Assemble with a zeroed signature payload.
    let mut buf = assemble_wire_bytes(
        module,
        &opcode_stream,
        &operand_pool_bytes,
        &aux_bytes,
        Some(SIGNATURE_SCHEME_ED25519),
    )?;
    let total_length = buf.len();

    // Zero the CRC trailer for the signing pass.
    let footer_start = total_length - WIRE_FORMAT_FOOTER_BYTES;
    for byte in &mut buf[footer_start..] {
        *byte = 0;
    }

    // Sign over (header + zeroed signature + body + zeroed CRC).
    let signature = signing_key.sign(&buf);
    let sig_bytes = signature.to_bytes();

    // Patch the real signature into the header.
    let sig_offset = WIRE_FORMAT_HEADER_BYTES + SIGNATURE_METADATA_BYTES;
    buf[sig_offset..sig_offset + ED25519_SIGNATURE_BYTES].copy_from_slice(&sig_bytes);

    // Recompute and write the real CRC trailer.
    let crc = crc32(&buf[..footer_start]);
    buf[footer_start..].copy_from_slice(&crc.to_le_bytes());

    Ok(buf)
}

/// Verify that the bytecode at `bytes` carries a signature
/// matching at least one of the supplied verifying keys. Returns
/// `Ok(())` on a successful match; `Err(LoadError::InvalidSignature)`
/// if no key matches; other `LoadError` variants if the framing
/// or signature metadata is malformed.
///
/// The verification message is the bytecode buffer with the
/// signature payload bytes and the CRC trailer bytes zeroed. This
/// matches the signing convention in
/// [`module_to_signed_wire_bytes`].
///
/// Requires the `signatures` cargo feature.
#[cfg(feature = "signatures")]
pub fn verify_module_signature(
    bytes: &[u8],
    keys: &[ed25519_dalek::VerifyingKey],
) -> Result<(), LoadError> {
    use ed25519_dalek::Verifier;

    let bytes = strip_shebang_prefix(bytes);
    if bytes.len() < WIRE_FORMAT_HEADER_BYTES + WIRE_FORMAT_FOOTER_BYTES {
        return Err(LoadError::Truncated);
    }
    if bytes[0..4] != BYTECODE_MAGIC {
        return Err(LoadError::BadMagic);
    }
    let header_length = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
    let total_length = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    if total_length > bytes.len() {
        return Err(LoadError::Truncated);
    }
    let bytes = &bytes[..total_length];
    let sig_info = parse_signature_metadata(bytes, header_length)?
        .ok_or_else(|| LoadError::Codec(String::from("module is not signed")))?;

    // Build the verification message: the entire framed buffer
    // with the signature payload and CRC trailer zeroed.
    let mut message: Vec<u8> = bytes.to_vec();
    let sig_start = sig_info.signature_offset;
    let sig_end = sig_start + sig_info.signature_length;
    for byte in &mut message[sig_start..sig_end] {
        *byte = 0;
    }
    let footer_start = total_length - WIRE_FORMAT_FOOTER_BYTES;
    for byte in &mut message[footer_start..] {
        *byte = 0;
    }

    // Extract the signature.
    let mut signature_bytes = [0u8; ED25519_SIGNATURE_BYTES];
    signature_bytes.copy_from_slice(&bytes[sig_start..sig_end]);
    let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes);

    // Try each key. The first match wins; an empty key set
    // produces `InvalidSignature` (no host trust matrix → reject).
    for key in keys {
        if key.verify(&message, &signature).is_ok() {
            return Ok(());
        }
    }
    Err(LoadError::InvalidSignature)
}

/// Returns `true` if the framing header carries the signature-
/// requirement flag. Used by load-time paths that may not have
/// the `signatures` feature compiled in: such builds reject the
/// module with `LoadError::SignaturesUnsupported` rather than
/// silently admitting an unverified signed module.
pub fn header_requires_signature(bytes: &[u8]) -> bool {
    let bytes = strip_shebang_prefix(bytes);
    bytes.len() > 15 && (bytes[15] & FLAG_REQUIRES_SIGNATURE) != 0
}

/// Returns `true` if the framing header carries the encryption
/// flag. Used by load-time paths that may not have the
/// `encryption` feature compiled in: such builds reject the
/// module with `LoadError::EncryptionUnsupported` rather than
/// silently admitting an encrypted module that the runtime
/// cannot decrypt.
pub fn header_requires_encryption(bytes: &[u8]) -> bool {
    let bytes = strip_shebang_prefix(bytes);
    bytes.len() > 15 && (bytes[15] & FLAG_ENCRYPTED) != 0
}

/// Compute the framing header length for a signed-and-encrypted
/// module using the Ed25519+X25519+AES-256-GCM scheme. The header
/// extends the signed header by an 88-byte encryption-metadata
/// block. The result is the offset at which the encrypted body
/// (ciphertext + AES-GCM tag) begins.
pub const fn encrypted_signed_header_length() -> usize {
    let signed_part = signed_header_length(ED25519_SIGNATURE_BYTES);
    // 88 bytes is already 8-byte aligned; no extra padding needed.
    signed_part + ENCRYPTION_METADATA_BYTES
}

/// Encode a [`Module`] into the V0.2.1 wire format with both an
/// Ed25519 signature and X25519+AES-256-GCM body encryption. Sets
/// both `FLAG_REQUIRES_SIGNATURE` and `FLAG_ENCRYPTED` in the
/// header's flags byte.
///
/// The body bytes (opcode stream + operand pool + aux body, with
/// alignment padding) are encrypted with a per-module AES-256
/// key derived from an X25519 Diffie-Hellman against the
/// destination runtime's public key. The encryption metadata
/// block (ephemeral public key, recipient_key_id, AES-GCM nonce)
/// is appended to the framing header.
///
/// The signature covers the entire on-disk framed buffer with
/// the signature payload bytes and the CRC trailer bytes zeroed.
/// This means the signature authenticates both the encryption
/// metadata and the ciphertext, so an adversary cannot strip the
/// encryption layer and substitute cleartext bytecode while
/// preserving signature validity.
///
/// Requires both the `signatures` and `encryption` cargo features.
///
/// # Parameters
///
/// - `module`: the compiled module to encrypt and sign.
/// - `signing_key`: the Ed25519 signing key (typically the head
///   office's release key). Held by the compiler.
/// - `recipient_public_key`: the destination runtime's X25519
///   public key. Encryption is against this key; only the
///   matching private key on the destination runtime can decrypt.
/// - `ephemeral_seed`: 32 bytes of cryptographic randomness for
///   the per-module ephemeral X25519 key. The caller is
///   responsible for sourcing this from the OS RNG.
#[cfg(all(feature = "signatures", feature = "encryption"))]
pub fn module_to_encrypted_signed_wire_bytes(
    module: &Module,
    signing_key: &ed25519_dalek::SigningKey,
    recipient_public_key: &[u8; crate::encryption::X25519_PUBLIC_KEY_LEN],
    ephemeral_seed: &[u8; crate::encryption::X25519_PRIVATE_KEY_LEN],
) -> Result<Vec<u8>, LoadError> {
    use crate::encryption::{AES_GCM_TAG_LEN, encrypt_to_recipient};
    use ed25519_dalek::Signer;

    // Reuse the signed-encode flow to produce the "virtual"
    // unencrypted-signed wire bytes. Then encrypt the body in
    // place and patch the header to advertise the encryption.

    // Step 1. Build the signed (but unencrypted) wire bytes.
    let signed_bytes = module_to_signed_wire_bytes(module, signing_key)?;

    // The body in the signed_bytes occupies the range
    // [signed_header_length, signed_bytes.len() - WIRE_FORMAT_FOOTER_BYTES).
    let signed_header_len = signed_header_length(ED25519_SIGNATURE_BYTES);
    let plaintext_body_start = signed_header_len;
    let plaintext_body_end = signed_bytes.len() - WIRE_FORMAT_FOOTER_BYTES;
    let plaintext_body = &signed_bytes[plaintext_body_start..plaintext_body_end];

    // Step 2. Encrypt the body. The ciphertext returned by AES-GCM
    // is the encrypted bytes plus a 16-byte authentication tag.
    let (encryption_metadata, ciphertext_with_tag) =
        encrypt_to_recipient(plaintext_body, recipient_public_key, ephemeral_seed)
            .map_err(|e| LoadError::Codec(format!("encryption failed: {}", e)))?;
    debug_assert_eq!(
        ciphertext_with_tag.len(),
        plaintext_body.len() + AES_GCM_TAG_LEN
    );

    // Step 3. Assemble the encrypted-signed buffer.
    //
    // Layout:
    // - bytes 0..64:                     base framing header
    // - bytes 64..72:                    signature metadata block
    // - bytes 72..136:                   Ed25519 signature (placeholder zero for now)
    // - bytes 136..224:                  encryption metadata block (88 bytes)
    // - bytes 224..(224+ciphertext_len): ciphertext + AES-GCM tag
    // - last 4 bytes:                    CRC trailer
    let encrypted_header_len = encrypted_signed_header_length();
    let on_disk_total = encrypted_header_len + ciphertext_with_tag.len() + WIRE_FORMAT_FOOTER_BYTES;

    let mut buf: Vec<u8> = Vec::with_capacity(on_disk_total);

    // Copy the base header from the signed buffer, then patch the
    // flags, header_length, total_length, and section offsets.
    buf.extend_from_slice(&signed_bytes[..WIRE_FORMAT_HEADER_BYTES]);

    // Patch flags to include FLAG_ENCRYPTED.
    buf[15] |= FLAG_ENCRYPTED;

    // Patch header_length to the encrypted-signed value.
    buf[6..8].copy_from_slice(&(encrypted_header_len as u16).to_le_bytes());

    // Patch total_length to the on-disk file size.
    buf[8..12].copy_from_slice(&(on_disk_total as u32).to_le_bytes());

    // Patch section offsets to point at the locations where the
    // sections will live in the reconstructed plaintext view after
    // decryption. Each section offset in the original signed
    // buffer pointed at signed_header_len + section_offset_within_body.
    // In the reconstructed plaintext buffer the same sections will
    // be at encrypted_header_len + section_offset_within_body.
    // So shift each offset by (encrypted_header_len - signed_header_len) = 88.
    let shift = encrypted_header_len - signed_header_len;
    patch_section_offsets(&mut buf, shift);

    // Copy the signature metadata block and the signature.
    buf.extend_from_slice(&signed_bytes[WIRE_FORMAT_HEADER_BYTES..signed_header_len]);

    // Append the encryption metadata block.
    buf.extend_from_slice(&encryption_metadata.to_bytes());
    debug_assert_eq!(buf.len(), encrypted_header_len);

    // Append ciphertext + tag.
    buf.extend_from_slice(&ciphertext_with_tag);

    // Append zeroed CRC trailer placeholder.
    buf.extend_from_slice(&[0u8; WIRE_FORMAT_FOOTER_BYTES]);
    debug_assert_eq!(buf.len(), on_disk_total);

    // Re-sign over the new buffer (with signature payload and CRC
    // zeroed; both already zero in the assembled buffer at this
    // point because the signature was copied from the original
    // signed buffer and then the encryption metadata + ciphertext
    // + tag were appended, changing what the signature should
    // cover. Zero out the signature payload before re-signing.)
    let sig_offset = WIRE_FORMAT_HEADER_BYTES + SIGNATURE_METADATA_BYTES;
    for byte in &mut buf[sig_offset..sig_offset + ED25519_SIGNATURE_BYTES] {
        *byte = 0;
    }
    let signature = signing_key.sign(&buf);
    let sig_bytes = signature.to_bytes();
    buf[sig_offset..sig_offset + ED25519_SIGNATURE_BYTES].copy_from_slice(&sig_bytes);

    // Compute and patch the real CRC.
    let crc_offset = on_disk_total - WIRE_FORMAT_FOOTER_BYTES;
    let crc = crc32(&buf[..crc_offset]);
    buf[crc_offset..].copy_from_slice(&crc.to_le_bytes());

    Ok(buf)
}

/// Patch the section-offset fields in the framing header by adding
/// `shift` to each. Used by the encryption assembler to shift
/// section offsets from "signed header" positions (header_length =
/// 136) to "encrypted-signed header" positions (header_length =
/// 224). The body sections move 88 bytes forward in the file.
///
/// Header field offsets being patched:
/// - bytes 32..36: opcode_stream_offset (u32 LE)
/// - bytes 40..44: operand_pool_offset (u32 LE)
/// - bytes 48..52: aux_body_offset (u32 LE)
#[cfg(all(feature = "signatures", feature = "encryption"))]
fn patch_section_offsets(buf: &mut [u8], shift: usize) {
    let shift = shift as u32;
    for offset in [32, 40, 48] {
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&buf[offset..offset + 4]);
        let old = u32::from_le_bytes(bytes);
        let new = old + shift;
        buf[offset..offset + 4].copy_from_slice(&new.to_le_bytes());
    }
}

/// Decrypt an encrypted-signed module and reconstruct a buffer
/// equivalent to an unencrypted-signed module of the same content.
/// The reconstructed buffer has the same shape an `unencrypted
/// signed` wire-format buffer would have, so the existing
/// signature-verifying load path can process it without further
/// modification.
///
/// Verifies the signature over the encrypted form FIRST (to
/// authenticate origin before doing any decryption work), then
/// decrypts the body, then constructs the plaintext view with
/// FLAG_ENCRYPTED cleared and section offsets adjusted.
///
/// Requires both the `signatures` and `encryption` cargo features.
///
/// # Parameters
///
/// - `bytes`: the on-disk encrypted-signed bytecode buffer.
/// - `verifying_keys`: the host's trust matrix for signature
///   verification. The signature must validate against at least
///   one of these.
/// - `local_private_key`: the host's X25519 private key. Used to
///   decrypt the body. The corresponding public key must match
///   the `recipient_key_id` in the encryption metadata.
#[cfg(all(feature = "signatures", feature = "encryption"))]
pub fn decrypt_encrypted_signed_to_signed_bytes(
    bytes: &[u8],
    verifying_keys: &[ed25519_dalek::VerifyingKey],
    local_private_key: &[u8; crate::encryption::X25519_PRIVATE_KEY_LEN],
) -> Result<Vec<u8>, LoadError> {
    use crate::encryption::{AES_GCM_TAG_LEN, EncryptionMetadata, decrypt_from_metadata};

    let bytes = strip_shebang_prefix(bytes);

    // Step 1. Basic framing validation.
    if bytes.len() < encrypted_signed_header_length() + WIRE_FORMAT_FOOTER_BYTES {
        return Err(LoadError::Truncated);
    }
    if bytes[0..4] != BYTECODE_MAGIC[..] {
        return Err(LoadError::BadMagic);
    }
    let flags = bytes[15];
    if flags & FLAG_ENCRYPTED == 0 {
        return Err(LoadError::Codec(String::from(
            "decrypt_encrypted_signed_to_signed_bytes called on bytes without FLAG_ENCRYPTED",
        )));
    }
    if flags & FLAG_REQUIRES_SIGNATURE == 0 {
        return Err(LoadError::Codec(String::from(
            "FLAG_ENCRYPTED requires FLAG_REQUIRES_SIGNATURE; encrypted modules must be signed",
        )));
    }

    // Step 2. Verify the signature against the encrypted form.
    // This authenticates origin before any decryption work runs,
    // and ensures the encryption metadata has not been tampered
    // with.
    verify_module_signature(bytes, verifying_keys)?;

    // Step 3. Parse the encryption metadata.
    let encrypted_header_len = encrypted_signed_header_length();
    let signed_header_len = signed_header_length(ED25519_SIGNATURE_BYTES);
    let metadata_offset = signed_header_len;
    let metadata_bytes = &bytes[metadata_offset..metadata_offset + ENCRYPTION_METADATA_BYTES];
    let metadata = EncryptionMetadata::from_bytes(metadata_bytes).ok_or_else(|| {
        LoadError::Codec(String::from(
            "encryption metadata malformed or scheme unsupported",
        ))
    })?;

    // Step 4. Extract ciphertext + tag from the body region.
    let body_start = encrypted_header_len;
    let body_end = bytes.len() - WIRE_FORMAT_FOOTER_BYTES;
    if body_end <= body_start || body_end - body_start < AES_GCM_TAG_LEN {
        return Err(LoadError::Truncated);
    }
    let ciphertext_with_tag = &bytes[body_start..body_end];

    // Step 5. Decrypt.
    let plaintext = decrypt_from_metadata(&metadata, ciphertext_with_tag, local_private_key)
        .map_err(|e| LoadError::Codec(format!("decryption failed: {}", e)))?;

    // Step 6. Reconstruct an equivalent unencrypted-signed buffer.
    // Layout:
    // - bytes 0..64:                  base header (modified)
    // - bytes 64..72:                 signature metadata (copied)
    // - bytes 72..136:                signature (copied)
    // - bytes 136..(136+plaintext):   plaintext body
    // - last 4 bytes:                 fresh CRC
    let reconstructed_total = signed_header_len + plaintext.len() + WIRE_FORMAT_FOOTER_BYTES;
    let mut buf: Vec<u8> = Vec::with_capacity(reconstructed_total);

    // Copy base header from the encrypted form, then patch.
    buf.extend_from_slice(&bytes[..WIRE_FORMAT_HEADER_BYTES]);

    // Clear FLAG_ENCRYPTED.
    buf[15] &= !FLAG_ENCRYPTED;

    // Patch header_length back to signed-only value.
    buf[6..8].copy_from_slice(&(signed_header_len as u16).to_le_bytes());

    // Patch total_length to the reconstructed size.
    buf[8..12].copy_from_slice(&(reconstructed_total as u32).to_le_bytes());

    // Shift section offsets back by 88 (encryption_metadata block
    // is gone in the reconstructed form).
    let shift = encrypted_header_len - signed_header_len;
    patch_section_offsets_subtract(&mut buf, shift);

    // Copy signature metadata and signature bytes.
    buf.extend_from_slice(&bytes[WIRE_FORMAT_HEADER_BYTES..signed_header_len]);

    // Append plaintext body.
    buf.extend_from_slice(&plaintext);

    // Compute and append fresh CRC. The CRC range excludes the
    // last 4 bytes (the CRC itself).
    let crc_offset = reconstructed_total - WIRE_FORMAT_FOOTER_BYTES;
    let crc = crc32(&buf[..crc_offset]);
    buf.extend_from_slice(&crc.to_le_bytes());
    debug_assert_eq!(buf.len(), reconstructed_total);

    // The signature in the reconstructed buffer was valid for the
    // encrypted form, but is no longer valid for the reconstructed
    // (decrypted) form. Callers must not re-verify the signature
    // on the reconstructed buffer; they should verify against the
    // original encrypted bytes (already done in step 2 above).
    //
    // The reconstructed buffer is intended to be passed directly to
    // a deserializer that does NOT re-verify the signature. The
    // simplest way to communicate this is by having the runtime
    // route this through a different entry point that skips the
    // signature check.
    Ok(buf)
}

/// Patch the section-offset fields in the framing header by
/// subtracting `shift` from each. Inverse of [`patch_section_offsets`].
#[cfg(all(feature = "signatures", feature = "encryption"))]
fn patch_section_offsets_subtract(buf: &mut [u8], shift: usize) {
    let shift = shift as u32;
    for offset in [32, 40, 48] {
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&buf[offset..offset + 4]);
        let old = u32::from_le_bytes(bytes);
        let new = old.saturating_sub(shift);
        buf[offset..offset + 4].copy_from_slice(&new.to_le_bytes());
    }
}

/// Decode the V0.2.0 wire format produced by
/// [`module_to_wire_bytes`] back into a [`Module`].
///
/// Validates the framing header, the CRC residue, and the
/// declared section offsets/lengths. Reads the opcode stream
/// and operand pool, deserializes the rkyv-archived auxiliary
/// body, and reconstructs each chunk's `ops` from its byte
/// offset and record count.
///
/// V0.2.0 Phase 7b ships this function as a parallel route to
/// [`Module::from_bytes`]; the cutover is Phase 7c.
pub fn module_from_wire_bytes(bytes: &[u8]) -> Result<Module, LoadError> {
    let bytes = strip_shebang_prefix(bytes);
    if bytes.len() < WIRE_FORMAT_HEADER_BYTES + WIRE_FORMAT_FOOTER_BYTES {
        return Err(LoadError::Truncated);
    }
    if bytes[0..4] != BYTECODE_MAGIC {
        return Err(LoadError::BadMagic);
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != BYTECODE_VERSION {
        return Err(LoadError::UnsupportedVersion {
            got: version,
            expected: BYTECODE_VERSION,
        });
    }
    let header_length = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
    if header_length < WIRE_FORMAT_HEADER_BYTES {
        return Err(LoadError::Codec(format!(
            "wire format header_length {} is below the minimum {}",
            header_length, WIRE_FORMAT_HEADER_BYTES
        )));
    }
    if header_length > bytes.len() {
        return Err(LoadError::Truncated);
    }
    let total_length = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    if total_length < header_length + WIRE_FORMAT_FOOTER_BYTES || total_length > bytes.len() {
        return Err(LoadError::Truncated);
    }
    let bytes = &bytes[..total_length];
    if crc32(bytes) != WIRE_FORMAT_CRC32_RESIDUE {
        return Err(LoadError::BadChecksum);
    }
    let word_bits_log2 = bytes[12];
    let addr_bits_log2 = bytes[13];
    let float_bits_log2 = bytes[14];
    let flags = bytes[15];
    let wcet_cycles = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let wcmu_bytes = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    let shared_data_bytes = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
    let private_data_bytes = u32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]);
    let opcode_stream_offset =
        u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]) as usize;
    let opcode_stream_length =
        u32::from_le_bytes([bytes[36], bytes[37], bytes[38], bytes[39]]) as usize;
    let operand_pool_offset =
        u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]) as usize;
    let operand_pool_length =
        u32::from_le_bytes([bytes[44], bytes[45], bytes[46], bytes[47]]) as usize;
    let aux_body_offset = u32::from_le_bytes([bytes[48], bytes[49], bytes[50], bytes[51]]) as usize;
    let aux_body_length = u32::from_le_bytes([bytes[52], bytes[53], bytes[54], bytes[55]]) as usize;
    let reserved_a = u32::from_le_bytes([bytes[56], bytes[57], bytes[58], bytes[59]]);
    let reserved_b = u32::from_le_bytes([bytes[60], bytes[61], bytes[62], bytes[63]]);
    if reserved_a != 0 || reserved_b != 0 {
        return Err(LoadError::Codec(format!(
            "wire format header reserved fields must be zero; got {:#010x} and {:#010x}",
            reserved_a, reserved_b,
        )));
    }
    // Signature-extension consistency. Reuse `parse_wire_sections`'
    // logic by inlining the same check.
    let signed = (flags & FLAG_REQUIRES_SIGNATURE) != 0;
    let sig_info = parse_signature_metadata(bytes, header_length)?;
    match (signed, sig_info) {
        (true, None) => {
            return Err(LoadError::Codec(String::from(
                "FLAG_REQUIRES_SIGNATURE is set but the header carries no signature extension",
            )));
        }
        (false, Some(_)) => {
            return Err(LoadError::Codec(String::from(
                "header carries a signature extension but FLAG_REQUIRES_SIGNATURE is not set; V0.2.0 does not admit audit-only signatures",
            )));
        }
        _ => {}
    }
    // Section bounds sanity. Each section fits entirely within
    // the frame (after the header, before the CRC trailer).
    let body_end = total_length - WIRE_FORMAT_FOOTER_BYTES;
    let in_body = |off: usize, len: usize| -> bool {
        off >= header_length && off.checked_add(len).is_some_and(|end| end <= body_end)
    };
    if !in_body(opcode_stream_offset, opcode_stream_length)
        || !in_body(operand_pool_offset, operand_pool_length)
        || !in_body(aux_body_offset, aux_body_length)
    {
        return Err(LoadError::Truncated);
    }
    if !opcode_stream_length.is_multiple_of(OPCODE_RECORD_BYTES) {
        return Err(LoadError::Codec(format!(
            "opcode stream length {} is not a multiple of the record size {}",
            opcode_stream_length, OPCODE_RECORD_BYTES,
        )));
    }
    if !operand_pool_length.is_multiple_of(OPERAND_POOL_ENTRY_BYTES) {
        return Err(LoadError::Codec(format!(
            "operand pool length {} is not a multiple of the entry size {}",
            operand_pool_length, OPERAND_POOL_ENTRY_BYTES,
        )));
    }

    // Slice the sections.
    let opcode_stream = &bytes[opcode_stream_offset..opcode_stream_offset + opcode_stream_length];
    let operand_pool_bytes = &bytes[operand_pool_offset..operand_pool_offset + operand_pool_length];
    let aux_body_bytes = &bytes[aux_body_offset..aux_body_offset + aux_body_length];

    // Decode the operand pool. Each entry's parity is validated
    // here so a corruption error surfaces before the chunks are
    // walked.
    let mut operand_pool: Vec<OperandPoolEntry> = Vec::with_capacity(operand_pool_bytes.len() / 8);
    for chunk_offset in (0..operand_pool_bytes.len()).step_by(OPERAND_POOL_ENTRY_BYTES) {
        let mut entry_bytes = [0u8; OPERAND_POOL_ENTRY_BYTES];
        entry_bytes.copy_from_slice(
            &operand_pool_bytes[chunk_offset..chunk_offset + OPERAND_POOL_ENTRY_BYTES],
        );
        let entry = OperandPoolEntry(entry_bytes);
        entry
            .check_parity()
            .map_err(|e| LoadError::Codec(format!("operand pool entry corruption: {:?}", e)))?;
        operand_pool.push(entry);
    }

    // Validate target widths against the runtime maxima before
    // touching the auxiliary body so the most specific error
    // surfaces. A header field that exceeds the runtime maximum
    // is reported as the matching SizeMismatch variant rather
    // than as a header-versus-aux mismatch.
    if word_bits_log2 > crate::bytecode::RUNTIME_WORD_BITS_LOG2 {
        return Err(LoadError::WordSizeMismatch {
            got: word_bits_log2,
            max_supported: crate::bytecode::RUNTIME_WORD_BITS_LOG2,
        });
    }
    if addr_bits_log2 > crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2 {
        return Err(LoadError::AddressSizeMismatch {
            got: addr_bits_log2,
            max_supported: crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
        });
    }
    if float_bits_log2 > crate::bytecode::RUNTIME_FLOAT_BITS_LOG2 {
        return Err(LoadError::FloatSizeMismatch {
            got: float_bits_log2,
            max_supported: crate::bytecode::RUNTIME_FLOAT_BITS_LOG2,
        });
    }

    // Deserialize the auxiliary body. `rkyv::from_bytes` calls
    // `rkyv::access` internally which validates the archive in
    // place; the validation requires the buffer to be aligned to
    // `align_of::<ArchivedWireAuxBody>()` (8 bytes). The
    // `aux_body_bytes` subslice inherits the input buffer's
    // alignment, which is not guaranteed for `include_bytes!`
    // payloads, file reads, or arbitrary host-supplied byte
    // sources. Copy into an 8-byte-aligned scratch buffer so the
    // archive validation observes the required alignment on every
    // target architecture. The legacy `Module::from_bytes` path
    // (pre V0.2.0 Phase 7c) used the same pattern; the cutover
    // dropped the copy step, which produced an unaligned decode
    // on 32-bit targets where the input buffer happens to land at
    // a 4-byte boundary.
    let mut aligned: rkyv::util::AlignedVec<8> =
        rkyv::util::AlignedVec::with_capacity(aux_body_bytes.len());
    aligned.extend_from_slice(aux_body_bytes);
    let aux = rkyv::from_bytes::<WireAuxBody, rkyv::rancor::Error>(&aligned)
        .map_err(|e| LoadError::Codec(format!("aux body decode failed: {}", e)))?;

    // Cross-check header-mirrored fields against the auxiliary
    // body. The header is the fast-path view; the aux body is
    // the authoritative copy. Disagreement signals a malformed
    // producer or a corrupted artefact.
    if aux.word_bits_log2 != word_bits_log2
        || aux.addr_bits_log2 != addr_bits_log2
        || aux.float_bits_log2 != float_bits_log2
        || aux.flags != flags
        || aux.wcet_cycles != wcet_cycles
        || aux.wcmu_bytes != wcmu_bytes
        || aux.shared_data_bytes != shared_data_bytes
        || aux.private_data_bytes != private_data_bytes
    {
        return Err(LoadError::Codec(String::from(
            "wire-format header fields disagree with the auxiliary body",
        )));
    }

    // Decode chunks. Each WireChunk identifies its opcode span
    // by (op_byte_offset, op_record_count). The records and any
    // pool entries they reference are read from the previously-
    // decoded `operand_pool`.
    let mut chunks: Vec<Chunk> = Vec::with_capacity(aux.chunks.len());
    for wc in &aux.chunks {
        let start = wc.op_byte_offset as usize;
        let record_count = wc.op_record_count as usize;
        let byte_span = record_count
            .checked_mul(OPCODE_RECORD_BYTES)
            .ok_or_else(|| LoadError::Codec(String::from("opcode span overflow")))?;
        let end = start
            .checked_add(byte_span)
            .ok_or_else(|| LoadError::Codec(String::from("opcode span overflow")))?;
        if end > opcode_stream.len() {
            return Err(LoadError::Codec(format!(
                "chunk `{}` opcode span [{}..{}) exceeds opcode stream length {}",
                wc.name,
                start,
                end,
                opcode_stream.len(),
            )));
        }
        let mut ops: Vec<Op> = Vec::with_capacity(record_count);
        let chunk_bytes = &opcode_stream[start..end];
        for offset in (0..byte_span).step_by(OPCODE_RECORD_BYTES) {
            let mut rec = [0u8; OPCODE_RECORD_BYTES];
            rec.copy_from_slice(&chunk_bytes[offset..offset + OPCODE_RECORD_BYTES]);
            let op = decode_op(OpcodeRecord(rec), &operand_pool)
                .map_err(|e| LoadError::Codec(format!("opcode decode failed: {:?}", e)))?;
            ops.push(op);
        }
        let debug_pool = match &wc.debug_pool_bytes {
            Some(bytes) => Some(
                crate::debug_meta::DebugPool::decode(bytes)
                    .map_err(|e| LoadError::Codec(format!("debug pool decode failed: {:?}", e)))?,
            ),
            None => None,
        };
        chunks.push(Chunk {
            name: wc.name.clone(),
            ops,
            constants: wc.constants.clone(),
            struct_templates: wc.struct_templates.clone(),
            local_count: wc.local_count,
            param_count: wc.param_count,
            block_type: wc.block_type,
            param_types: wc.param_types.clone(),
            debug_pool,
        });
    }

    Ok(Module {
        chunks,
        native_names: aux.native_names,
        entry_point: aux.entry_point,
        data_layout: aux.data_layout,
        word_bits_log2: aux.word_bits_log2,
        addr_bits_log2: aux.addr_bits_log2,
        float_bits_log2: aux.float_bits_log2,
        wcet_cycles: aux.wcet_cycles,
        wcmu_bytes: aux.wcmu_bytes,
        flags: aux.flags,
        shared_data_bytes: aux.shared_data_bytes,
        private_data_bytes: aux.private_data_bytes,
        schema_hash: aux.schema_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(op: Op) {
        let mut pool: Vec<OperandPoolEntry> = Vec::new();
        let record = encode_op(&op, &mut pool).expect("encode");
        record.check_parity().expect("parity");
        let decoded = decode_op(record, &pool).expect("decode");
        assert_eq!(op, decoded);
    }

    #[test]
    fn opcode_id_table_is_dense_and_unique() {
        // Every identifier in the table is in range and the
        // mapping is a bijection. The table is part of the wire-
        // format ABI; a regression here is a wire-format break.
        let mut seen: alloc::collections::BTreeSet<u8> = alloc::collections::BTreeSet::new();
        for (_, id) in OPCODE_ID_TABLE.iter() {
            assert!(*id < 128, "opcode id {} does not fit in seven bits", id);
            assert!(seen.insert(*id), "duplicate opcode id {}", id);
        }
        assert_eq!(seen.len(), OPCODE_ID_TABLE.len());
    }

    #[test]
    fn opcode_id_of_matches_table() {
        // For each variant in the canonical table, build the
        // representative `Op` value and confirm `opcode_id_of`
        // returns the same identifier.
        let cases: &[(Op, u8)] = &[
            (Op::Const(0), 0),
            (Op::GetLocal(0), 1),
            (Op::SetLocal(0), 2),
            (Op::GetData(0), 3),
            (Op::SetData(0), 4),
            (Op::GetDataIndexed(0, 0), 5),
            (Op::SetDataIndexed(0, 0), 6),
            (Op::BoundsCheck(0), 7),
            (Op::Add, 8),
            (Op::Sub, 9),
            (Op::Mul, 10),
            (Op::Div, 11),
            (Op::Mod, 12),
            (Op::Neg, 13),
            (Op::CmpEq, 14),
            (Op::CmpNe, 15),
            (Op::CmpLt, 16),
            (Op::CmpGt, 17),
            (Op::CmpLe, 18),
            (Op::CmpGe, 19),
            (Op::Not, 20),
            (Op::If(0), 21),
            (Op::Else(0), 22),
            (Op::EndIf, 23),
            (Op::Loop(0), 24),
            (Op::EndLoop(0), 25),
            (Op::Break(0), 26),
            (Op::BreakIf(0), 27),
            (Op::Stream, 28),
            (Op::Reset, 29),
            (Op::Call(0, 0), 30),
            (Op::Return, 31),
            (Op::Yield, 32),
            (Op::Dup, 33),
            (Op::NewStruct(0), 34),
            (Op::NewEnum(0, 0, 0), 35),
            (Op::NewArray(0), 36),
            (Op::NewTuple(0), 37),
            (
                Op::GetField(crate::bytecode::StructField::Boxed { name_const: 0 }),
                38,
            ),
            (Op::GetIndex(crate::bytecode::ArrayElem::Boxed), 39),
            (
                Op::GetTupleField(crate::bytecode::TupleField::Boxed { index: 0 }),
                40,
            ),
            (
                Op::GetEnumField(crate::bytecode::EnumField::Boxed { index: 0 }),
                41,
            ),
            (Op::Len, 42),
            (Op::IsEnum(0, 0), 43),
            (Op::IsStruct(0), 44),
            (Op::IntToFloat, 45),
            (Op::FloatToInt, 46),
            (Op::WordToByte, 47),
            (Op::ByteToWord, 48),
            (Op::WordToFixed(0), 49),
            (Op::FixedToWord(0), 50),
            (Op::FixedMul(0), 51),
            (Op::FixedDiv(0), 52),
            (Op::Trap(0), 53),
            (Op::CheckedAdd, 54),
            (Op::CheckedSub, 55),
            (Op::CheckedMul(0), 56),
            (Op::CheckedNeg, 57),
            (Op::CheckedDiv(0), 58),
            (Op::CheckedMod, 59),
            (Op::PushImmediate(0), 60),
            (Op::PopN(0), 61),
            (Op::BitAnd, 62),
            (Op::BitOr, 63),
            (Op::BitXor, 64),
            (Op::Shl, 65),
            (Op::Shr, 66),
            (Op::CallVerifiedNative(0, 0), 67),
            (Op::CallExternalNative(0, 0), 68),
        ];
        for (op, expected) in cases {
            assert_eq!(
                opcode_id_of(op).0,
                *expected,
                "wrong opcode id for {:?}",
                op,
            );
        }
        assert_eq!(cases.len(), OPCODE_ID_TABLE.len());
    }

    #[test]
    fn opcode_record_roundtrip_no_operand() {
        for op in [
            Op::Add,
            Op::Sub,
            Op::Mul,
            Op::Div,
            Op::Mod,
            Op::Neg,
            Op::CmpEq,
            Op::CmpNe,
            Op::CmpLt,
            Op::CmpGt,
            Op::CmpLe,
            Op::CmpGe,
            Op::Not,
            Op::EndIf,
            Op::Stream,
            Op::Reset,
            Op::Return,
            Op::Yield,
            Op::Dup,
            Op::Len,
            Op::IntToFloat,
            Op::FloatToInt,
            Op::WordToByte,
            Op::ByteToWord,
            Op::CheckedAdd,
            Op::CheckedSub,
            Op::CheckedNeg,
            Op::CheckedMod,
            Op::BitAnd,
            Op::BitOr,
            Op::BitXor,
            Op::Shl,
            Op::Shr,
        ] {
            roundtrip(op);
        }
    }

    #[test]
    fn opcode_record_roundtrip_u8_operand() {
        for op in [
            Op::NewTuple(0),
            Op::NewTuple(255),
            Op::GetEnumField(crate::bytecode::EnumField::Boxed { index: 3 }),
            Op::WordToFixed(32),
            Op::FixedToWord(16),
            Op::FixedMul(8),
            Op::FixedDiv(4),
            Op::CheckedMul(0),
            Op::CheckedMul(8),
            Op::CheckedDiv(0),
            Op::CheckedDiv(4),
            Op::PushImmediate(5),
            Op::PopN(2),
        ] {
            roundtrip(op);
        }
    }

    #[test]
    fn opcode_record_roundtrip_tuple_field() {
        use crate::bytecode::TupleField;
        use crate::value_layout::ScalarKind;
        for op in [
            Op::GetTupleField(TupleField::Boxed { index: 0 }),
            Op::GetTupleField(TupleField::Boxed { index: 255 }),
            Op::GetTupleField(TupleField::Flat {
                offset: 0,
                kind: ScalarKind::Bool,
            }),
            Op::GetTupleField(TupleField::Flat {
                offset: 9,
                kind: ScalarKind::Int,
            }),
            Op::GetTupleField(TupleField::Flat {
                offset: 65535,
                kind: ScalarKind::Byte,
            }),
            Op::GetTupleField(TupleField::Flat {
                offset: 16,
                kind: ScalarKind::Fixed,
            }),
            Op::GetIndex(crate::bytecode::ArrayElem::Boxed),
            Op::GetIndex(crate::bytecode::ArrayElem::Flat {
                kind: ScalarKind::Int,
            }),
            Op::GetIndex(crate::bytecode::ArrayElem::Flat {
                kind: ScalarKind::Byte,
            }),
        ] {
            roundtrip(op);
        }
    }

    #[test]
    fn tuple_field_unknown_kind_tag_rejected() {
        // Byte three carries a kind tag of 9, which maps to no
        // ScalarKind and is not the boxed sentinel, so the decoder
        // reports a corrupted operand rather than fabricating a kind.
        let record = OpcodeRecord::from_id_and_operand(OpcodeId(40), [0, 0, 9]);
        let result = decode_op(record, &[]);
        assert_eq!(result, Err(WireFormatError::TupleFieldKindUnknown(9)));
    }

    #[test]
    fn opcode_record_roundtrip_u16_operand() {
        for op in [
            Op::Const(0),
            Op::Const(65535),
            Op::GetLocal(12),
            Op::SetLocal(34),
            Op::GetData(56),
            Op::SetData(78),
            Op::BoundsCheck(100),
            Op::If(200),
            Op::Else(300),
            Op::Loop(400),
            Op::EndLoop(500),
            Op::Break(600),
            Op::BreakIf(700),
            Op::NewStruct(800),
            Op::NewArray(900),
            Op::GetField(crate::bytecode::StructField::Boxed { name_const: 1000 }),
            Op::IsStruct(1100),
            Op::Trap(1200),
        ] {
            roundtrip(op);
        }
    }

    #[test]
    fn opcode_record_roundtrip_u16_u8_operand() {
        for op in [
            Op::Call(0, 0),
            Op::Call(65535, 255),
            Op::CallVerifiedNative(42, 3),
            Op::CallExternalNative(43, 1),
        ] {
            roundtrip(op);
        }
    }

    #[test]
    fn opcode_record_roundtrip_pool_u16_u16() {
        for op in [
            Op::GetDataIndexed(0, 0),
            Op::GetDataIndexed(65535, 65535),
            Op::SetDataIndexed(100, 200),
            Op::IsEnum(7, 13),
        ] {
            roundtrip(op);
        }
    }

    #[test]
    fn opcode_record_roundtrip_pool_u16_u16_u8() {
        for op in [
            Op::NewEnum(0, 0, 0),
            Op::NewEnum(65535, 65535, 255),
            Op::NewEnum(1, 2, 3),
        ] {
            roundtrip(op);
        }
    }

    #[test]
    fn parity_detects_bit_flip_in_opcode_record() {
        let mut pool: Vec<OperandPoolEntry> = Vec::new();
        let record = encode_op(&Op::Const(42), &mut pool).expect("encode");
        // Flip one bit in byte one.
        let mut corrupted = record.0;
        corrupted[1] ^= 0x01;
        let result = OpcodeRecord(corrupted).check_parity();
        assert_eq!(result, Err(WireFormatError::OpcodeRecordParityMismatch));
    }

    #[test]
    fn parity_detects_bit_flip_in_opcode_id() {
        let mut pool: Vec<OperandPoolEntry> = Vec::new();
        let record = encode_op(&Op::Add, &mut pool).expect("encode");
        let mut corrupted = record.0;
        // Flip a bit in the opcode id (not the parity bit).
        corrupted[0] ^= 0x01;
        let result = OpcodeRecord(corrupted).check_parity();
        assert_eq!(result, Err(WireFormatError::OpcodeRecordParityMismatch));
    }

    #[test]
    fn parity_detects_bit_flip_in_pool_entry() {
        let entry = OperandPoolEntry::from_u16_u16(1234, 5678);
        let mut corrupted = entry.0;
        // Flip a bit in the payload.
        corrupted[3] ^= 0x80;
        let result = OperandPoolEntry(corrupted).check_parity();
        assert_eq!(result, Err(WireFormatError::OperandPoolParityMismatch));
    }

    #[test]
    fn pool_tag_mismatch_surfaces_error() {
        // A pool entry tagged for (u16, u16) cannot satisfy an
        // opcode that wants (u16, u16, u8). Hand-craft the
        // mismatch and confirm the decoder rejects.
        let mut pool: Vec<OperandPoolEntry> = Vec::new();
        // First, encode something that uses the (u16, u16) shape.
        let _record = encode_op(&Op::GetDataIndexed(1, 2), &mut pool).expect("encode");
        // Manufacture a NewEnum record that references the same
        // (mismatched) entry.
        let id = opcode_id_of(&Op::NewEnum(0, 0, 0));
        let idx_bytes = (0u32).to_le_bytes();
        let bad_record =
            OpcodeRecord::from_id_and_operand(id, [idx_bytes[0], idx_bytes[1], idx_bytes[2]]);
        let result = decode_op(bad_record, &pool);
        assert_eq!(
            result,
            Err(WireFormatError::OperandPoolTagMismatch {
                observed: POOL_TAG_U16_U16,
                expected: POOL_TAG_U16_U16_U8,
            }),
        );
    }

    #[test]
    fn pool_index_out_of_bounds_surfaces_error() {
        let pool: Vec<OperandPoolEntry> = Vec::new();
        let id = opcode_id_of(&Op::GetDataIndexed(0, 0));
        let idx_bytes = (5u32).to_le_bytes();
        let record =
            OpcodeRecord::from_id_and_operand(id, [idx_bytes[0], idx_bytes[1], idx_bytes[2]]);
        let result = decode_op(record, &pool);
        assert_eq!(result, Err(WireFormatError::OperandPoolIndexOutOfBounds(5)));
    }

    #[test]
    fn unknown_opcode_id_surfaces_error() {
        // Identifier 100 is unassigned in V0.2.0. The decoder
        // surfaces `UnknownOpcodeId` rather than panicking.
        let record = OpcodeRecord::from_id_and_operand(OpcodeId(100), [0, 0, 0]);
        let pool: Vec<OperandPoolEntry> = Vec::new();
        let result = decode_op(record, &pool);
        assert_eq!(result, Err(WireFormatError::UnknownOpcodeId(100)));
    }

    fn module_roundtrip_through_wire_format(module: Module) {
        let bytes = module_to_wire_bytes(&module).expect("encode");
        // Header sanity: magic and version.
        assert_eq!(&bytes[0..4], &BYTECODE_MAGIC);
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), BYTECODE_VERSION,);
        assert_eq!(
            u16::from_le_bytes([bytes[6], bytes[7]]),
            WIRE_FORMAT_HEADER_BYTES as u16,
        );
        let decoded = module_from_wire_bytes(&bytes).expect("decode");
        // Compare structurally. The Module derives `Debug + Clone`
        // but not `PartialEq`; compare through serialized bytes.
        let re_encoded = module_to_wire_bytes(&decoded).expect("re-encode");
        assert_eq!(bytes, re_encoded, "wire-format round trip differs");
    }

    fn make_minimal_module() -> Module {
        // Hand-crafted chunk: PushImmediate(1) then Return.
        let chunk = Chunk {
            name: alloc::string::String::from("main"),
            ops: alloc::vec![Op::PushImmediate(5), Op::Return],
            constants: alloc::vec::Vec::new(),
            struct_templates: alloc::vec::Vec::new(),
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Func,
            param_types: alloc::vec::Vec::new(),
            debug_pool: None,
        };
        Module {
            chunks: alloc::vec![chunk],
            native_names: alloc::vec::Vec::new(),
            entry_point: Some(0),
            data_layout: None,
            word_bits_log2: crate::bytecode::RUNTIME_WORD_BITS_LOG2,
            addr_bits_log2: crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
            float_bits_log2: crate::bytecode::RUNTIME_FLOAT_BITS_LOG2,
            wcet_cycles: 0,
            wcmu_bytes: 0,
            flags: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
            schema_hash: 0,
        }
    }

    #[test]
    fn module_roundtrip_empty_chunks() {
        // Zero chunks: opcode stream and operand pool both empty.
        // Verifies the section-offset math and the rkyv encoding
        // of an empty `WireAuxBody::chunks`.
        let module = Module {
            chunks: alloc::vec::Vec::new(),
            native_names: alloc::vec::Vec::new(),
            entry_point: None,
            data_layout: None,
            word_bits_log2: crate::bytecode::RUNTIME_WORD_BITS_LOG2,
            addr_bits_log2: crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
            float_bits_log2: crate::bytecode::RUNTIME_FLOAT_BITS_LOG2,
            wcet_cycles: 0,
            wcmu_bytes: 0,
            flags: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
            schema_hash: 0,
        };
        module_roundtrip_through_wire_format(module);
    }

    #[test]
    fn module_roundtrip_minimal_program() {
        module_roundtrip_through_wire_format(make_minimal_module());
    }

    fn sample_debug_pool() -> crate::debug_meta::DebugPool {
        use crate::debug_meta::{DebugPool, DebugRecord, DebugRecordKind};
        DebugPool {
            string_pool: alloc::vec![alloc::string::String::from("main.kel")],
            span_pool: alloc::vec![(0, 0, 2)],
            type_pool: alloc::vec::Vec::new(),
            records: alloc::vec![
                DebugRecord {
                    op_index: 0,
                    kind: DebugRecordKind::SourceSpan,
                    operands: alloc::vec![0],
                },
                DebugRecord {
                    op_index: 1,
                    kind: DebugRecordKind::CallSite,
                    operands: alloc::vec![0],
                },
            ],
        }
    }

    #[test]
    fn module_roundtrip_preserves_debug_pool() {
        let mut module = make_minimal_module();
        let pool = sample_debug_pool();
        module.chunks[0].debug_pool = Some(pool.clone());
        let bytes = module_to_wire_bytes(&module).expect("encode");
        let decoded = module_from_wire_bytes(&bytes).expect("decode");
        let decoded_pool = decoded.chunks[0]
            .debug_pool
            .as_ref()
            .expect("debug pool survives round trip");
        // Compare canonically; `encode` is record-order independent.
        assert_eq!(decoded_pool.encode(), pool.encode());
        // Decode then re-encode is byte-stable.
        let re = module_to_wire_bytes(&decoded).expect("re-encode");
        assert_eq!(bytes, re);
    }

    #[test]
    fn debug_pool_does_not_alter_opcode_stream() {
        // B29 invariant 4: debug metadata lives only in the auxiliary
        // body, so the opcode stream section is byte-identical whether
        // or not a chunk carries a debug pool.
        let without = make_minimal_module();
        let mut with = make_minimal_module();
        with.chunks[0].debug_pool = Some(sample_debug_pool());

        let bytes_without = module_to_wire_bytes(&without).expect("encode");
        let bytes_with = module_to_wire_bytes(&with).expect("encode");

        let s_without = parse_wire_sections(&bytes_without).expect("sections");
        let s_with = parse_wire_sections(&bytes_with).expect("sections");
        assert_eq!(
            s_without.opcode_stream, s_with.opcode_stream,
            "debug metadata must not change the opcode stream"
        );
    }

    #[test]
    fn stripping_debug_pool_reproduces_release_bytes() {
        // B29 invariant 5 at the wire level: encoding a module whose
        // debug pool has been dropped yields bytes identical to a module
        // that never carried one.
        let release = make_minimal_module();
        let release_bytes = module_to_wire_bytes(&release).expect("encode");

        let mut debug = make_minimal_module();
        debug.chunks[0].debug_pool = Some(sample_debug_pool());
        // Strip: drop the section.
        debug.chunks[0].debug_pool = None;
        let stripped_bytes = module_to_wire_bytes(&debug).expect("encode");

        assert_eq!(
            release_bytes, stripped_bytes,
            "stripped bytecode must be byte-identical to a release build"
        );
    }

    #[test]
    fn module_roundtrip_branchy_program() {
        // Exercise If/Else/EndIf and a loop with a BreakIf.
        let body_chunk = Chunk {
            name: alloc::string::String::from("loop_body"),
            ops: alloc::vec![
                Op::Loop(7),
                Op::PushImmediate(2),
                Op::PushImmediate(2),
                Op::CmpEq,
                Op::BreakIf(7),
                Op::EndLoop(1),
                Op::PushImmediate(0),
                Op::Return,
            ],
            constants: alloc::vec::Vec::new(),
            struct_templates: alloc::vec::Vec::new(),
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Func,
            param_types: alloc::vec::Vec::new(),
            debug_pool: None,
        };
        let if_chunk = Chunk {
            name: alloc::string::String::from("if_chain"),
            ops: alloc::vec![
                Op::PushImmediate(1),
                Op::If(4),
                Op::PushImmediate(5),
                Op::Else(5),
                Op::PushImmediate(6),
                Op::EndIf,
                Op::Return,
            ],
            constants: alloc::vec::Vec::new(),
            struct_templates: alloc::vec::Vec::new(),
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Func,
            param_types: alloc::vec::Vec::new(),
            debug_pool: None,
        };
        let module = Module {
            chunks: alloc::vec![body_chunk, if_chunk],
            native_names: alloc::vec::Vec::new(),
            entry_point: Some(0),
            data_layout: None,
            word_bits_log2: crate::bytecode::RUNTIME_WORD_BITS_LOG2,
            addr_bits_log2: crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
            float_bits_log2: crate::bytecode::RUNTIME_FLOAT_BITS_LOG2,
            wcet_cycles: 0,
            wcmu_bytes: 0,
            flags: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
            schema_hash: 0,
        };
        module_roundtrip_through_wire_format(module);
    }

    #[test]
    fn module_roundtrip_pool_using_program() {
        // Exercise the operand pool. NewEnum uses the
        // `(u16, u16, u8)` shape; IsEnum and GetDataIndexed use
        // `(u16, u16)`. All four pool-using opcodes appear at
        // least once.
        let chunk = Chunk {
            name: alloc::string::String::from("pool_user"),
            ops: alloc::vec![
                Op::NewEnum(3, 4, 1),
                Op::NewEnum(0, 0, 0),
                Op::IsEnum(3, 4),
                Op::GetDataIndexed(7, 8),
                Op::SetDataIndexed(7, 8),
                Op::Return,
            ],
            constants: alloc::vec::Vec::new(),
            struct_templates: alloc::vec::Vec::new(),
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Func,
            param_types: alloc::vec::Vec::new(),
            debug_pool: None,
        };
        let module = Module {
            chunks: alloc::vec![chunk],
            native_names: alloc::vec::Vec::new(),
            entry_point: Some(0),
            data_layout: None,
            word_bits_log2: crate::bytecode::RUNTIME_WORD_BITS_LOG2,
            addr_bits_log2: crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
            float_bits_log2: crate::bytecode::RUNTIME_FLOAT_BITS_LOG2,
            wcet_cycles: 0,
            wcmu_bytes: 0,
            flags: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
            schema_hash: 0,
        };
        module_roundtrip_through_wire_format(module);
    }

    #[test]
    fn module_roundtrip_stream_chunk() {
        // Stream chunks have distinct structural constraints
        // (Op::Stream, Op::Yield, Op::Reset). Round-trip a
        // minimal Stream chunk to confirm block-type preservation
        // and full opcode stream coverage.
        let chunk = Chunk {
            name: alloc::string::String::from("tick"),
            ops: alloc::vec![
                Op::Stream,
                Op::PushImmediate(7),
                Op::Yield,
                Op::PopN(1),
                Op::Reset,
            ],
            constants: alloc::vec::Vec::new(),
            struct_templates: alloc::vec::Vec::new(),
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Stream,
            param_types: alloc::vec::Vec::new(),
            debug_pool: None,
        };
        let module = Module {
            chunks: alloc::vec![chunk],
            native_names: alloc::vec::Vec::new(),
            entry_point: Some(0),
            data_layout: None,
            word_bits_log2: crate::bytecode::RUNTIME_WORD_BITS_LOG2,
            addr_bits_log2: crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
            float_bits_log2: crate::bytecode::RUNTIME_FLOAT_BITS_LOG2,
            wcet_cycles: 0,
            wcmu_bytes: 0,
            flags: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
            schema_hash: 0,
        };
        module_roundtrip_through_wire_format(module);
    }

    #[test]
    fn module_roundtrip_bad_magic_rejected() {
        let module = make_minimal_module();
        let mut bytes = module_to_wire_bytes(&module).expect("encode");
        bytes[0] = b'X';
        let err = module_from_wire_bytes(&bytes).unwrap_err();
        assert!(matches!(err, LoadError::BadMagic));
    }

    #[test]
    fn module_roundtrip_bad_crc_rejected() {
        let module = make_minimal_module();
        let mut bytes = module_to_wire_bytes(&module).expect("encode");
        let len = bytes.len();
        bytes[len - 1] ^= 0x01;
        let err = module_from_wire_bytes(&bytes).unwrap_err();
        assert!(matches!(err, LoadError::BadChecksum));
    }

    #[test]
    fn module_roundtrip_truncated_rejected() {
        let module = make_minimal_module();
        let bytes = module_to_wire_bytes(&module).expect("encode");
        let err = module_from_wire_bytes(&bytes[..32]).unwrap_err();
        assert!(matches!(err, LoadError::Truncated));
    }

    #[test]
    fn module_roundtrip_shebang_stripped() {
        let module = make_minimal_module();
        let bytes = module_to_wire_bytes(&module).expect("encode");
        let mut with_shebang: alloc::vec::Vec<u8> =
            alloc::vec::Vec::from(b"#!/usr/bin/env keleusma\n".as_slice());
        with_shebang.extend_from_slice(&bytes);
        let decoded = module_from_wire_bytes(&with_shebang).expect("decode");
        // Confirm the decoded chunk preserves the ops.
        assert_eq!(decoded.chunks.len(), 1);
        assert_eq!(decoded.chunks[0].ops.len(), 2);
    }

    #[test]
    fn unsigned_module_header_length_is_64() {
        let bytes = module_to_wire_bytes(&make_minimal_module()).expect("encode");
        let header_length = u16::from_le_bytes([bytes[6], bytes[7]]);
        assert_eq!(header_length, WIRE_FORMAT_HEADER_BYTES as u16);
        // Flags byte has FLAG_REQUIRES_SIGNATURE clear.
        assert_eq!(bytes[15] & FLAG_REQUIRES_SIGNATURE, 0);
    }

    #[test]
    fn flag_requires_signature_without_extension_rejected() {
        // Construct an otherwise-valid file but flip the signed
        // bit without adding a signature extension. The decoder
        // must reject as malformed (header_length still 64).
        let mut bytes = module_to_wire_bytes(&make_minimal_module()).expect("encode");
        bytes[15] |= FLAG_REQUIRES_SIGNATURE;
        // Repair the CRC trailer so we exercise the flag/extension
        // consistency check rather than the CRC check.
        let footer_start = bytes.len() - WIRE_FORMAT_FOOTER_BYTES;
        let crc = crc32(&bytes[..footer_start]);
        bytes[footer_start..].copy_from_slice(&crc.to_le_bytes());
        let err = module_from_wire_bytes(&bytes).unwrap_err();
        match err {
            LoadError::Codec(msg) => assert!(
                msg.contains("FLAG_REQUIRES_SIGNATURE is set"),
                "expected flag/extension consistency error, got: {}",
                msg
            ),
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn parse_signature_metadata_rejects_unsupported_scheme() {
        // Hand-craft a buffer with a fake signature extension
        // claiming scheme_id = 99 (not Ed25519).
        let mut bytes = alloc::vec![0u8; 144];
        bytes[64] = 99; // scheme_id
        bytes[66] = 64; // signature_length (lo)
        let err = parse_signature_metadata(&bytes, 144).unwrap_err();
        match err {
            LoadError::Codec(msg) => assert!(
                msg.contains("scheme_id 99 is not supported"),
                "expected scheme rejection, got: {}",
                msg
            ),
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn parse_signature_metadata_rejects_scheme_id_zero() {
        let bytes = alloc::vec![0u8; 144];
        let err = parse_signature_metadata(&bytes, 144).unwrap_err();
        match err {
            LoadError::Codec(msg) => assert!(
                msg.contains("scheme_id 0 is reserved"),
                "expected zero-scheme rejection, got: {}",
                msg
            ),
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn signed_header_length_for_ed25519() {
        assert_eq!(signed_header_length(ED25519_SIGNATURE_BYTES), 136);
    }

    #[cfg(feature = "signatures")]
    #[test]
    fn ed25519_round_trip_verifies() {
        use ed25519_dalek::SigningKey;
        // Deterministic 32-byte seed so the test is reproducible.
        let seed = [7u8; 32];
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();

        let module = make_minimal_module();
        let signed_bytes = module_to_signed_wire_bytes(&module, &signing_key).expect("sign+encode");
        // Header reflects the signed extension.
        assert_eq!(
            u16::from_le_bytes([signed_bytes[6], signed_bytes[7]]),
            signed_header_length(ED25519_SIGNATURE_BYTES) as u16,
        );
        assert_ne!(signed_bytes[15] & FLAG_REQUIRES_SIGNATURE, 0);
        assert_eq!(signed_bytes[64], SIGNATURE_SCHEME_ED25519);
        assert_eq!(
            u16::from_le_bytes([signed_bytes[66], signed_bytes[67]]),
            ED25519_SIGNATURE_BYTES as u16,
        );

        // Verification matches.
        verify_module_signature(&signed_bytes, &[verifying_key]).expect("verify");

        // The decoded module preserves the entry point and op
        // count from the original.
        let decoded = module_from_wire_bytes(&signed_bytes).expect("decode");
        assert_eq!(decoded.entry_point, module.entry_point);
        assert_eq!(decoded.chunks.len(), module.chunks.len());
        assert_eq!(decoded.chunks[0].ops.len(), module.chunks[0].ops.len());
    }

    #[cfg(feature = "signatures")]
    #[test]
    fn ed25519_verify_rejects_wrong_key() {
        use ed25519_dalek::SigningKey;
        let signer = SigningKey::from_bytes(&[7u8; 32]);
        let wrong = SigningKey::from_bytes(&[8u8; 32]).verifying_key();
        let signed = module_to_signed_wire_bytes(&make_minimal_module(), &signer).expect("sign");
        let err = verify_module_signature(&signed, &[wrong]).unwrap_err();
        assert!(
            matches!(err, LoadError::InvalidSignature),
            "expected InvalidSignature, got: {:?}",
            err
        );
    }

    #[cfg(feature = "signatures")]
    #[test]
    fn ed25519_verify_rejects_empty_key_set() {
        use ed25519_dalek::SigningKey;
        let signer = SigningKey::from_bytes(&[7u8; 32]);
        let signed = module_to_signed_wire_bytes(&make_minimal_module(), &signer).expect("sign");
        let err = verify_module_signature(&signed, &[]).unwrap_err();
        assert!(
            matches!(err, LoadError::InvalidSignature),
            "expected InvalidSignature, got: {:?}",
            err
        );
    }

    #[cfg(feature = "signatures")]
    #[test]
    fn ed25519_tamper_in_body_caught_by_crc_before_signature() {
        use ed25519_dalek::SigningKey;
        let signer = SigningKey::from_bytes(&[7u8; 32]);
        let verifying = signer.verifying_key();
        let mut signed =
            module_to_signed_wire_bytes(&make_minimal_module(), &signer).expect("sign");
        // Flip a byte in the opcode stream (well past the
        // signature section). The CRC residue check inside
        // `verify_module_signature` is at the framing layer; it
        // runs before the signature math.
        let opcode_offset = signed_header_length(ED25519_SIGNATURE_BYTES);
        signed[opcode_offset] ^= 0x01;
        let err = verify_module_signature(&signed, &[verifying]).unwrap_err();
        match err {
            LoadError::BadChecksum | LoadError::Codec(_) | LoadError::InvalidSignature => {}
            other => panic!(
                "expected BadChecksum / Codec / InvalidSignature, got: {:?}",
                other
            ),
        }
    }

    #[cfg(feature = "signatures")]
    #[test]
    fn ed25519_signature_mutation_caught_after_crc_repair() {
        use ed25519_dalek::SigningKey;
        let signer = SigningKey::from_bytes(&[7u8; 32]);
        let verifying = signer.verifying_key();
        let mut signed =
            module_to_signed_wire_bytes(&make_minimal_module(), &signer).expect("sign");
        // Flip a bit inside the signature section.
        let sig_offset = WIRE_FORMAT_HEADER_BYTES + SIGNATURE_METADATA_BYTES;
        signed[sig_offset] ^= 0x01;
        // Repair the CRC trailer so the framing-level check
        // passes and only the cryptographic check rejects.
        let footer_start = signed.len() - WIRE_FORMAT_FOOTER_BYTES;
        let crc = crc32(&signed[..footer_start]);
        signed[footer_start..].copy_from_slice(&crc.to_le_bytes());
        let err = verify_module_signature(&signed, &[verifying]).unwrap_err();
        assert!(
            matches!(err, LoadError::InvalidSignature),
            "expected InvalidSignature after sig mutation, got: {:?}",
            err
        );
    }

    #[cfg(all(feature = "signatures", feature = "encryption"))]
    #[test]
    fn encrypted_signed_wire_round_trip() {
        use crate::encryption::public_key_from_private;
        use ed25519_dalek::SigningKey;

        let signer = SigningKey::from_bytes(&[0xa1; 32]);
        let verifying = signer.verifying_key();

        // Recipient X25519 keypair.
        let recipient_sk = [0xb2u8; 32];
        let recipient_pk = public_key_from_private(&recipient_sk);

        // Ephemeral seed for this module.
        let ephemeral_seed = [0xc3u8; 32];

        let module = make_minimal_module();

        // Encrypt and sign.
        let encrypted =
            module_to_encrypted_signed_wire_bytes(&module, &signer, &recipient_pk, &ephemeral_seed)
                .expect("encrypt+sign");

        // Header flags should advertise both signing and encryption.
        assert_ne!(encrypted[15] & FLAG_REQUIRES_SIGNATURE, 0);
        assert_ne!(encrypted[15] & FLAG_ENCRYPTED, 0);

        // header_length should equal encrypted_signed_header_length.
        let hl = u16::from_le_bytes([encrypted[6], encrypted[7]]) as usize;
        assert_eq!(hl, encrypted_signed_header_length());

        // Decrypt back to signed-only form.
        let reconstructed =
            decrypt_encrypted_signed_to_signed_bytes(&encrypted, &[verifying], &recipient_sk)
                .expect("decrypt");

        // The reconstructed buffer should NOT have FLAG_ENCRYPTED.
        assert_eq!(reconstructed[15] & FLAG_ENCRYPTED, 0);
        // But should still carry FLAG_REQUIRES_SIGNATURE.
        assert_ne!(reconstructed[15] & FLAG_REQUIRES_SIGNATURE, 0);

        // The reconstructed buffer should parse back to the same
        // module as the original. Round-trip through
        // module_from_wire_bytes gives us a Module value to compare.
        let decoded = module_from_wire_bytes(&reconstructed).expect("parse reconstructed");
        assert_eq!(decoded.chunks.len(), module.chunks.len());
        assert_eq!(decoded.chunks[0].ops, module.chunks[0].ops);
    }

    #[cfg(all(feature = "signatures", feature = "encryption"))]
    #[test]
    fn encrypted_signed_wrong_recipient_rejected() {
        use crate::encryption::public_key_from_private;
        use ed25519_dalek::SigningKey;

        let signer = SigningKey::from_bytes(&[0xa2; 32]);
        let verifying = signer.verifying_key();

        let alice_sk = [0x11u8; 32];
        let alice_pk = public_key_from_private(&alice_sk);
        let bob_sk = [0x22u8; 32];

        let ephemeral_seed = [0x33u8; 32];

        let encrypted = module_to_encrypted_signed_wire_bytes(
            &make_minimal_module(),
            &signer,
            &alice_pk,
            &ephemeral_seed,
        )
        .expect("encrypt+sign");

        let result = decrypt_encrypted_signed_to_signed_bytes(&encrypted, &[verifying], &bob_sk);
        assert!(result.is_err(), "expected error decrypting as bob");
    }

    #[cfg(all(feature = "signatures", feature = "encryption"))]
    #[test]
    fn encrypted_signed_wrong_signer_rejected() {
        use crate::encryption::public_key_from_private;
        use ed25519_dalek::SigningKey;

        let real_signer = SigningKey::from_bytes(&[0xa3; 32]);
        let other_signer = SigningKey::from_bytes(&[0xa4; 32]);
        let other_verifying = other_signer.verifying_key();

        let recipient_sk = [0x55u8; 32];
        let recipient_pk = public_key_from_private(&recipient_sk);

        let ephemeral_seed = [0x66u8; 32];

        let encrypted = module_to_encrypted_signed_wire_bytes(
            &make_minimal_module(),
            &real_signer,
            &recipient_pk,
            &ephemeral_seed,
        )
        .expect("encrypt+sign");

        // Try to verify with a different signer's key.
        let result =
            decrypt_encrypted_signed_to_signed_bytes(&encrypted, &[other_verifying], &recipient_sk);
        match result {
            Err(LoadError::InvalidSignature) => (),
            other => panic!(
                "expected InvalidSignature when verifying with wrong key, got: {:?}",
                other
            ),
        }
    }

    #[cfg(all(feature = "signatures", feature = "encryption"))]
    #[test]
    fn encrypted_signed_tampered_ciphertext_rejected() {
        use crate::encryption::public_key_from_private;
        use ed25519_dalek::SigningKey;

        let signer = SigningKey::from_bytes(&[0xa5; 32]);
        let verifying = signer.verifying_key();

        let recipient_sk = [0x77u8; 32];
        let recipient_pk = public_key_from_private(&recipient_sk);

        let ephemeral_seed = [0x88u8; 32];

        let mut encrypted = module_to_encrypted_signed_wire_bytes(
            &make_minimal_module(),
            &signer,
            &recipient_pk,
            &ephemeral_seed,
        )
        .expect("encrypt+sign");

        // Flip a byte in the ciphertext region. Then repair the
        // CRC so framing validation passes but the signature or
        // decryption fails as the substantive check.
        let body_start = encrypted_signed_header_length();
        encrypted[body_start + 4] ^= 0x01;
        let footer_start = encrypted.len() - WIRE_FORMAT_FOOTER_BYTES;
        let crc = crc32(&encrypted[..footer_start]);
        encrypted[footer_start..].copy_from_slice(&crc.to_le_bytes());

        let result =
            decrypt_encrypted_signed_to_signed_bytes(&encrypted, &[verifying], &recipient_sk);
        assert!(
            result.is_err(),
            "expected rejection on tampered ciphertext after CRC repair"
        );
    }

    #[test]
    fn header_requires_encryption_detects_flag() {
        let mut bytes: Vec<u8> = alloc::vec![0u8; 20];
        bytes[0..4].copy_from_slice(&BYTECODE_MAGIC[..]);
        assert!(!header_requires_encryption(&bytes));
        bytes[15] = FLAG_ENCRYPTED;
        assert!(header_requires_encryption(&bytes));
        bytes[15] = FLAG_REQUIRES_SIGNATURE | FLAG_ENCRYPTED;
        assert!(header_requires_encryption(&bytes));
        assert!(header_requires_signature(&bytes));
    }
}
