extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Runtime value in the Keleusma VM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    /// Unit value `()`.
    Unit,
    /// Boolean.
    Bool(bool),
    /// 64-bit signed integer.
    Int(i64),
    /// 64-bit floating-point number.
    Float(f64),
    /// Immutable static string referenced from the rodata region. Source-level
    /// string literals compile to this variant. Permitted to flow through the
    /// dialogue type B and across hot updates subject to the host attestation
    /// for rodata pointer validity. See R31, R32, R33 and B5.
    StaticStr(String),
    /// Dynamic string allocated in the arena heap. Produced by native function
    /// calls and runtime string operations. Lifetime bound to the arena and
    /// cleared at RESET. Cannot cross the yield boundary. Cannot reside in
    /// the data segment.
    DynStr(String),
    /// Tuple of values.
    Tuple(Vec<Value>),
    /// Fixed-size array of values.
    Array(Vec<Value>),
    /// Named struct with ordered fields.
    Struct {
        type_name: String,
        fields: Vec<(String, Value)>,
    },
    /// Enum variant with optional payload.
    Enum {
        type_name: String,
        variant: String,
        fields: Vec<Value>,
    },
    /// Option::None.
    None,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Unit, Value::Unit) | (Value::None, Value::None) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            // Static and dynamic strings compare equal if their contents match.
            // This relaxation follows the convention that the discipline is
            // about lifetime and provenance, not about value identity.
            (Value::StaticStr(a), Value::StaticStr(b))
            | (Value::DynStr(a), Value::DynStr(b))
            | (Value::StaticStr(a), Value::DynStr(b))
            | (Value::DynStr(a), Value::StaticStr(b)) => a == b,
            (Value::Tuple(a), Value::Tuple(b)) | (Value::Array(a), Value::Array(b)) => a == b,
            (
                Value::Struct {
                    type_name: na,
                    fields: fa,
                },
                Value::Struct {
                    type_name: nb,
                    fields: fb,
                },
            ) => na == nb && fa == fb,
            (
                Value::Enum {
                    type_name: na,
                    variant: va,
                    fields: fa,
                },
                Value::Enum {
                    type_name: nb,
                    variant: vb,
                    fields: fb,
                },
            ) => na == nb && va == vb && fa == fb,
            _ => false,
        }
    }
}

impl Value {
    /// Return a human-readable type name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Unit => "Unit",
            Value::Bool(_) => "Bool",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::StaticStr(_) => "StaticStr",
            Value::DynStr(_) => "DynStr",
            Value::Tuple(_) => "Tuple",
            Value::Array(_) => "Array",
            Value::Struct { .. } => "Struct",
            Value::Enum { .. } => "Enum",
            Value::None => "None",
        }
    }

    /// Borrow the underlying UTF-8 contents of either string variant.
    ///
    /// Returns `None` if the value is not a string. Used at sites that read
    /// string contents without caring about static-versus-dynamic provenance,
    /// such as type-name lookups in the constant pool and string-consuming
    /// natives like `length` and `println`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::StaticStr(s) | Value::DynStr(s) => Some(s.as_str()),
            _ => Option::None,
        }
    }

    /// Returns true if the value is a dynamic string or transitively contains
    /// a dynamic string. Used to enforce the cross-yield prohibition (R31).
    pub fn contains_dynstr(&self) -> bool {
        match self {
            Value::DynStr(_) => true,
            Value::Tuple(items) | Value::Array(items) => items.iter().any(Value::contains_dynstr),
            Value::Struct { fields, .. } => fields.iter().any(|(_, v)| v.contains_dynstr()),
            Value::Enum { fields, .. } => fields.iter().any(Value::contains_dynstr),
            _ => false,
        }
    }
}

/// Classification of a compiled function chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockType {
    /// Atomic total function (`fn`). No yields, no streaming.
    Func,
    /// Non-atomic total function (`yield fn`). Must contain at least one Yield.
    Reentrant,
    /// Productive divergent function (`loop fn`). Contains Stream/Reset and Yield.
    Stream,
}

/// A bytecode instruction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    /// Push a constant from the chunk's constant pool.
    Const(u16),
    /// Push unit value `()`.
    PushUnit,
    /// Push `true`.
    PushTrue,
    /// Push `false`.
    PushFalse,

    /// Push local variable by slot index.
    GetLocal(u16),
    /// Pop and store to local variable slot.
    SetLocal(u16),

    /// Push data segment slot value onto stack.
    GetData(u16),
    /// Pop value and store into data segment slot.
    SetData(u16),

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
    If(u32),
    /// Skip to target (matching EndIf). Reached when then-block falls through.
    Else(u32),
    /// Block delimiter for If/Else. No-op at runtime.
    EndIf,

    /// Begin loop block. Target is past EndLoop (used by Break/BreakIf).
    Loop(u32),
    /// Back-edge to instruction after matching Loop.
    EndLoop(u32),
    /// Unconditional forward jump past enclosing EndLoop.
    Break(u32),
    /// Pop bool; if true, forward jump past enclosing EndLoop.
    BreakIf(u32),

    // -- Streaming --
    /// Stream block entry marker. No-op at runtime.
    Stream,
    /// Clear arena, return VmState::Reset to host.
    Reset,

    // -- Functions --
    /// Call compiled function by chunk index with N arguments.
    Call(u16, u8),
    /// Call native function by registry index with N arguments.
    CallNative(u16, u8),
    /// Return from the current function.
    Return,

    /// Yield: pop output value, suspend. On resume, input is pushed.
    Yield,

    /// Pop and discard top of stack.
    Pop,
    /// Duplicate top of stack.
    Dup,

    /// Build struct from template. Pop field_count values in field order.
    NewStruct(u16),
    /// Build enum variant. Pop arg_count values.
    NewEnum(u16, u16, u8),
    /// Build array from top N stack values.
    NewArray(u16),
    /// Build tuple from top N stack values.
    NewTuple(u8),
    /// Wrap top of stack in Some (identity for value representation).
    WrapSome,
    /// Push None.
    PushNone,

    /// Pop struct, push field value by name (const pool index).
    GetField(u16),
    /// Pop index (Int), pop array, push element.
    GetIndex,
    /// Pop tuple, push element at literal index.
    GetTupleField(u8),
    /// Pop enum, push field at literal index.
    GetEnumField(u8),
    /// Pop composite value, push its length as Int.
    Len,

    /// Peek at TOS: push true if matching enum type and variant, false otherwise.
    IsEnum(u16, u16),
    /// Peek at TOS: push true if matching struct type, false otherwise.
    IsStruct(u16),

    /// Cast i64 to f64.
    IntToFloat,
    /// Cast f64 to i64 (truncation).
    FloatToInt,

    /// Halt execution with a runtime error.
    Trap(u16),
}

/// Size in bytes of one operand-stack slot, namely the size of `Value` on
/// the modern 64-bit target. The actual `core::mem::size_of::<Value>()` is
/// implementation-dependent and may include padding to align variant
/// discriminators. For WCMU analysis, the conservative upper bound is
/// chosen so that the analysis remains sound even if the runtime
/// representation grows.
///
/// On the V0.0 cycle target (R33), this constant is 32 bytes. Future work
/// under B10 may parameterize this by target.
pub const VALUE_SLOT_SIZE_BYTES: u32 = 32;

impl Op {
    /// Return the relative integer cost of this instruction.
    ///
    /// Costs are unitless relative weights, not cycle counts. Higher values
    /// indicate more expensive operations. The scale is chosen so that
    /// simple data movement operations cost 1 and complex operations cost
    /// proportionally more. These values are preliminary and subject to
    /// refinement as the instruction set stabilizes.
    pub fn cost(&self) -> u32 {
        match self {
            // Data movement: minimal cost.
            Op::Const(_)
            | Op::PushUnit
            | Op::PushTrue
            | Op::PushFalse
            | Op::GetLocal(_)
            | Op::SetLocal(_)
            | Op::GetData(_)
            | Op::SetData(_)
            | Op::Pop
            | Op::Dup
            | Op::PushNone
            | Op::WrapSome
            | Op::Not => 1,

            // Control flow markers: minimal overhead.
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

            // Simple arithmetic and comparisons.
            Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Neg
            | Op::CmpEq
            | Op::CmpNe
            | Op::CmpLt
            | Op::CmpGt
            | Op::CmpLe
            | Op::CmpGe
            | Op::GetIndex
            | Op::GetTupleField(_)
            | Op::GetEnumField(_)
            | Op::Len
            | Op::IntToFloat
            | Op::FloatToInt
            | Op::Return => 2,

            // Division, field lookup, type checks (string comparison).
            Op::Div | Op::Mod | Op::GetField(_) | Op::IsEnum(_, _) | Op::IsStruct(_) => 3,

            // Composite value construction.
            Op::NewStruct(_) | Op::NewEnum(_, _, _) | Op::NewArray(_) | Op::NewTuple(_) => 5,

            // Function calls.
            Op::Call(_, _) | Op::CallNative(_, _) => 10,
        }
    }

    /// Number of operand-stack slots pushed by this instruction.
    ///
    /// This is the maximum the operand stack can grow during execution of
    /// this single instruction relative to its starting depth. Used by the
    /// WCMU analysis to compute peak stack consumption.
    pub fn stack_growth(&self) -> u32 {
        match self {
            Op::Const(_)
            | Op::PushUnit
            | Op::PushTrue
            | Op::PushFalse
            | Op::GetLocal(_)
            | Op::GetData(_)
            | Op::Dup
            | Op::PushNone => 1,

            Op::WrapSome | Op::Not | Op::Neg => 0,

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

            Op::SetLocal(_) | Op::SetData(_) | Op::Pop => 0,

            Op::If(_) | Op::BreakIf(_) => 0,
            Op::Else(_) | Op::EndIf | Op::Loop(_) | Op::EndLoop(_) | Op::Break(_) => 0,
            Op::Stream | Op::Reset => 0,
            Op::Yield => 0,

            Op::Call(_, _) | Op::CallNative(_, _) => 1,
            Op::Return => 0,

            Op::NewStruct(_) | Op::NewEnum(_, _, _) | Op::NewArray(_) | Op::NewTuple(_) => 1,

            Op::GetField(_)
            | Op::GetIndex
            | Op::GetTupleField(_)
            | Op::GetEnumField(_)
            | Op::Len => 0,

            Op::IsEnum(_, _) | Op::IsStruct(_) => 0,

            Op::IntToFloat | Op::FloatToInt => 0,

            Op::Trap(_) => 0,
        }
    }

    /// Number of operand-stack slots popped by this instruction.
    pub fn stack_shrink(&self) -> u32 {
        match self {
            Op::Const(_)
            | Op::PushUnit
            | Op::PushTrue
            | Op::PushFalse
            | Op::GetLocal(_)
            | Op::GetData(_)
            | Op::Dup
            | Op::PushNone => 0,

            Op::WrapSome | Op::Not | Op::Neg => 0,

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

            Op::SetLocal(_) | Op::SetData(_) | Op::Pop => 1,

            Op::If(_) | Op::BreakIf(_) => 1,
            Op::Else(_) | Op::EndIf | Op::Loop(_) | Op::EndLoop(_) | Op::Break(_) => 0,
            Op::Stream | Op::Reset => 0,
            Op::Yield => 1,

            Op::Call(_, n) | Op::CallNative(_, n) => *n as u32,
            Op::Return => 0,

            Op::NewStruct(_) => 0,
            Op::NewEnum(_, _, n) => *n as u32,
            Op::NewArray(n) => *n as u32,
            Op::NewTuple(n) => *n as u32,

            Op::GetField(_) | Op::GetIndex | Op::GetTupleField(_) | Op::GetEnumField(_) => 1,
            Op::Len => 0,

            Op::IsEnum(_, _) | Op::IsStruct(_) => 0,

            Op::IntToFloat | Op::FloatToInt => 0,

            Op::Trap(_) => 0,
        }
    }

    /// Bytes allocated to the arena heap by this instruction, ignoring
    /// transitive allocations through called functions.
    ///
    /// For composite-construction instructions, the size is the count of
    /// stored field slots times `VALUE_SLOT_SIZE_BYTES`. For `NewStruct`,
    /// the field count comes from the chunk's struct templates and so is
    /// looked up using the provided `chunk` reference.
    ///
    /// Calls and native calls are reported as zero local heap. The
    /// transitive heap contribution of a `Call` is the WCMU of the called
    /// function and is computed at the analysis level by recursive
    /// traversal of the call graph. The heap contribution of a
    /// `CallNative` comes from the host's WCMU attestation, recorded
    /// against the native function entry.
    pub fn heap_alloc(&self, chunk: &Chunk) -> u32 {
        match self {
            Op::NewStruct(template_idx) => {
                let idx = *template_idx as usize;
                let field_count = chunk
                    .struct_templates
                    .get(idx)
                    .map_or(0, |t| t.field_names.len() as u32);
                field_count * VALUE_SLOT_SIZE_BYTES
            }
            Op::NewEnum(_, _, n) => *n as u32 * VALUE_SLOT_SIZE_BYTES,
            Op::NewArray(n) => *n as u32 * VALUE_SLOT_SIZE_BYTES,
            Op::NewTuple(n) => *n as u32 * VALUE_SLOT_SIZE_BYTES,
            _ => 0,
        }
    }
}

/// Template for struct construction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructTemplate {
    /// Struct type name.
    pub type_name: String,
    /// Field names in order.
    pub field_names: Vec<String>,
}

/// A named slot in the data segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSlot {
    /// Slot name (for host initialization and debugging).
    pub name: String,
}

/// Data segment layout declaration.
///
/// Defines the fixed-size, fixed-layout set of persistent values that
/// survive across RESET boundaries. The host initializes data slots
/// before execution begins. Scripts read and write slots by index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataLayout {
    /// Named slots in declaration order. Slot index corresponds to
    /// the `GetData`/`SetData` operand.
    pub slots: Vec<DataSlot>,
}

/// A compiled function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// Function name (for debugging and lookup).
    pub name: String,
    /// Bytecode instructions.
    pub ops: Vec<Op>,
    /// Constant pool.
    pub constants: Vec<Value>,
    /// Struct field layout templates.
    pub struct_templates: Vec<StructTemplate>,
    /// Total local variable slots (including parameters).
    pub local_count: u16,
    /// Number of parameters.
    pub param_count: u8,
    /// Block type classification for structural verification.
    pub block_type: BlockType,
}

/// A compiled Keleusma module.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

/// Magic prefix identifying serialized Keleusma bytecode (`KELE`).
pub const BYTECODE_MAGIC: [u8; 4] = *b"KELE";

/// Wire format version for serialized bytecode. Bytecode produced under a
/// different version is rejected at load time.
pub const BYTECODE_VERSION: u16 = 1;

/// Header length in bytes (4-byte magic plus 2-byte little-endian version).
const HEADER_LEN: usize = 6;

/// Footer length in bytes (4-byte little-endian CRC-32).
const FOOTER_LEN: usize = 4;

/// Reflected polynomial for the standard CRC-32 (IEEE 802.3, gzip, PNG,
/// ZIP). Reflected form of 0x04C11DB7. Paired with init 0xFFFFFFFF,
/// refin/refout true, and xor-out 0xFFFFFFFF.
const CRC32_POLY: u32 = 0xEDB88320;

/// Residue constant for the CRC-32 parameters above. After computing the
/// CRC over any byte sequence followed by the little-endian encoding of
/// that sequence's CRC, the result equals this constant. The verifier
/// exploits this property to check integrity in a single pass without
/// separating the CRC field from the data, satisfying the algebraic
/// self-inclusion contract recorded in R39.
const CRC32_RESIDUE: u32 = 0x2144DF1C;

/// Compute the standard CRC-32 of `bytes`.
///
/// Bit-by-bit implementation. Adequate for bytecode-sized inputs in the
/// kilobyte to megabyte range. The verifier runs this once over the
/// entire serialized form including the appended CRC and checks against
/// [`CRC32_RESIDUE`]. Visibility is `pub(crate)` for use by integrity
/// tests that need to construct bytecode with a hand-tweaked field and
/// a recomputed checksum.
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
    /// The buffer was shorter than the required header plus footer.
    Truncated,
    /// The bytecode version is not supported by this runtime.
    UnsupportedVersion {
        /// Version recorded in the bytecode header.
        got: u16,
        /// Version the runtime supports.
        expected: u16,
    },
    /// The CRC-32 trailer did not satisfy the algebraic self-inclusion
    /// residue. The bytecode is corrupted or was produced by a different
    /// CRC implementation.
    BadChecksum,
    /// The body could not be encoded or decoded.
    Codec(String),
}

impl core::fmt::Display for LoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoadError::BadMagic => f.write_str("bytecode header missing magic 'KELE'"),
            LoadError::Truncated => {
                f.write_str("bytecode shorter than required header plus footer")
            }
            LoadError::UnsupportedVersion { got, expected } => {
                write!(
                    f,
                    "bytecode version {} not supported, expected {}",
                    got, expected
                )
            }
            LoadError::BadChecksum => f.write_str("bytecode CRC-32 residue check failed"),
            LoadError::Codec(msg) => write!(f, "bytecode codec error: {}", msg),
        }
    }
}

impl core::error::Error for LoadError {}

impl Module {
    /// Serialize the module to a self-describing byte vector.
    ///
    /// The output begins with [`BYTECODE_MAGIC`] followed by
    /// [`BYTECODE_VERSION`] in little-endian order, then the module body
    /// in postcard wire format, then a four-byte little-endian CRC-32
    /// trailer. The CRC covers the magic, version, and body. The
    /// algebraic self-inclusion property of CRC-32 means that running
    /// the CRC over the entire serialized form including the trailer
    /// produces a fixed residue constant, which the verifier checks in
    /// a single pass without separating the trailer from the data.
    ///
    /// Returns [`LoadError::Codec`] if postcard rejects any field. The
    /// `Module` type is composed entirely of types that postcard supports,
    /// so encode failures are not expected in practice and indicate
    /// corruption of the runtime data.
    pub fn to_bytes(&self) -> Result<Vec<u8>, LoadError> {
        use alloc::format;
        let body = postcard::to_allocvec(self)
            .map_err(|e| LoadError::Codec(format!("encode failed: {}", e)))?;
        let mut buf = Vec::with_capacity(HEADER_LEN + body.len() + FOOTER_LEN);
        buf.extend_from_slice(&BYTECODE_MAGIC);
        buf.extend_from_slice(&BYTECODE_VERSION.to_le_bytes());
        buf.extend_from_slice(&body);
        let crc = crc32(&buf);
        buf.extend_from_slice(&crc.to_le_bytes());
        Ok(buf)
    }

    /// Deserialize a module from a self-describing byte slice.
    ///
    /// Validates the magic, the CRC-32 trailer through the residue
    /// property, and the version header in that order, then deserializes
    /// the postcard body. The input may originate from any addressable
    /// byte slice including in-memory buffers, file-loaded buffers, or
    /// `&'static [u8]` data placed in `.rodata`.
    ///
    /// The CRC is checked before the version because a corrupted byte
    /// in the version field would otherwise be reported as
    /// `UnsupportedVersion` rather than the more accurate `BadChecksum`.
    ///
    /// Does not run structural verification or resource bounds checks.
    /// Pass the result to [`crate::vm::Vm::new`] for full verification or
    /// to [`crate::vm::Vm::new_unchecked`] for trust-based skipping of
    /// the bounds checks.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, LoadError> {
        use alloc::format;
        if bytes.len() < HEADER_LEN + FOOTER_LEN {
            return Err(LoadError::Truncated);
        }
        if bytes[0..4] != BYTECODE_MAGIC {
            return Err(LoadError::BadMagic);
        }
        // CRC residue check covers the entire byte slice including the
        // trailer. A correctly produced bytecode produces CRC32_RESIDUE.
        if crc32(bytes) != CRC32_RESIDUE {
            return Err(LoadError::BadChecksum);
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != BYTECODE_VERSION {
            return Err(LoadError::UnsupportedVersion {
                got: version,
                expected: BYTECODE_VERSION,
            });
        }
        let body = &bytes[HEADER_LEN..bytes.len() - FOOTER_LEN];
        postcard::from_bytes(body).map_err(|e| LoadError::Codec(format!("decode failed: {}", e)))
    }
}
