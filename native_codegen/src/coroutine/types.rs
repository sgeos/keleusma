//! Coroutine scalar admission with the actual host reply type.
//!
//! The bytecode verifier deliberately defers unknown reply types to VM guards.
//! Untagged native operands need a stronger admission condition. This pass joins
//! possible tags at control-flow edges, including back edges and Reset. A state
//! can only gain bits, so the finite tag lattice makes the worklist terminate.
//! Composite extent validation remains the existing typed verifier's job.

use crate::LowerError;
use keleusma::bytecode::{BlockType, ConstValue, Module, Op, TypeTag, WireShape};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const UNIT: u16 = 1;
const FALSE: u16 = 2;
const TRUE: u16 = 4096;
const BOOL: u16 = FALSE | TRUE;
const BYTE: u16 = 4;
const WORD: u16 = 8;
const FIXED: u16 = 16;
const FLOAT: u16 = 32;
const TUPLE: u16 = 256;
const ARRAY: u16 = 512;
const STRUCT: u16 = 1024;
const ENUM: u16 = 2048;
const BODY: u16 = TUPLE | ARRAY | STRUCT | ENUM;
const UNKNOWN: u16 = 8191;

pub(crate) struct Analysis {
    pub(crate) false_inspections: BTreeSet<(usize, usize)>,
    pub(crate) branches: Vec<BTreeMap<usize, bool>>,
}
fn scalar(kind: u8) -> u16 {
    if kind == 1 {
        BOOL
    } else if kind < 8 {
        1 << kind
    } else {
        UNKNOWN
    }
}
fn body(kind: u8) -> u16 {
    if kind < 4 { 1 << (8 + kind) } else { BODY }
}
fn kind(mask: u16) -> u16 {
    (mask & !TRUE) | if mask & TRUE != 0 { FALSE } else { 0 }
}

fn shape(s: WireShape) -> u16 {
    match s {
        WireShape::Scalar { kind } => scalar(kind),
        WireShape::Flat { kind, .. } => body(kind),
        _ => UNKNOWN,
    }
}
fn constant(c: &ConstValue) -> u16 {
    match c {
        ConstValue::Unit => UNIT,
        ConstValue::Bool(b) => {
            if *b {
                TRUE
            } else {
                FALSE
            }
        }
        ConstValue::Byte(_) => BYTE,
        ConstValue::Int(_) => WORD,
        ConstValue::Fixed(_) => FIXED,
        ConstValue::Float(_) => FLOAT,
        ConstValue::StaticStr(_) => 64,
        ConstValue::Tuple(_) => TUPLE,
        ConstValue::Array(_) => ARRAY,
        ConstValue::Struct { .. } => STRUCT,
        ConstValue::Enum { .. } => ENUM,
        _ => UNKNOWN,
    }
}
#[derive(Clone)]
struct State {
    stack: Vec<u16>,
    locals: Vec<u16>,
}
impl State {
    fn join(&mut self, other: &Self) -> bool {
        assert_eq!(self.stack.len(), other.stack.len(), "verified stack depth");
        let mut changed = false;
        for (a, b) in self
            .stack
            .iter_mut()
            .chain(&mut self.locals)
            .zip(other.stack.iter().chain(&other.locals))
        {
            let union = *a | *b;
            changed |= union != *a;
            *a = union;
        }
        changed
    }
}
fn slot(m: &Module, index: u32) -> u16 {
    let Some(d) = &m.data_layout else {
        return UNKNOWN;
    };
    if let Some(s) = d.shared_layout.get(index as usize) {
        return if s.kind < 8 { scalar(s.kind) } else { BODY };
    }
    if d.private_composite_layout
        .iter()
        .any(|s| u32::from(s.slot) == index)
    {
        return BODY;
    }
    d.private_init
        .get(index as usize - d.shared_layout.len())
        .map(|c| {
            if matches!(c, ConstValue::Bool(_)) {
                BOOL
            } else {
                constant(c)
            }
        })
        .unwrap_or(UNKNOWN)
}

pub(super) fn check(m: &Module, entry: usize) -> Result<Analysis, LowerError> {
    if m.signatures.len() != m.chunks.len()
        || m.chunks
            .iter()
            .zip(&m.signatures)
            .any(|(c, s)| c.ops.is_empty() || s.params.len() != usize::from(c.param_count))
    {
        return Err(LowerError::UnsupportedShape(
            "coroutine scalar admission requires complete signatures".into(),
        ));
    }
    for (c, s) in m.chunks.iter().zip(&m.signatures) {
        for (tag, declared) in c.param_types.iter().zip(&s.params) {
            let actual = match tag {
                TypeTag::Unit => UNIT,
                TypeTag::Bool => BOOL,
                TypeTag::Byte => BYTE,
                TypeTag::Word => WORD,
                TypeTag::Fixed => FIXED,
                TypeTag::Float => FLOAT,
                TypeTag::Text => 64,
                TypeTag::Composite => continue,
            };
            if shape(*declared) != actual {
                return Err(LowerError::UnsupportedShape(
                    "coroutine parameter signature disagrees with its runtime type".into(),
                ));
            }
        }
    }
    let mut analysis = Analysis {
        false_inspections: BTreeSet::new(),
        branches: vec![BTreeMap::new(); m.chunks.len()],
    };
    let reply = shape(
        m.signatures[entry]
            .params
            .first()
            .copied()
            .unwrap_or(WireShape::Scalar { kind: 0 }),
    );
    let output = shape(m.signatures[entry].ret);
    for (ci, chunk) in m.chunks.iter().enumerate() {
        let sig = &m.signatures[ci];
        let mut initial = State {
            stack: Vec::new(),
            locals: vec![UNIT; chunk.local_count as usize],
        };
        for (i, p) in sig.params.iter().enumerate() {
            initial.locals[i] = shape(*p);
        }
        let mut states: Vec<Option<State>> = vec![None; chunk.ops.len()];
        states[0] = Some(initial);
        let mut work = VecDeque::from([0]);
        while let Some(ip) = work.pop_front() {
            let mut state = states[ip].clone().unwrap();
            let op = &chunk.ops[ip];
            let fail = || {
                LowerError::UnsupportedShape(format!(
                    "coroutine scalar types are not proven in {} at {ip}: {op:?}",
                    chunk.name
                ))
            };
            let require = |actual: u16, allowed: u16| {
                if actual != 0 && actual & !allowed == 0 {
                    Ok(())
                } else {
                    Err(fail())
                }
            };
            let (n, delta) = keleusma::verify::op_depth_effect(op, chunk);
            let n = n as usize;
            let start = state.stack.len().checked_sub(n).ok_or_else(fail)?;
            let args = state.stack.split_off(start);
            let produced = (n as i32 + delta) as usize;
            let mut out = vec![UNKNOWN; produced];
            let same = |allowed: u16| -> Result<u16, LowerError> {
                let a = kind(args[0]);
                require(a, kind(allowed))?;
                if a.count_ones() != 1 || args.iter().any(|b| kind(*b) != a) {
                    return Err(fail());
                }
                Ok(a)
            };
            match op {
                Op::Const(i) => out[0] = constant(&chunk.constants[*i as usize]),
                Op::PushImmediate(i) => {
                    out[0] = match i {
                        0 => UNIT,
                        1 => TRUE,
                        2 => FALSE,
                        4..=19 => WORD,
                        _ => UNKNOWN,
                    }
                }
                Op::GetLocal(i) => out[0] = state.locals[*i as usize],
                Op::SetLocal(i) => state.locals[*i as usize] = args[0],
                Op::Dup => out.fill(args[0]),
                Op::PopN(_) => {}
                Op::Yield => {
                    require(args[0], output)?;
                    out[0] = reply;
                    if ci == entry && chunk.block_type == BlockType::Stream && chunk.param_count > 0
                    {
                        state.locals[0] = reply;
                    }
                }
                Op::Call(target, _) => {
                    let callee = &m.signatures[*target as usize];
                    for (a, p) in args.iter().zip(&callee.params) {
                        require(*a, shape(*p))?;
                    }
                    out[0] = shape(callee.ret);
                    // A delegated suspension updates the entry parameter, even
                    // when the call's own return value is discarded.
                    if ci == entry
                        && chunk.block_type == BlockType::Stream
                        && chunk.param_count > 0
                        && m.chunks[*target as usize].block_type != BlockType::Func
                    {
                        state.locals[0] |= reply;
                    }
                }
                Op::Return => {
                    require(*state.stack.last().ok_or_else(fail)?, shape(sig.ret))?;
                }
                Op::CheckedAdd
                | Op::CheckedSub
                | Op::CheckedMul(_)
                | Op::CheckedDiv(_)
                | Op::CheckedMod
                | Op::CheckedNeg => {
                    let allowed = if matches!(op, Op::CheckedMod) {
                        WORD | BYTE | FIXED
                    } else {
                        WORD | BYTE | FIXED | FLOAT
                    };
                    let k = same(allowed)?;
                    // Native checked multiply/divide select their Word/Byte
                    // or Fixed implementation using this baked fraction count.
                    if let Op::CheckedMul(frac) | Op::CheckedDiv(frac) = op {
                        require(k, if *frac == 0 { WORD | BYTE } else { FIXED })?;
                    }
                    out.copy_from_slice(&[k, k, WORD]);
                }
                Op::Add | Op::Sub | Op::Mul | Op::Neg => out[0] = same(BYTE | FIXED | FLOAT)?,
                Op::Div | Op::Mod => out[0] = same(WORD | BYTE | FLOAT)?,
                Op::CmpEq | Op::CmpNe | Op::CmpLt | Op::CmpGt | Op::CmpLe | Op::CmpGe => {
                    same(UNIT | BOOL | BYTE | WORD | FIXED | FLOAT)?;
                    out[0] = BOOL;
                }
                Op::Not => {
                    require(args[0], BOOL)?;
                    out[0] = if args[0] & TRUE != 0 { FALSE } else { 0 }
                        | if args[0] & FALSE != 0 { TRUE } else { 0 };
                }
                Op::BitAnd | Op::BitOr | Op::BitXor => out[0] = same(WORD)?,
                Op::Shl | Op::Shr => {
                    require(args[0], WORD)?;
                    require(args[1], WORD)?;
                    out[0] = args[0];
                }
                Op::FixedMul(_) | Op::FixedDiv(_) => {
                    same(FIXED)?;
                    out[0] = FIXED;
                }
                Op::FixedToWord(_) => {
                    require(args[0], FIXED)?;
                    out[0] = WORD;
                }
                Op::WordToFixed(_) => {
                    require(args[0], WORD)?;
                    out[0] = FIXED;
                }
                Op::IntToFloat => {
                    require(args[0], WORD)?;
                    out[0] = FLOAT;
                }
                Op::FloatToInt => {
                    require(args[0], FLOAT)?;
                    out[0] = WORD;
                }
                Op::WordToByte => {
                    require(args[0], WORD)?;
                    out[0] = BYTE;
                }
                Op::ByteToWord => {
                    require(args[0], BYTE)?;
                    out[0] = WORD;
                }
                Op::BoundsCheck(_) => {
                    require(args[0], WORD)?;
                    out[0] = WORD;
                }
                Op::GetData(i)
                | Op::SetData(i)
                | Op::GetDataIndexed(i, _)
                | Op::SetDataIndexed(i, _) => {
                    let count = match op {
                        Op::GetDataIndexed(_, n) | Op::SetDataIndexed(_, n) => *n,
                        _ => 1,
                    };
                    let end = i.checked_add(count).ok_or_else(fail)?;
                    if end as usize > m.data_layout.as_ref().map_or(0, |d| d.slots.len()) {
                        return Err(fail());
                    }
                    let mut kinds = 0;
                    for s in *i..end {
                        kinds |= slot(m, s);
                    }
                    if matches!(op, Op::GetDataIndexed(..) | Op::SetDataIndexed(..)) {
                        require(*args.last().ok_or_else(fail)?, WORD)?;
                    }
                    if out.is_empty() {
                        require(args[0], kinds)?;
                    } else {
                        out[0] = kinds;
                    }
                }
                Op::NewComposite(c) => out[0] = body(c.kind().to_tag()),
                Op::GetField(keleusma::bytecode::StructField::Flat { kind, .. })
                | Op::GetTupleField(keleusma::bytecode::TupleField::Flat { kind, .. })
                | Op::GetEnumField(keleusma::bytecode::EnumField::Flat { kind, .. }) => {
                    require(args[0], BODY)?;
                    out[0] = scalar(kind.to_tag());
                }
                Op::GetField(keleusma::bytecode::StructField::FlatNested { variant, .. })
                | Op::GetTupleField(keleusma::bytecode::TupleField::FlatNested {
                    variant, ..
                })
                | Op::GetEnumField(keleusma::bytecode::EnumField::FlatNested { variant, .. }) => {
                    require(args[0], BODY)?;
                    out[0] = body(variant.to_tag());
                }
                Op::GetField(_) | Op::GetTupleField(_) | Op::GetEnumField(_) => return Err(fail()),
                Op::GetIndex(kind) => {
                    require(args[0], BODY)?;
                    require(args[1], WORD)?;
                    out[0] = match kind {
                        keleusma::bytecode::ArrayElem::Flat { kind } => scalar(kind.to_tag()),
                        keleusma::bytecode::ArrayElem::FlatNested { variant, .. } => {
                            body(variant.to_tag())
                        }
                        _ => UNKNOWN,
                    };
                }
                Op::IsEnum(..) | Op::IsStruct(..) => {
                    let inspected = if matches!(op, Op::IsEnum(..)) {
                        ENUM
                    } else {
                        STRUCT
                    };
                    out.copy_from_slice(&[
                        args[0],
                        if args[0] & inspected == 0 {
                            FALSE
                        } else {
                            BOOL
                        },
                    ]);
                }
                Op::CallVerifiedNative(i, _) | Op::CallExternalNative(i, _) => {
                    out[0] = m
                        .native_return_shapes
                        .get(*i as usize)
                        .copied()
                        .map(shape)
                        .unwrap_or(UNKNOWN);
                    if out.len() == 2 {
                        out[1] = WORD;
                    }
                }
                Op::If(_) | Op::BreakIf(_) => require(args[0], BOOL)?,
                Op::Reset => {
                    state.stack.clear();
                    state.locals.fill(UNIT);
                    if ci == entry && chunk.block_type == BlockType::Stream && chunk.param_count > 0
                    {
                        state.locals[0] = reply;
                    }
                }
                Op::Stream
                | Op::Loop(_)
                | Op::EndLoop(_)
                | Op::Break(_)
                | Op::Else(_)
                | Op::EndIf
                | Op::Trap(_) => {}
                _ => return Err(fail()),
            }
            state.stack.extend(out);
            let mut successors = vec![ip + 1];
            match op {
                Op::Return | Op::Trap(_) => successors.clear(),
                Op::Else(t) | Op::EndLoop(t) | Op::Break(t) => successors = vec![*t as usize],
                Op::If(t) | Op::BreakIf(t) => {
                    let taken = match args[0] {
                        FALSE => Some(false),
                        TRUE => Some(true),
                        _ => None,
                    };
                    let jumps_on_true = matches!(op, Op::BreakIf(_));
                    match taken {
                        Some(value) if value == jumps_on_true => successors = vec![*t as usize],
                        Some(_) => {}
                        None => successors.push(*t as usize),
                    }
                }
                Op::Reset => successors = vec![1],
                _ => {}
            }
            for next in successors {
                if next >= states.len() {
                    continue;
                }
                let changed = if let Some(old) = &mut states[next] {
                    old.join(&state)
                } else {
                    states[next] = Some(state.clone());
                    true
                };
                if changed {
                    work.push_back(next);
                }
            }
        }
        // Only converged states justify folding. An early visit can miss a
        // later loop iteration that brings a different kind to the same site.
        for (ip, state) in states.iter().enumerate() {
            let Some(state) = state else { continue };
            match chunk.ops[ip] {
                Op::IsEnum(..) | Op::IsStruct(..) => {
                    let inspected = if matches!(chunk.ops[ip], Op::IsEnum(..)) {
                        ENUM
                    } else {
                        STRUCT
                    };
                    let actual = *state.stack.last().expect("verified inspection operand");
                    if actual & inspected == 0 {
                        analysis.false_inspections.insert((ci, ip));
                    } else if actual != inspected {
                        return Err(LowerError::UnsupportedShape(
                            "coroutine inspection has mixed or unknown receiver kinds".into(),
                        ));
                    }
                }
                Op::If(_) | Op::BreakIf(_) => {
                    let value = match state.stack.last() {
                        Some(&FALSE) => Some(false),
                        Some(&TRUE) => Some(true),
                        _ => None,
                    };
                    if let Some(value) = value {
                        analysis.branches[ci].insert(ip, value);
                    }
                }
                _ => {}
            }
        }
    }
    Ok(analysis)
}
