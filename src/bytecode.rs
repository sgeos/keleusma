extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// Runtime value in the Keleusma VM.
#[derive(Debug, Clone)]
pub enum Value {
    /// Unit value `()`.
    Unit,
    /// Boolean.
    Bool(bool),
    /// 64-bit signed integer.
    Int(i64),
    /// 64-bit floating-point number.
    Float(f64),
    /// UTF-8 string.
    Str(String),
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
            (Value::Str(a), Value::Str(b)) => a == b,
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
            Value::Str(_) => "String",
            Value::Tuple(_) => "Tuple",
            Value::Array(_) => "Array",
            Value::Struct { .. } => "Struct",
            Value::Enum { .. } => "Enum",
            Value::None => "None",
        }
    }
}

/// Classification of a compiled function chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    /// Atomic total function (`fn`). No yields, no streaming.
    Func,
    /// Non-atomic total function (`yield fn`). Must contain at least one Yield.
    Reentrant,
    /// Productive divergent function (`loop fn`). Contains Stream/Reset and Yield.
    Stream,
}

/// A bytecode instruction.
#[derive(Debug, Clone, PartialEq)]
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
}

/// Template for struct construction.
#[derive(Debug, Clone)]
pub struct StructTemplate {
    /// Struct type name.
    pub type_name: String,
    /// Field names in order.
    pub field_names: Vec<String>,
}

/// A named slot in the data segment.
#[derive(Debug, Clone)]
pub struct DataSlot {
    /// Slot name (for host initialization and debugging).
    pub name: String,
}

/// Data segment layout declaration.
///
/// Defines the fixed-size, fixed-layout set of persistent values that
/// survive across RESET boundaries. The host initializes data slots
/// before execution begins. Scripts read and write slots by index.
#[derive(Debug, Clone)]
pub struct DataLayout {
    /// Named slots in declaration order. Slot index corresponds to
    /// the `GetData`/`SetData` operand.
    pub slots: Vec<DataSlot>,
}

/// A compiled function.
#[derive(Debug, Clone)]
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
}
