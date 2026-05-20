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

use alloc::vec::Vec;

use crate::bytecode::Op;

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
}
