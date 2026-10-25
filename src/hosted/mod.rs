//! The hosted runtime: a bytecode interpreter for Keleusma written in Keleusma (Order 4, roadmap
//! Workstream D), and the Rust driver that admits a module, marshals it and runs it.
//!
//! The interpreter is `interp.kel` beside this file. It runs on the Rust virtual machine as a
//! `loop` that executes at most [`FUEL`] interpreted instructions per tick, so its own per-tick
//! time and memory bounds are constants. This driver decides admission before anything runs. A
//! module is admitted only when the Rust verifier accepted it, every instruction is in the
//! supported set, every constant is a supported scalar, and its tables fit the interpreter's
//! capacities. See `docs/decisions/ORDER4_HOSTED_RUNTIME_PLAN.md` for the rulings, the design
//! and the bound composition.
//!
//! **Trust boundary.** The interpreter treats its input as already verified. Admission is
//! host-side in the first pass. A [`Outcome::Fault`] therefore names a defect in admission or in
//! the interpreter, not a behaviour of the interpreted program, and the differential tests treat
//! one as a failure.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::Arena;
use crate::bytecode::{
    ArrayElem, BlockType, ConstValue, EnumField, Module, NewCompositeOperand, Op, StructField,
    TupleField, TypeTag, Value,
};
use crate::value_layout::{CompositeKind, ScalarKind};
use crate::vm::{Vm, VmError, VmState, required_persistent_capacity_for};

/// The interpreter's source, embedded so the driver needs no filesystem access.
pub const INTERP_SOURCE: &str = include_str!("interp.kel");

/// Interpreted instructions per tick, the literal in `tick` in `interp.kel`.
pub const FUEL: u64 = 32;

/// Capacities of the interpreter's tables, matching the array sizes in `interp.kel`.
pub mod caps {
    /// Instructions across all chunks.
    pub const OPS: usize = 4096;
    /// Constants across all chunks.
    pub const CONSTS: usize = 512;
    /// Chunks.
    pub const CHUNKS: usize = 64;
    /// Entry arguments.
    pub const ARGS: usize = 16;
    /// Operand-stack slots, including locals.
    pub const STACK: usize = 512;
    /// Heap bytes, which hold the composite arguments and every body built while running.
    pub const HEAP: usize = 2048;
    /// The largest composite body.
    pub const BODY: usize = 256;
    /// Data slots.
    pub const DATA: usize = 256;
}

/// The interpreter's instruction tags. Kept in lockstep with the `match` in `step` in
/// `interp.kel`. Operands not listed are zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Tag {
    /// `Const(i)`, with `a` the index into the module-wide constant table.
    Const = 1,
    /// `GetLocal(n)`.
    GetLocal = 2,
    /// `SetLocal(n)`.
    SetLocal = 3,
    /// `PopN(n)`.
    PopN = 4,
    /// `Dup`.
    Dup = 5,
    /// `PushImmediate(v)`.
    PushImmediate = 6,
    /// `If(target)`.
    If = 7,
    /// `Else(target)`.
    Else = 8,
    /// `EndIf`.
    EndIf = 9,
    /// `Loop(exit)`.
    Loop = 10,
    /// `EndLoop(back_edge)`.
    EndLoop = 11,
    /// `Break(exit)`.
    Break = 12,
    /// `BreakIf(exit)`.
    BreakIf = 13,
    /// `Return`.
    Return = 14,
    /// `Not`.
    Not = 15,
    /// `CmpEq`.
    CmpEq = 16,
    /// `CmpNe`.
    CmpNe = 17,
    /// `CmpLt`.
    CmpLt = 18,
    /// `CmpGt`.
    CmpGt = 19,
    /// `CmpLe`.
    CmpLe = 20,
    /// `CmpGe`.
    CmpGe = 21,
    /// `CheckedAdd`.
    CheckedAdd = 22,
    /// `CheckedSub`.
    CheckedSub = 23,
    /// `CheckedMul(0)`, integer multiply only.
    CheckedMul = 24,
    /// `CheckedDiv(0)`, integer divide only.
    CheckedDiv = 25,
    /// `CheckedMod`.
    CheckedMod = 26,
    /// `CheckedNeg`.
    CheckedNeg = 27,
    /// `BitAnd`.
    BitAnd = 28,
    /// `BitOr`.
    BitOr = 29,
    /// `BitXor`.
    BitXor = 30,
    /// `Shl`.
    Shl = 31,
    /// `Shr`.
    Shr = 32,
    /// `ByteToWord`.
    ByteToWord = 33,
    /// `WordToByte`.
    WordToByte = 34,
    /// `Trap(kind)`, kinds 0 to 8.
    Trap = 35,
    /// `Add`, which the compiler emits only for Byte.
    Add = 36,
    /// `Sub`, which the compiler emits only for Byte.
    Sub = 37,
    /// `Mul`, which the compiler emits only for Byte.
    Mul = 38,
    /// `NewComposite(Flat)`, with `a` the composite kind, `b` the count and `c` the baked size.
    NewComposite = 39,
    /// A flat scalar field of a struct, tuple or enum, with `a` the offset and `b` the scalar
    /// kind code.
    Field = 40,
    /// A nested composite field, with `a` the offset, `b` the size and `c` the composite kind.
    FieldNested = 41,
    /// `GetIndex(Flat)` for Word and Byte, with `a` the scalar kind code.
    Index = 42,
    /// `GetIndex(FlatNested)`, with `a` the element size and `b` the composite kind.
    IndexNested = 43,
    /// `IsEnum`, with `a` the module-wide index of the discriminant constant.
    IsEnum = 44,
    /// `GetData(slot)`.
    GetData = 45,
    /// `SetData(slot)`.
    SetData = 46,
    /// `GetDataIndexed(base, len)`, with `a` the base and `b` the length.
    GetDataIndexed = 47,
    /// `SetDataIndexed(base, len)`, with `a` the base and `b` the length.
    SetDataIndexed = 48,
    /// `BoundsCheck(bound)`.
    BoundsCheck = 49,
    /// `Call(chunk, argc)`, with `a` the chunk and `b` the argument count.
    Call = 50,
    /// `Stream`, a marker.
    Stream = 51,
    /// `Yield`.
    Yield = 52,
    /// `Reset`.
    Reset = 53,
}

/// The interpreter's code for a flat scalar kind, which is the scalar's value tag. `None` for a
/// kind the first pass does not read.
fn scalar_code(k: ScalarKind) -> Option<i64> {
    match k {
        ScalarKind::Unit => Some(0),
        ScalarKind::Int => Some(1),
        ScalarKind::Bool => Some(2),
        ScalarKind::Byte => Some(3),
        _ => None,
    }
}

fn composite_code(k: CompositeKind) -> i64 {
    match k {
        CompositeKind::Tuple => 0,
        CompositeKind::Array => 1,
        CompositeKind::Struct => 2,
        CompositeKind::Enum => 3,
    }
}

/// A value as the interpreter holds it. The scalar tag numbers are those of `interp.kel`. A
/// composite is its kind code, 0 tuple, 1 array, 2 struct or 3 enum, and its flat body bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostedValue {
    /// Tag 0.
    Unit,
    /// Tag 1.
    Word(i64),
    /// Tag 2.
    Bool(bool),
    /// Tag 3.
    Byte(u8),
    /// Tag 4.
    None,
    /// A flat composite, tags 5 to 8.
    Composite {
        /// The composite kind code.
        kind: i64,
        /// The flat body.
        bytes: Vec<u8>,
    },
}

impl HostedValue {
    /// The scalar tag and word, or `None` for a composite, which travels through the heap.
    fn encode_scalar(&self) -> Option<(i64, i64)> {
        match *self {
            HostedValue::Unit => Some((0, 0)),
            HostedValue::Word(w) => Some((1, w)),
            HostedValue::Bool(b) => Some((2, i64::from(b))),
            HostedValue::Byte(b) => Some((3, i64::from(b))),
            HostedValue::None => Some((4, 0)),
            HostedValue::Composite { .. } => None,
        }
    }

    fn decode(t: i64, w: i64) -> Option<Self> {
        match (t, w) {
            (0, _) => Some(HostedValue::Unit),
            (1, w) => Some(HostedValue::Word(w)),
            (2, 0) => Some(HostedValue::Bool(false)),
            (2, 1) => Some(HostedValue::Bool(true)),
            (3, w) => u8::try_from(w).ok().map(HostedValue::Byte),
            (4, _) => Some(HostedValue::None),
            _ => None,
        }
    }

    /// The interpreter's view of a Rust virtual machine value, when it is a supported scalar or
    /// a flat composite whose body is resident in `arena`.
    pub fn from_value(v: &Value, arena: &Arena) -> Option<Self> {
        use crate::bytecode::{ArrayBody, EnumBody, StructBody, TupleBody};
        let flat = |kind: i64, fc: &crate::flat_value::FlatComposite| {
            fc.resolve(arena).ok().map(|b| HostedValue::Composite {
                kind,
                bytes: b.to_vec(),
            })
        };
        match v {
            Value::Unit => Some(HostedValue::Unit),
            Value::Int(w) => Some(HostedValue::Word(*w)),
            Value::Bool(b) => Some(HostedValue::Bool(*b)),
            Value::Byte(b) => Some(HostedValue::Byte(*b)),
            Value::None => Some(HostedValue::None),
            Value::Tuple(TupleBody::Flat(fc)) => flat(0, fc),
            Value::Array(ArrayBody::Flat(fc)) => flat(1, fc),
            Value::Struct(StructBody::Flat(fc)) => flat(2, fc),
            Value::Enum(EnumBody::Flat(fc)) => flat(3, fc),
            _ => None,
        }
    }

    /// A module constant, when it is a supported scalar.
    pub fn from_const(c: &ConstValue) -> Option<Self> {
        match c {
            ConstValue::Unit => Some(HostedValue::Unit),
            ConstValue::Int(w) => Some(HostedValue::Word(*w)),
            ConstValue::Bool(b) => Some(HostedValue::Bool(*b)),
            ConstValue::Byte(b) => Some(HostedValue::Byte(*b)),
            _ => None,
        }
    }

    /// The Rust virtual machine value for this scalar. `None` for a composite, which the caller
    /// builds as a Rust value directly.
    pub fn to_value(&self) -> Option<Value> {
        match *self {
            HostedValue::Unit => Some(Value::Unit),
            HostedValue::Word(w) => Some(Value::Int(w)),
            HostedValue::Bool(b) => Some(Value::Bool(b)),
            HostedValue::Byte(b) => Some(Value::Byte(b)),
            HostedValue::None => Some(Value::None),
            HostedValue::Composite { .. } => None,
        }
    }
}

/// Why a module was not admitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// An instruction outside the supported set, named with its chunk and position.
    Unsupported(String),
    /// A constant that is not a supported scalar.
    Constant(String),
    /// A table larger than the interpreter's capacity.
    Capacity(String),
    /// The module has no entry chunk.
    NoEntry,
    /// The entry chunk has no verified nominal time bound.
    Unbounded(String),
}

/// One chunk's row in the interpreter's chunk table.
#[derive(Clone, Debug)]
struct ChunkEntry {
    start: i64,
    len: i64,
    locals: i64,
    params: i64,
    /// The position of the chunk's `Stream`, or -1.
    stream: i64,
}

/// A module encoded into the interpreter's tables.
#[derive(Clone, Debug)]
pub struct Admitted {
    ops: Vec<[i64; 4]>,
    consts: Vec<(i64, i64)>,
    chunks: Vec<ChunkEntry>,
    entry: i64,
    instruction_bound: u64,
    data: Vec<(i64, i64)>,
}

impl Admitted {
    /// The entry chunk's declared parameter count.
    pub fn entry_params(&self, module: &Module) -> usize {
        module.chunks[self.entry as usize].param_count as usize
    }

    /// An upper bound on the interpreted instructions one run executes. It is the entry chunk's
    /// verified nominal time bound, which bounds the instruction count because every
    /// instruction costs at least one nominal cycle. The module's own `wcet_cycles` field is not
    /// used, because zero there means the producer declared nothing.
    pub fn instruction_bound(&self) -> u64 {
        self.instruction_bound
    }

    /// The ticks one run may take: the instruction bound over [`FUEL`], plus the tick in which
    /// the run ends.
    pub fn tick_bound(&self) -> u64 {
        self.instruction_bound.div_ceil(FUEL) + 1
    }
}

// `Const` carries a chunk-local index, so `admit` encodes it and this function does not.
fn encode_op(op: &Op) -> Option<[i64; 4]> {
    let t = |tag: Tag, a: i64| Some([tag as i64, a, 0, 0]);
    match *op {
        Op::GetLocal(n) => t(Tag::GetLocal, i64::from(n)),
        Op::SetLocal(n) => t(Tag::SetLocal, i64::from(n)),
        Op::PopN(n) => t(Tag::PopN, i64::from(n)),
        Op::Dup => t(Tag::Dup, 0),
        Op::PushImmediate(v) => t(Tag::PushImmediate, i64::from(v)),
        Op::If(x) => t(Tag::If, i64::from(x)),
        Op::Else(x) => t(Tag::Else, i64::from(x)),
        Op::EndIf => t(Tag::EndIf, 0),
        Op::Loop(x) => t(Tag::Loop, i64::from(x)),
        Op::EndLoop(x) => t(Tag::EndLoop, i64::from(x)),
        Op::Break(x) => t(Tag::Break, i64::from(x)),
        Op::BreakIf(x) => t(Tag::BreakIf, i64::from(x)),
        Op::Return => t(Tag::Return, 0),
        Op::Not => t(Tag::Not, 0),
        Op::CmpEq => t(Tag::CmpEq, 0),
        Op::CmpNe => t(Tag::CmpNe, 0),
        Op::CmpLt => t(Tag::CmpLt, 0),
        Op::CmpGt => t(Tag::CmpGt, 0),
        Op::CmpLe => t(Tag::CmpLe, 0),
        Op::CmpGe => t(Tag::CmpGe, 0),
        Op::CheckedAdd => t(Tag::CheckedAdd, 0),
        Op::CheckedSub => t(Tag::CheckedSub, 0),
        Op::CheckedMul(0) => t(Tag::CheckedMul, 0),
        Op::CheckedDiv(0) => t(Tag::CheckedDiv, 0),
        Op::CheckedMod => t(Tag::CheckedMod, 0),
        Op::CheckedNeg => t(Tag::CheckedNeg, 0),
        Op::BitAnd => t(Tag::BitAnd, 0),
        Op::BitOr => t(Tag::BitOr, 0),
        Op::BitXor => t(Tag::BitXor, 0),
        Op::Shl => t(Tag::Shl, 0),
        Op::Shr => t(Tag::Shr, 0),
        Op::ByteToWord => t(Tag::ByteToWord, 0),
        Op::WordToByte => t(Tag::WordToByte, 0),
        Op::Trap(k) if k <= 8 => t(Tag::Trap, i64::from(k)),
        Op::Add => t(Tag::Add, 0),
        Op::Sub => t(Tag::Sub, 0),
        Op::Mul => t(Tag::Mul, 0),
        Op::NewComposite(NewCompositeOperand::Flat {
            kind,
            count,
            byte_size,
        }) => Some([
            Tag::NewComposite as i64,
            composite_code(kind),
            i64::from(count),
            i64::from(byte_size),
        ]),
        Op::GetField(StructField::Flat { offset, kind })
        | Op::GetTupleField(TupleField::Flat { offset, kind })
        | Op::GetEnumField(EnumField::Flat { offset, kind }) => {
            Some([Tag::Field as i64, i64::from(offset), scalar_code(kind)?, 0])
        }
        Op::GetField(StructField::FlatNested {
            offset,
            size,
            variant,
        })
        | Op::GetTupleField(TupleField::FlatNested {
            offset,
            size,
            variant,
        })
        | Op::GetEnumField(EnumField::FlatNested {
            offset,
            size,
            variant,
        }) => Some([
            Tag::FieldNested as i64,
            i64::from(offset),
            i64::from(size),
            composite_code(variant),
        ]),
        Op::GetIndex(ArrayElem::Flat {
            kind: k @ (ScalarKind::Int | ScalarKind::Byte),
        }) => Some([Tag::Index as i64, scalar_code(k)?, 0, 0]),
        Op::GetIndex(ArrayElem::FlatNested { size, variant }) => Some([
            Tag::IndexNested as i64,
            i64::from(size),
            composite_code(variant),
            0,
        ]),
        Op::GetData(slot) => t(Tag::GetData, i64::from(slot)),
        Op::BoundsCheck(bound) => t(Tag::BoundsCheck, i64::from(bound)),
        Op::Call(callee, argc) => Some([Tag::Call as i64, i64::from(callee), i64::from(argc), 0]),
        Op::Stream => t(Tag::Stream, 0),
        Op::Yield => t(Tag::Yield, 0),
        Op::Reset => t(Tag::Reset, 0),
        Op::SetData(slot) => t(Tag::SetData, i64::from(slot)),
        Op::GetDataIndexed(base, len) => Some([
            Tag::GetDataIndexed as i64,
            i64::from(base),
            i64::from(len),
            0,
        ]),
        Op::SetDataIndexed(base, len) => Some([
            Tag::SetDataIndexed as i64,
            i64::from(base),
            i64::from(len),
            0,
        ]),
        _ => None,
    }
}

/// The initial value of every data slot, as the Rust virtual machine sees it at load. The first
/// pass supports private scalar slots only. The values are read from the module's wire round
/// trip rather than from the in-memory module, because the virtual machine reads them from the
/// encoded form, where an omitted initializer becomes `Unit`.
fn data_inits(module: &Module) -> Result<Vec<(i64, i64)>, Refusal> {
    let Some(layout) = module.data_layout.as_ref() else {
        return Ok(Vec::new());
    };
    if !layout.shared_layout.is_empty() {
        return Err(Refusal::Unsupported(String::from("shared data")));
    }
    if !layout.private_composite_layout.is_empty() {
        return Err(Refusal::Unsupported(String::from("private composite data")));
    }
    if layout.slots.len() > caps::DATA {
        return Err(Refusal::Capacity(format!(
            "{} data slots",
            layout.slots.len()
        )));
    }
    let wire = module
        .to_bytes()
        .and_then(|b| Module::from_bytes(&b))
        .map_err(|e| Refusal::Unsupported(format!("data layout round trip: {e:?}")))?;
    let inits = wire.data_layout.map(|l| l.private_init).unwrap_or_default();
    (0..layout.slots.len())
        .map(|i| match inits.get(i) {
            None => Ok((0, 0)),
            Some(c) => HostedValue::from_const(c)
                .and_then(|v| v.encode_scalar())
                .ok_or_else(|| Refusal::Constant(format!("data slot {i} starts as {c:?}"))),
        })
        .collect()
}

/// A chunk's verified nominal time bound with every call charged its callee's bound.
///
/// The verifier's per-chunk bound charges a call its dispatch and not the callee's body. Its
/// path walk does charge a verified native call an attested per-call cost at that instruction,
/// with branch maxima and loop multiplicities applied. So the bound is computed on an
/// analysis-only copy of the chunk in which each `Call(i, n)` becomes `CallVerifiedNative(i, n)`
/// with native `i` attested at callee `i`'s bound. Both instructions cost the same nominal
/// cycles. The module itself is not changed.
fn chunk_bound(chunk: &crate::bytecode::Chunk, callee_bounds: &[u32]) -> Result<u64, Refusal> {
    use crate::verify::NativeIterationBound;
    let mut copy = chunk.clone();
    for op in copy.ops.iter_mut() {
        if let Op::Call(i, n) = *op {
            *op = Op::CallVerifiedNative(i, n);
        }
    }
    let natives: Vec<NativeIterationBound> = callee_bounds
        .iter()
        .map(|&b| NativeIterationBound {
            per_call_wcmu_bytes: 0,
            per_call_wcet_cycles: b,
            max_invocations: None,
        })
        .collect();
    let model = &crate::bytecode::NOMINAL_COST_MODEL;
    if matches!(copy.block_type, BlockType::Stream) {
        crate::verify::wcet_stream_iteration_with_cost_model(&copy, model, &natives)
    } else {
        crate::verify::wcet_whole_chunk_with_cost_model(&copy, model, &natives)
    }
    .map(u64::from)
    .map_err(|e| Refusal::Unbounded(format!("chunk {}: {e:?}", chunk.name)))
}

/// An upper bound on the instructions one run of `entry` executes, including every callee, in
/// nominal cycles. Every instruction costs at least one nominal cycle, so the figure also bounds
/// the instruction count. For a stream chunk it is the bound of one iteration.
///
/// The bounds are computed bottom-up over the call graph by [`chunk_bound`], which must be
/// acyclic. A cycle is refused rather than assumed absent. A bound that does not fit the
/// verifier's 32-bit attestation field is refused.
pub fn transitive_bound(module: &Module, entry: usize) -> Result<u64, Refusal> {
    let n = module.chunks.len();
    let callees: Vec<Vec<usize>> = module
        .chunks
        .iter()
        .map(|c| {
            c.ops
                .iter()
                .filter_map(|op| match op {
                    Op::Call(i, _) => Some(*i as usize),
                    _ => None,
                })
                .collect()
        })
        .collect();
    let mut bound: Vec<Option<u64>> = alloc::vec![None; n];
    // Each pass settles every chunk whose callees are all settled. An acyclic graph of `n`
    // chunks settles within `n` passes, so a pass that settles nothing means a cycle.
    for _ in 0..=n {
        let mut progressed = false;
        for c in 0..n {
            if bound[c].is_some()
                || !callees[c]
                    .iter()
                    .all(|&k| bound.get(k).copied().flatten().is_some())
            {
                continue;
            }
            let attested: Vec<u32> = bound
                .iter()
                .map(|b| b.map_or(0, |v| u32::try_from(v).unwrap_or(u32::MAX)))
                .collect();
            if callees[c].iter().any(|&k| attested[k] == u32::MAX) {
                return Err(Refusal::Unbounded(format!(
                    "a callee of chunk {c} exceeds 32 bits"
                )));
            }
            bound[c] = Some(chunk_bound(&module.chunks[c], &attested)?);
            progressed = true;
        }
        if !progressed {
            break;
        }
    }
    bound
        .get(entry)
        .copied()
        .flatten()
        .ok_or_else(|| Refusal::Unbounded(String::from("the call graph has a cycle")))
}

/// Decide whether the interpreter supports `module` and encode it. The module must already
/// have passed the Rust verifier; this function checks only what the interpreter needs.
pub fn admit(module: &Module) -> Result<Admitted, Refusal> {
    let entry = module.entry_point.ok_or(Refusal::NoEntry)?;
    if module.chunks.len() > caps::CHUNKS {
        return Err(Refusal::Capacity(format!("{} chunks", module.chunks.len())));
    }
    let data = data_inits(module)?;
    let mut ops = Vec::new();
    let mut consts = Vec::new();
    let mut chunks = Vec::new();
    for (ci, chunk) in module.chunks.iter().enumerate() {
        let k0 = consts.len() as i64;
        for (ki, c) in chunk.constants.iter().enumerate() {
            // A string constant is carried as the inert tag 9. The first pass uses one only as a
            // name operand of `IsEnum`, which a flat enum ignores, and any other use faults.
            let e = match c {
                ConstValue::StaticStr(_) => (9, 0),
                _ => HostedValue::from_const(c)
                    .and_then(|v| v.encode_scalar())
                    .ok_or_else(|| {
                        Refusal::Constant(format!("chunk {ci} constant {ki} is {c:?}"))
                    })?,
            };
            consts.push(e);
        }
        let start = ops.len() as i64;
        for (ip, op) in chunk.ops.iter().enumerate() {
            // Constant indices are per chunk in the module and module-wide in the tables.
            let e = match *op {
                Op::Const(i) => [Tag::Const as i64, k0 + i64::from(i), 0, 0],
                Op::IsEnum(_, _, d) => [Tag::IsEnum as i64, k0 + i64::from(d), 0, 0],
                _ => encode_op(op)
                    .ok_or_else(|| Refusal::Unsupported(format!("chunk {ci} op {ip} {op:?}")))?,
            };
            ops.push(e);
        }
        let stream = chunk
            .ops
            .iter()
            .position(|op| matches!(op, Op::Stream))
            .map_or(-1, |p| p as i64);
        if stream >= 0 {
            // The first pass resumes with scalars only, so a stream's parameter must be one.
            let scalar = |t: &TypeTag| {
                matches!(
                    t,
                    TypeTag::Word | TypeTag::Byte | TypeTag::Bool | TypeTag::Unit
                )
            };
            if !chunk.param_types.iter().all(scalar) {
                return Err(Refusal::Unsupported(format!(
                    "chunk {ci} resumes with a non-scalar parameter"
                )));
            }
        }
        chunks.push(ChunkEntry {
            start,
            len: chunk.ops.len() as i64,
            locals: i64::from(chunk.local_count),
            params: i64::from(chunk.param_count),
            stream,
        });
    }
    if ops.len() > caps::OPS {
        return Err(Refusal::Capacity(format!("{} instructions", ops.len())));
    }
    if consts.len() > caps::CONSTS {
        return Err(Refusal::Capacity(format!("{} constants", consts.len())));
    }
    let bound = transitive_bound(module, entry)?;
    Ok(Admitted {
        ops,
        consts,
        chunks,
        entry: entry as i64,
        instruction_bound: bound,
        data,
    })
}

/// How an interpreted run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The entry chunk returned this value.
    Finished(HostedValue),
    /// The program trapped with this kind. For the bounds trap, kind 8, the index and bound.
    Trapped {
        /// The trap kind code.
        kind: i64,
        /// The bounds trap's index, zero otherwise.
        index: i64,
        /// The bounds trap's bound, zero otherwise.
        bound: i64,
    },
    /// The interpreter faulted with this code. See the fault list in `interp.kel`.
    Fault(i64),
    /// The program yielded this value and waits to be resumed.
    Yielded(HostedValue),
    /// The program ended a stream iteration and waits to be resumed.
    Reset,
    /// The run used every tick it was given without ending.
    OutOfTicks,
}

/// A completed interpreted run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    /// How it ended.
    pub outcome: Outcome,
    /// Ticks used, counting the tick that ended it.
    pub ticks: u64,
    /// Interpreted instructions executed since the program started.
    pub steps: u64,
    /// The most instructions any completed stream iteration executed, zero before the first
    /// reset.
    pub max_iteration_steps: u64,
}

/// Slot bases of the interpreter's shared fields, found by name in its data layout.
struct Slots {
    n_chunks: usize,
    entry: usize,
    nargs: usize,
    op_t: usize,
    op_a: usize,
    op_b: usize,
    op_c: usize,
    k_count: usize,
    k_t: usize,
    k_w: usize,
    ch_start: usize,
    ch_len: usize,
    ch_locals: usize,
    arg_t: usize,
    arg_w: usize,
    arg_sz: usize,
    hp0: usize,
    heap: usize,
    out_t: usize,
    out_w: usize,
    out_sz: usize,
    out_kind: usize,
    out_index: usize,
    out_bound: usize,
    out_fault: usize,
    steps: usize,
    d_count: usize,
    d_t: usize,
    d_w: usize,
    ch_params: usize,
    ch_stream: usize,
    res_t: usize,
    res_w: usize,
    max_iter: usize,
}

impl Slots {
    fn find(module: &Module) -> Result<Self, VmError> {
        let layout = module
            .data_layout
            .as_ref()
            .ok_or_else(|| VmError::InvalidBytecode(String::from("interp.kel has no data")))?;
        let base = |name: &str| -> Result<usize, VmError> {
            let full = format!("io.{name}");
            layout
                .slots
                .iter()
                .position(|s| s.name == full)
                .ok_or_else(|| VmError::InvalidBytecode(format!("interp.kel lacks {full}")))
        };
        Ok(Slots {
            n_chunks: base("n_chunks")?,
            entry: base("entry")?,
            nargs: base("nargs")?,
            op_t: base("op_t")?,
            op_a: base("op_a")?,
            op_b: base("op_b")?,
            op_c: base("op_c")?,
            k_count: base("k_count")?,
            k_t: base("k_t")?,
            k_w: base("k_w")?,
            ch_start: base("ch_start")?,
            ch_len: base("ch_len")?,
            ch_locals: base("ch_locals")?,
            arg_t: base("arg_t")?,
            arg_w: base("arg_w")?,
            arg_sz: base("arg_sz")?,
            hp0: base("hp0")?,
            heap: base("heap")?,
            out_t: base("out_t")?,
            out_w: base("out_w")?,
            out_sz: base("out_sz")?,
            out_kind: base("out_kind")?,
            out_index: base("out_index")?,
            out_bound: base("out_bound")?,
            out_fault: base("out_fault")?,
            steps: base("steps")?,
            d_count: base("d_count")?,
            d_t: base("d_t")?,
            d_w: base("d_w")?,
            ch_params: base("ch_params")?,
            ch_stream: base("ch_stream")?,
            res_t: base("res_t")?,
            res_w: base("res_w")?,
            max_iter: base("max_iter")?,
        })
    }
}

/// Compile `interp.kel` with the reference compiler. The Rust verifier runs when the
/// interpreter's virtual machine is built in [`Interpreter::new`].
pub fn interpreter_module() -> Result<Module, VmError> {
    let tokens = crate::lexer::tokenize(INTERP_SOURCE)
        .map_err(|e| VmError::InvalidBytecode(format!("interp.kel lex: {e:?}")))?;
    let program = crate::parser::parse(&tokens)
        .map_err(|e| VmError::InvalidBytecode(format!("interp.kel parse: {e:?}")))?;
    crate::compiler::compile(&program)
        .map_err(|e| VmError::InvalidBytecode(format!("interp.kel compile: {}", e.message)))
}

/// The persistent arena capacity the interpreter's private state needs.
pub fn interpreter_persistent_bytes(module: &Module) -> usize {
    required_persistent_capacity_for(module)
}

/// The interpreter running on the Rust virtual machine, with its shared buffer.
pub struct Interpreter<'arena> {
    vm: Vm<'static, 'arena>,
    shared: Vec<u8>,
    slots: Slots,
}

impl<'arena> Interpreter<'arena> {
    /// Build the interpreter on `arena`, whose persistent region must already be sized by
    /// [`interpreter_persistent_bytes`]. The Rust verifier checks `interp.kel` here.
    pub fn new(module: Module, arena: &'arena Arena) -> Result<Self, VmError> {
        let slots = Slots::find(&module)?;
        let vm = Vm::new(module, arena)?;
        let shared = alloc::vec![0u8; vm.shared_data_bytes()];
        Ok(Interpreter { vm, shared, slots })
    }

    fn set(&mut self, slot: usize, w: i64) -> Result<(), VmError> {
        self.vm.set_shared(&mut self.shared, slot, Value::Int(w))
    }

    fn get(&self, slot: usize) -> Result<i64, VmError> {
        match self.vm.get_shared(&self.shared, slot)? {
            Value::Int(w) => Ok(w),
            other => Err(VmError::TypeError(format!(
                "interp.kel shared slot {slot} holds {}",
                other.type_name()
            ))),
        }
    }

    fn load(&mut self, adm: &Admitted, args: &[HostedValue]) -> Result<(), VmError> {
        if args.len() > caps::ARGS {
            return Err(VmError::InvalidBytecode(format!(
                "{} arguments",
                args.len()
            )));
        }
        let s = &self.slots;
        let (op_t, op_a, op_b, op_c) = (s.op_t, s.op_a, s.op_b, s.op_c);
        let (k_t, k_w, ch_start, ch_len, ch_locals) =
            (s.k_t, s.k_w, s.ch_start, s.ch_len, s.ch_locals);
        let (arg_t, arg_w, arg_sz, heap) = (s.arg_t, s.arg_w, s.arg_sz, s.heap);
        self.set(self.slots.n_chunks, adm.chunks.len() as i64)?;
        self.set(self.slots.entry, adm.entry)?;
        self.set(self.slots.nargs, args.len() as i64)?;
        for (i, op) in adm.ops.iter().enumerate() {
            self.set(op_t + i, op[0])?;
            self.set(op_a + i, op[1])?;
            self.set(op_b + i, op[2])?;
            self.set(op_c + i, op[3])?;
        }
        self.set(self.slots.k_count, adm.consts.len() as i64)?;
        for (i, &(t, w)) in adm.consts.iter().enumerate() {
            self.set(k_t + i, t)?;
            self.set(k_w + i, w)?;
        }
        let (ch_params, ch_stream) = (self.slots.ch_params, self.slots.ch_stream);
        for (i, c) in adm.chunks.iter().enumerate() {
            self.set(ch_start + i, c.start)?;
            self.set(ch_len + i, c.len)?;
            self.set(ch_locals + i, c.locals)?;
            self.set(ch_params + i, c.params)?;
            self.set(ch_stream + i, c.stream)?;
        }
        // Composite arguments are written to the bottom of the heap, in order, and the
        // interpreter allocates above them.
        let mut hp = 0usize;
        for (i, a) in args.iter().enumerate() {
            let (t, w, sz) = match a {
                HostedValue::Composite { kind, bytes } => {
                    if bytes.len() > caps::BODY || hp + bytes.len() > caps::HEAP {
                        return Err(VmError::InvalidBytecode(format!(
                            "argument {i} of {} bytes exceeds the interpreter heap",
                            bytes.len()
                        )));
                    }
                    for (k, &b) in bytes.iter().enumerate() {
                        self.set(heap + hp + k, i64::from(b))?;
                    }
                    let at = hp as i64;
                    hp += bytes.len();
                    (5 + kind, at, bytes.len() as i64)
                }
                scalar => {
                    let (t, w) = scalar.encode_scalar().expect("a scalar");
                    (t, w, 0)
                }
            };
            self.set(arg_t + i, t)?;
            self.set(arg_w + i, w)?;
            self.set(arg_sz + i, sz)?;
        }
        self.set(self.slots.hp0, hp as i64)?;
        let (d_t, d_w) = (self.slots.d_t, self.slots.d_w);
        self.set(self.slots.d_count, adm.data.len() as i64)?;
        for (i, &(t, w)) in adm.data.iter().enumerate() {
            self.set(d_t + i, t)?;
            self.set(d_w + i, w)?;
        }
        Ok(())
    }

    /// Send one command and return the yielded status. The first command enters `loop main`
    /// through a call, and every later one through a resume. The machine reports which it wants
    /// with `NotSuspended`. A resume after a yield ends the loop body, which reports `Reset`, and
    /// the same command then re-enters the body as its parameter.
    fn send(&mut self, cmd: i64) -> Result<i64, VmError> {
        for _ in 0..2 {
            let state = match self
                .vm
                .resume_with_shared(&mut self.shared, Value::Int(cmd))
            {
                Err(VmError::NotSuspended) => self
                    .vm
                    .call_with_shared(&mut self.shared, &[Value::Int(cmd)])?,
                other => other?,
            };
            match state {
                VmState::Yielded(Value::Int(s)) => return Ok(s),
                VmState::Reset => continue,
                other => {
                    return Err(VmError::TypeError(format!(
                        "interp.kel produced {other:?} instead of a status"
                    )));
                }
            }
        }
        Err(VmError::TypeError(String::from(
            "interp.kel reset twice without yielding a status",
        )))
    }

    /// Run `adm` with `args` for at most `max_ticks` ticks. An error is a failure of the
    /// interpreter program itself on the Rust virtual machine, distinct from an [`Outcome`].
    pub fn run(
        &mut self,
        adm: &Admitted,
        args: &[HostedValue],
        max_ticks: u64,
    ) -> Result<Run, VmError> {
        self.load(adm, args)?;
        let status = self.send(1)?;
        self.drive(status, max_ticks)
    }

    /// Resume an interpreted program that yielded or reset, with the scalar `input`, and run it
    /// until it next yields, resets or ends, for at most `max_ticks` ticks. The input is
    /// delivered by the tick that runs first, which is counted.
    pub fn resume(&mut self, input: &HostedValue, max_ticks: u64) -> Result<Run, VmError> {
        let (t, w) = input.encode_scalar().ok_or_else(|| {
            VmError::TypeError(String::from("the first pass resumes with scalars only"))
        })?;
        self.set(self.slots.res_t, t)?;
        self.set(self.slots.res_w, w)?;
        let status = self.send(3)?;
        self.drive_from(status, max_ticks, 1)
    }

    fn drive(&mut self, status: i64, max_ticks: u64) -> Result<Run, VmError> {
        self.drive_from(status, max_ticks, 0)
    }

    fn drive_from(
        &mut self,
        mut status: i64,
        max_ticks: u64,
        mut ticks: u64,
    ) -> Result<Run, VmError> {
        while status == 0 && ticks < max_ticks {
            status = self.send(2)?;
            ticks += 1;
        }
        let steps = self.get(self.slots.steps)? as u64;
        let max_iteration_steps = self.get(self.slots.max_iter)? as u64;
        let outcome = match status {
            0 => Outcome::OutOfTicks,
            1 => Outcome::Finished(self.output()?),
            2 => Outcome::Yielded(self.output()?),
            5 => Outcome::Reset,
            3 => {
                let kind = self.get(self.slots.out_kind)?;
                let (index, bound) = if kind == 8 {
                    (
                        self.get(self.slots.out_index)?,
                        self.get(self.slots.out_bound)?,
                    )
                } else {
                    (0, 0)
                };
                Outcome::Trapped { kind, index, bound }
            }
            4 => Outcome::Fault(self.get(self.slots.out_fault)?),
            other => {
                return Err(VmError::TypeError(format!("interp.kel status {other}")));
            }
        };
        Ok(Run {
            outcome,
            ticks,
            steps,
            max_iteration_steps,
        })
    }

    /// The published output value: a returned or yielded value, read from the heap when it is a
    /// composite.
    fn output(&self) -> Result<HostedValue, VmError> {
        let (t, w) = (self.get(self.slots.out_t)?, self.get(self.slots.out_w)?);
        let value = if (5..=8).contains(&t) {
            let sz = self.get(self.slots.out_sz)?;
            let mut bytes = Vec::new();
            for k in 0..sz {
                let b = self.get(self.slots.heap + (w + k) as usize)?;
                bytes.push(
                    u8::try_from(b)
                        .map_err(|_| VmError::TypeError(format!("interp.kel heap byte {b}")))?,
                );
            }
            Some(HostedValue::Composite { kind: t - 5, bytes })
        } else {
            HostedValue::decode(t, w)
        };
        value.ok_or_else(|| VmError::TypeError(format!("interp.kel output tag {t} word {w}")))
    }
}

/// The Rust virtual machine's outcome for a run, in the interpreter's terms, so the two can be
/// compared. A result the interpreter cannot represent, or an error that is not a trap, is
/// returned as `Err` with its description.
pub fn reference_outcome(
    result: Result<VmState, VmError>,
    arena: &Arena,
) -> Result<Outcome, String> {
    use crate::bytecode::TrapKind;
    match result {
        Ok(VmState::Finished(v)) => HostedValue::from_value(&v, arena)
            .map(Outcome::Finished)
            .ok_or_else(|| format!("unsupported result {v:?}")),
        Ok(VmState::Yielded(v)) => HostedValue::from_value(&v, arena)
            .map(Outcome::Yielded)
            .ok_or_else(|| format!("unsupported yield {v:?}")),
        Ok(VmState::Reset) => Ok(Outcome::Reset),
        Ok(other) => Err(format!("unsupported state {other:?}")),
        Err(e) => {
            let kind = match &e {
                VmError::RefinementFailed => TrapKind::RefinementFailed,
                VmError::NoMatchingHead => TrapKind::NoMatchingHead,
                VmError::NoMatchingArm => TrapKind::NoMatchingArm,
                VmError::CheckedArithNoArm => TrapKind::CheckedArithNoArm,
                VmError::EnumVariantUnmapped => TrapKind::EnumVariantUnmapped,
                VmError::DivisionByZero => TrapKind::ZeroDivisor,
                VmError::AssertionFailed => TrapKind::AssertionFailed,
                VmError::LoopLimitExceeded => TrapKind::LoopLimitExceeded,
                VmError::IndexOutOfBounds(i, b) => {
                    return Ok(Outcome::Trapped {
                        kind: i64::from(TrapKind::IndexOutOfBounds.code()),
                        index: *i,
                        bound: *b as i64,
                    });
                }
                other => return Err(format!("non-trap error {other:?}")),
            };
            Ok(Outcome::Trapped {
                kind: i64::from(kind.code()),
                index: 0,
                bound: 0,
            })
        }
    }
}
