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

/// Width of an opcode record in bytes.
pub const OPCODE_RECORD_BYTES: usize = 4;

/// Width of an operand pool entry in bytes.
pub const OPERAND_POOL_ENTRY_BYTES: usize = 8;

/// Width of the framing header in bytes.
pub const WIRE_FORMAT_HEADER_BYTES: usize = 64;

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
        Op::GetIndex => 39,
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
        Op::CheckedMul => 56,
        Op::CheckedNeg => 57,
        Op::CheckedDiv => 58,
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
        | Op::GetIndex
        | Op::Len
        | Op::IntToFloat
        | Op::FloatToInt
        | Op::WordToByte
        | Op::ByteToWord
        | Op::CheckedAdd
        | Op::CheckedSub
        | Op::CheckedMul
        | Op::CheckedNeg
        | Op::CheckedDiv
        | Op::CheckedMod
        | Op::BitAnd
        | Op::BitOr
        | Op::BitXor
        | Op::Shl
        | Op::Shr => [0, 0, 0],

        // `u8` operand carried inline in byte one.
        Op::NewTuple(n)
        | Op::GetTupleField(n)
        | Op::GetEnumField(n)
        | Op::WordToFixed(n)
        | Op::FixedToWord(n)
        | Op::FixedMul(n)
        | Op::FixedDiv(n)
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
        | Op::GetField(v)
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
        38 => Op::GetField(record.operand_u16()),
        39 => Op::GetIndex,
        40 => Op::GetTupleField(record.operand_u8()),
        41 => Op::GetEnumField(record.operand_u8()),
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
        56 => Op::CheckedMul,
        57 => Op::CheckedNeg,
        58 => Op::CheckedDiv,
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
}

/// Wire-format auxiliary body. Mirrors [`Module`] but carries
/// [`WireChunk`] metadata (ops live in the opcode stream
/// section).
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct WireAuxBody {
    pub chunks: Vec<WireChunk>,
    pub native_names: Vec<String>,
    pub entry_point: Option<usize>,
    pub data_layout: Option<DataLayout>,
    pub word_bits_log2: u8,
    pub addr_bits_log2: u8,
    pub float_bits_log2: u8,
    pub wcet_cycles: u32,
    pub wcmu_bytes: u32,
    pub flags: u8,
    pub shared_data_bytes: u32,
    pub private_data_bytes: u32,
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
    pub word_bits_log2: u8,
    pub addr_bits_log2: u8,
    pub float_bits_log2: u8,
    pub flags: u8,
    pub wcet_cycles: u32,
    pub wcmu_bytes: u32,
    pub shared_data_bytes: u32,
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
    if header_length != WIRE_FORMAT_HEADER_BYTES {
        return Err(LoadError::Codec(format!(
            "wire format header length {} does not match the expected {}",
            header_length, WIRE_FORMAT_HEADER_BYTES
        )));
    }
    let total_length = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    if total_length < WIRE_FORMAT_HEADER_BYTES + WIRE_FORMAT_FOOTER_BYTES
        || total_length > bytes.len()
    {
        return Err(LoadError::Truncated);
    }
    let bytes = &bytes[..total_length];
    if crc32(bytes) != WIRE_FORMAT_CRC32_RESIDUE {
        return Err(LoadError::BadChecksum);
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
        off >= WIRE_FORMAT_HEADER_BYTES && off.checked_add(len).is_some_and(|end| end <= body_end)
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

    // Compute section offsets. The opcode stream begins
    // immediately after the 64-byte framing header. The operand
    // pool begins at the next 8-byte aligned offset, which is
    // already aligned because the opcode stream is a multiple
    // of 4 bytes per record and the section count is a multiple
    // of 8 records (records are 4 bytes; 8 records = 32 bytes
    // align). Conservatively pad to 8-byte alignment.
    let opcode_stream_offset = WIRE_FORMAT_HEADER_BYTES as u32;
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
    // Framing header.
    buf.extend_from_slice(&BYTECODE_MAGIC);
    buf.extend_from_slice(&BYTECODE_VERSION.to_le_bytes());
    buf.extend_from_slice(&(WIRE_FORMAT_HEADER_BYTES as u16).to_le_bytes());
    buf.extend_from_slice(&total_length.to_le_bytes());
    buf.push(module.word_bits_log2);
    buf.push(module.addr_bits_log2);
    buf.push(module.float_bits_log2);
    buf.push(module.flags);
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
    buf.extend_from_slice(&[0u8; 4]); // reserved
    buf.extend_from_slice(&[0u8; 4]); // reserved
    debug_assert_eq!(buf.len(), WIRE_FORMAT_HEADER_BYTES);

    // Opcode stream + alignment padding.
    buf.extend_from_slice(&opcode_stream);
    buf.resize(buf.len() + opcode_padding as usize, 0);

    // Operand pool + alignment padding.
    buf.extend_from_slice(&operand_pool_bytes);
    buf.resize(buf.len() + pool_padding as usize, 0);

    // Auxiliary body.
    buf.extend_from_slice(&aux_bytes);

    // CRC trailer over header + sections.
    let crc = crc32(&buf);
    buf.extend_from_slice(&crc.to_le_bytes());
    debug_assert_eq!(buf.len(), total_length as usize);
    Ok(buf)
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
    if header_length != WIRE_FORMAT_HEADER_BYTES {
        return Err(LoadError::Codec(format!(
            "wire format header length {} does not match the expected {}",
            header_length, WIRE_FORMAT_HEADER_BYTES
        )));
    }
    let total_length = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    if total_length < WIRE_FORMAT_HEADER_BYTES + WIRE_FORMAT_FOOTER_BYTES
        || total_length > bytes.len()
    {
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
    // Section bounds sanity. Each section fits entirely within
    // the frame (after the header, before the CRC trailer).
    let body_end = total_length - WIRE_FORMAT_FOOTER_BYTES;
    let in_body = |off: usize, len: usize| -> bool {
        off >= WIRE_FORMAT_HEADER_BYTES && off.checked_add(len).is_some_and(|end| end <= body_end)
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
        chunks.push(Chunk {
            name: wc.name.clone(),
            ops,
            constants: wc.constants.clone(),
            struct_templates: wc.struct_templates.clone(),
            local_count: wc.local_count,
            param_count: wc.param_count,
            block_type: wc.block_type,
            param_types: wc.param_types.clone(),
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
            (Op::GetField(0), 38),
            (Op::GetIndex, 39),
            (Op::GetTupleField(0), 40),
            (Op::GetEnumField(0), 41),
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
            (Op::CheckedMul, 56),
            (Op::CheckedNeg, 57),
            (Op::CheckedDiv, 58),
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
            Op::GetIndex,
            Op::Len,
            Op::IntToFloat,
            Op::FloatToInt,
            Op::WordToByte,
            Op::ByteToWord,
            Op::CheckedAdd,
            Op::CheckedSub,
            Op::CheckedMul,
            Op::CheckedNeg,
            Op::CheckedDiv,
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
            Op::GetTupleField(7),
            Op::GetEnumField(3),
            Op::WordToFixed(32),
            Op::FixedToWord(16),
            Op::FixedMul(8),
            Op::FixedDiv(4),
            Op::PushImmediate(5),
            Op::PopN(2),
        ] {
            roundtrip(op);
        }
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
            Op::GetField(1000),
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
}
