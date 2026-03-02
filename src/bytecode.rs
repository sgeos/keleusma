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
                Value::Struct { type_name: na, fields: fa },
                Value::Struct { type_name: nb, fields: fb },
            ) => na == nb && fa == fb,
            (
                Value::Enum { type_name: na, variant: va, fields: fa },
                Value::Enum { type_name: nb, variant: vb, fields: fb },
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

    /// Unconditional jump to instruction index.
    Jump(u32),
    /// Pop top of stack; jump if false.
    JumpIfFalse(u32),

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

    /// Peek at TOS: continue if matching enum variant, else jump.
    TestEnum(u16, u16, u32),
    /// Peek at TOS: continue if matching struct type, else jump.
    TestStruct(u16, u32),

    /// Cast i64 to f64.
    IntToFloat,
    /// Cast f64 to i64 (truncation).
    FloatToInt,

    /// Halt execution with a runtime error.
    Trap(u16),
}

/// Template for struct construction.
#[derive(Debug, Clone)]
pub struct StructTemplate {
    /// Struct type name.
    pub type_name: String,
    /// Field names in order.
    pub field_names: Vec<String>,
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
    /// Whether this is a `loop` category function.
    pub is_loop: bool,
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
}
