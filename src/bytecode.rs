extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use rkyv::{Archive, Deserialize, Serialize};

/// Runtime value in the Keleusma VM.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
#[rkyv(
    serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext, <__C as rkyv::rancor::Fallible>::Error: rkyv::rancor::Source))
)]
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
    Tuple(#[rkyv(omit_bounds)] Vec<Value>),
    /// Fixed-size array of values.
    Array(#[rkyv(omit_bounds)] Vec<Value>),
    /// Named struct with ordered fields.
    Struct {
        type_name: String,
        #[rkyv(omit_bounds)]
        fields: Vec<(String, Value)>,
    },
    /// Enum variant with optional payload.
    Enum {
        type_name: String,
        variant: String,
        #[rkyv(omit_bounds)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum BlockType {
    /// Atomic total function (`fn`). No yields, no streaming.
    Func,
    /// Non-atomic total function (`yield fn`). Must contain at least one Yield.
    Reentrant,
    /// Productive divergent function (`loop fn`). Contains Stream/Reset and Yield.
    Stream,
}

/// A bytecode instruction.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
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
}

/// Magic prefix identifying serialized Keleusma bytecode (`KELE`).
pub const BYTECODE_MAGIC: [u8; 4] = *b"KELE";

/// Wire format version for serialized bytecode. Bytecode produced under a
/// different version is rejected at load time.
pub const BYTECODE_VERSION: u16 = 4;

/// Word size in bits assumed by this runtime build, encoded as the
/// base-2 exponent. Actual width in bits is `1 << RUNTIME_WORD_BITS_LOG2`.
/// The current Keleusma runtime uses 64-bit words (i64 and f64), so the
/// exponent is 6.
pub const RUNTIME_WORD_BITS_LOG2: u8 = 6;

/// Address size in bits assumed by this runtime build, encoded as the
/// base-2 exponent. Actual width in bits is
/// `1 << RUNTIME_ADDRESS_BITS_LOG2`. The current Keleusma runtime
/// targets 64-bit address spaces, so the exponent is 6.
pub const RUNTIME_ADDRESS_BITS_LOG2: u8 = 6;

/// Header length in bytes. The fields are
///
/// - bytes 0..4: magic (`KELE`)
/// - bytes 4..6: version (u16 little-endian)
/// - bytes 6..10: total framing length (u32 little-endian, includes
///   header and CRC trailer)
/// - bytes 10..11: word_bits_log2 (u8). Actual width is `1 << value`.
/// - bytes 11..12: addr_bits_log2 (u8). Actual width is `1 << value`.
/// - bytes 12..16: reserved (zero). Pads the header so the rkyv body
///   begins at an 8-byte-aligned offset within the buffer when the
///   buffer base is itself 8-byte-aligned. Required for in-place
///   access through `rkyv::access`.
const HEADER_LEN: usize = 16;

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
            LoadError::BadChecksum => f.write_str("bytecode CRC-32 residue check failed"),
            LoadError::Codec(msg) => write!(f, "bytecode codec error: {}", msg),
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
        use alloc::format;
        let body = rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map_err(|e| LoadError::Codec(format!("encode failed: {}", e)))?;
        let total_len = (HEADER_LEN + body.len() + FOOTER_LEN) as u32;
        let mut buf = Vec::with_capacity(total_len as usize);
        buf.extend_from_slice(&BYTECODE_MAGIC);
        buf.extend_from_slice(&BYTECODE_VERSION.to_le_bytes());
        buf.extend_from_slice(&total_len.to_le_bytes());
        buf.push(self.word_bits_log2);
        buf.push(self.addr_bits_log2);
        // Reserved bytes pad the header to 16 so the rkyv body begins
        // at an 8-byte-aligned offset within the buffer.
        buf.extend_from_slice(&[0u8; 4]);
        buf.extend_from_slice(&body);
        let crc = crc32(&buf);
        buf.extend_from_slice(&crc.to_le_bytes());
        Ok(buf)
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
        use alloc::format;
        if bytes.len() < HEADER_LEN + FOOTER_LEN {
            return Err(LoadError::Truncated);
        }
        if bytes[0..4] != BYTECODE_MAGIC {
            return Err(LoadError::BadMagic);
        }
        // Read the recorded total length and validate that the slice has
        // at least that many bytes and that the recorded length is at
        // least the minimum framing size. Trailing bytes after the
        // recorded length are ignored.
        let length = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
        if length < HEADER_LEN + FOOTER_LEN || length > bytes.len() {
            return Err(LoadError::Truncated);
        }
        let bytes = &bytes[..length];
        // CRC residue check covers the entire truncated slice including
        // the trailer. A correctly produced bytecode produces
        // CRC32_RESIDUE.
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
        let word_bits_log2 = bytes[10];
        if word_bits_log2 > RUNTIME_WORD_BITS_LOG2 {
            return Err(LoadError::WordSizeMismatch {
                got: word_bits_log2,
                max_supported: RUNTIME_WORD_BITS_LOG2,
            });
        }
        let addr_bits_log2 = bytes[11];
        if addr_bits_log2 > RUNTIME_ADDRESS_BITS_LOG2 {
            return Err(LoadError::AddressSizeMismatch {
                got: addr_bits_log2,
                max_supported: RUNTIME_ADDRESS_BITS_LOG2,
            });
        }
        let body = &bytes[HEADER_LEN..length - FOOTER_LEN];
        // rkyv requires the body buffer to be 8-byte aligned. Copy
        // into an AlignedVec to satisfy this for arbitrary host slices.
        // For hosts that supply an aligned buffer directly, see
        // [`Module::view_bytes`] which skips the copy.
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(body.len());
        aligned.extend_from_slice(body);
        rkyv::from_bytes::<Module, rkyv::rancor::Error>(&aligned)
            .map_err(|e| LoadError::Codec(format!("decode failed: {}", e)))
    }

    /// Validate framing and return a borrowed archived view of the module.
    ///
    /// Performs the same framing checks as [`Module::from_bytes`] (magic,
    /// length, CRC residue, version, word size, address size) and then
    /// runs `rkyv::access` on the body to obtain a `&'a ArchivedModule`
    /// without deserialization.
    ///
    /// The body must be 8-byte aligned within the slice. Because
    /// [`HEADER_LEN`] is 16, the body is 8-byte aligned within the slice
    /// when the slice base itself is 8-byte aligned. Hosts that compute
    /// or load bytecode into an `rkyv::util::AlignedVec` or a static
    /// buffer with `#[repr(align(8))]` satisfy this requirement.
    /// Bytecode placed by the linker into a section that aligns to at
    /// least 8 bytes also satisfies it.
    ///
    /// Returns `LoadError::Codec` with an alignment message when the
    /// body is not aligned, or when the rkyv structural validator
    /// rejects the body. Returns the other `LoadError` variants for
    /// header validation failures.
    pub fn access_bytes(bytes: &[u8]) -> Result<&ArchivedModule, LoadError> {
        use alloc::format;
        if bytes.len() < HEADER_LEN + FOOTER_LEN {
            return Err(LoadError::Truncated);
        }
        if bytes[0..4] != BYTECODE_MAGIC {
            return Err(LoadError::BadMagic);
        }
        let length = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
        if length < HEADER_LEN + FOOTER_LEN || length > bytes.len() {
            return Err(LoadError::Truncated);
        }
        let bytes = &bytes[..length];
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
        let word_bits_log2 = bytes[10];
        if word_bits_log2 > RUNTIME_WORD_BITS_LOG2 {
            return Err(LoadError::WordSizeMismatch {
                got: word_bits_log2,
                max_supported: RUNTIME_WORD_BITS_LOG2,
            });
        }
        let addr_bits_log2 = bytes[11];
        if addr_bits_log2 > RUNTIME_ADDRESS_BITS_LOG2 {
            return Err(LoadError::AddressSizeMismatch {
                got: addr_bits_log2,
                max_supported: RUNTIME_ADDRESS_BITS_LOG2,
            });
        }
        let body = &bytes[HEADER_LEN..length - FOOTER_LEN];
        if !(body.as_ptr() as usize).is_multiple_of(8) {
            return Err(LoadError::Codec(format!(
                "body not 8-byte aligned (slice base 0x{:x}); use Module::from_bytes for unaligned input",
                bytes.as_ptr() as usize
            )));
        }
        rkyv::access::<ArchivedModule, rkyv::rancor::Error>(body)
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
        use alloc::format;
        let archived = Self::access_bytes(bytes)?;
        rkyv::deserialize::<Module, rkyv::rancor::Error>(archived)
            .map_err(|e| LoadError::Codec(format!("deserialize failed: {}", e)))
    }
}

/// Sign-extending mask for narrower-than-runtime integer arithmetic.
///
/// When a bytecode declares a word size narrower than the runtime
/// supports, the VM applies this mask after each integer arithmetic
/// op so that overflow points match the bytecode's declared width.
/// For `word_bits_log2 >= 6` the function is the identity, since the
/// runtime's native i64 already matches or exceeds the declared width.
pub(crate) fn truncate_int(value: i64, word_bits_log2: u8) -> i64 {
    if word_bits_log2 >= 6 {
        return value;
    }
    let bits = 1u32 << word_bits_log2;
    let shift = 64 - bits;
    (value << shift) >> shift
}
