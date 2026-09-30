//! Coroutine scalar admission with the actual host reply type.
//!
//! The bytecode verifier deliberately defers unknown reply types to VM guards.
//! Untagged native operands need a stronger admission condition. This pass joins
//! possible tags at control-flow edges, including back edges and Reset. A state
//! can only gain bits, so the finite tag lattice makes the worklist terminate.
//! The native emitter additionally proves composite read and transfer extents.

use crate::{LowerError, Width};
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
    pub(crate) plan: super::dialogue::Plan,
    pub(crate) extents: Vec<BTreeMap<usize, Extents>>,
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
pub(crate) struct Extents {
    pub stack: Vec<Width>,
    pub locals: Vec<Width>,
}
#[derive(Clone)]
struct State {
    extents: Extents,
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
        for (a, b) in self
            .extents
            .stack
            .iter_mut()
            .chain(&mut self.extents.locals)
            .zip(other.extents.stack.iter().chain(&other.extents.locals))
        {
            if *a != *b && *a != Width::Unknown {
                *a = Width::Unknown;
                changed = true;
            }
        }
        changed
    }
}
fn slot(m: &Module, index: u32, private_kinds: &[u16]) -> u16 {
    let Some(d) = &m.data_layout else {
        return UNKNOWN;
    };
    if let Some(s) = d.shared_layout.get(index as usize) {
        return if s.kind < 8 {
            scalar(s.kind)
        } else {
            body(s.kind & !keleusma::bytecode::SHARED_SLOT_COMPOSITE_FLAG)
        };
    }
    if d.private_composite_layout
        .iter()
        .any(|s| u32::from(s.slot) == index)
    {
        return private_kinds[index as usize];
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

fn scalar_width(mask: u16, float_bytes: u32) -> Width {
    match kind(mask) {
        UNIT => Width::Scalar(0),
        FALSE | BYTE => Width::Scalar(1),
        WORD | FIXED => Width::Scalar(8),
        FLOAT => Width::Scalar(float_bytes),
        _ => Width::Unknown,
    }
}

fn data_width(m: &Module, slot_index: u32, float_bytes: u32, private_kinds: &[u16]) -> Width {
    let Some(layout) = &m.data_layout else {
        return Width::Unknown;
    };
    if let Some(field) = layout
        .private_composite_layout
        .iter()
        .find(|f| u32::from(f.slot) == slot_index)
    {
        // The emitter validates the pool partition before using this extent.
        let end = layout
            .private_composite_layout
            .iter()
            .map(|f| f.offset)
            .filter(|offset| *offset > field.offset)
            .min()
            .unwrap_or(m.persistent_composite_bytes);
        return end
            .checked_sub(field.offset)
            .map_or(Width::Unknown, Width::Body);
    }
    scalar_width(slot(m, slot_index, private_kinds), float_bytes)
}

pub(super) fn check(
    m: &Module,
    entry: usize,
    plan: super::dialogue::Plan,
) -> Result<Analysis, LowerError> {
    for (index, chunk) in m.chunks.iter().enumerate() {
        if !plan.reachable[index] {
            continue;
        }
        let signature = m.signatures.get(index).ok_or_else(|| {
            LowerError::UnsupportedShape(
                "coroutine scalar admission requires complete signatures".into(),
            )
        })?;
        if chunk.ops.is_empty() || signature.params.len() != usize::from(chunk.param_count) {
            return Err(LowerError::UnsupportedShape(
                "coroutine scalar admission requires complete signatures".into(),
            ));
        }
        for (tag, declared) in chunk.param_types.iter().zip(&signature.params) {
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
        plan,
        extents: vec![BTreeMap::new(); m.chunks.len()],
        false_inspections: BTreeSet::new(),
        branches: vec![BTreeMap::new(); m.chunks.len()],
    };
    // A private body read traps until a write initializes the slot. Infer the
    // union of kinds of every possible write, including through other slots and
    // calls. This is a finite ascending lattice. It does not assume source types
    // which the bytecode data layout does not record.
    let slot_count = m
        .data_layout
        .as_ref()
        .map_or(0, |layout| layout.slots.len());
    let mut private_kinds = vec![0; slot_count];
    loop {
        let writes = analyze(m, entry, &mut analysis, &private_kinds, false)?;
        let mut changed = false;
        for (known, written) in private_kinds.iter_mut().zip(writes) {
            let union = *known | written;
            changed |= union != *known;
            *known = union;
        }
        if !changed {
            break;
        }
    }
    analyze(m, entry, &mut analysis, &private_kinds, true)?;
    Ok(analysis)
}

fn analyze(
    m: &Module,
    entry: usize,
    analysis: &mut Analysis,
    private_kinds: &[u16],
    validate: bool,
) -> Result<Vec<u16>, LowerError> {
    let float_bytes = 1u32 << m.float_bits_log2 >> 3;
    let declared_width =
        |shape: WireShape| crate::width_of_declared_shape(Some(&shape), float_bytes);
    let reply = shape(
        m.signatures[entry]
            .params
            .first()
            .copied()
            .unwrap_or(WireShape::Scalar { kind: 0 }),
    );
    let mut writes = vec![0; private_kinds.len()];
    for (ci, chunk) in m.chunks.iter().enumerate() {
        if !analysis.plan.reachable[ci] {
            continue;
        }
        let sig = &m.signatures[ci];
        let mut initial = State {
            extents: Extents {
                stack: Vec::new(),
                locals: vec![Width::Scalar(0); chunk.local_count as usize],
            },
            stack: Vec::new(),
            locals: vec![UNIT; chunk.local_count as usize],
        };
        for (i, p) in sig.params.iter().enumerate() {
            initial.locals[i] = shape(*p);
            initial.extents.locals[i] = declared_width(*p);
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
                if !validate || (actual != 0 && actual & !allowed == 0) {
                    Ok(())
                } else {
                    Err(fail())
                }
            };
            let (n, delta) = keleusma::verify::op_depth_effect(op, chunk);
            let n = n as usize;
            let start = state.stack.len().checked_sub(n).ok_or_else(fail)?;
            let args = state.stack.split_off(start);
            let argument_widths = state.extents.stack.split_off(start);
            let produced = (n as i32 + delta) as usize;
            let mut out = vec![UNKNOWN; produced];
            let same = |allowed: u16| -> Result<u16, LowerError> {
                let a = kind(args[0]);
                require(a, kind(allowed))?;
                if validate && (a.count_ones() != 1 || args.iter().any(|b| kind(*b) != a)) {
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
                    let contract = analysis.plan.sites[ci][&ip].1;
                    let reply = shape(contract.reply);
                    require(args[0], shape(contract.yielded))?;
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
                        kinds |= slot(m, s, private_kinds);
                    }
                    if matches!(op, Op::GetDataIndexed(..) | Op::SetDataIndexed(..)) {
                        require(*args.last().ok_or_else(fail)?, WORD)?;
                    }
                    if out.is_empty() {
                        for slot_index in *i..end {
                            if m.data_layout
                                .as_ref()
                                .unwrap()
                                .private_composite_layout
                                .iter()
                                .any(|field| u32::from(field.slot) == slot_index)
                            {
                                require(args[0], BODY)?;
                                writes[slot_index as usize] |= args[0];
                            } else {
                                require(args[0], slot(m, slot_index, private_kinds))?;
                            }
                        }
                    } else {
                        out[0] = kinds;
                    }
                }
                Op::NewComposite(c) => out[0] = body(c.kind().to_tag()),
                Op::GetField(keleusma::bytecode::StructField::Flat { kind, .. })
                | Op::GetTupleField(keleusma::bytecode::TupleField::Flat { kind, .. })
                | Op::GetEnumField(keleusma::bytecode::EnumField::Flat { kind, .. }) => {
                    require(
                        args[0],
                        match op {
                            Op::GetField(_) => STRUCT,
                            Op::GetTupleField(_) => TUPLE,
                            _ => ENUM,
                        },
                    )?;
                    out[0] = scalar(kind.to_tag());
                }
                Op::GetField(keleusma::bytecode::StructField::FlatNested { variant, .. })
                | Op::GetTupleField(keleusma::bytecode::TupleField::FlatNested {
                    variant, ..
                })
                | Op::GetEnumField(keleusma::bytecode::EnumField::FlatNested { variant, .. }) => {
                    require(
                        args[0],
                        match op {
                            Op::GetField(_) => STRUCT,
                            Op::GetTupleField(_) => TUPLE,
                            _ => ENUM,
                        },
                    )?;
                    out[0] = body(variant.to_tag());
                }
                Op::GetField(_) | Op::GetTupleField(_) | Op::GetEnumField(_) => return Err(fail()),
                Op::GetIndex(kind) => {
                    require(args[0], ARRAY)?;
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
                        if args[0] == 0 {
                            0
                        } else if args[0] & inspected == 0 {
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
            let mut widths: Vec<_> = out
                .iter()
                .map(|kind| scalar_width(*kind, float_bytes))
                .collect();
            match op {
                Op::GetLocal(index) => widths[0] = state.extents.locals[*index as usize],
                Op::SetLocal(index) => state.extents.locals[*index as usize] = argument_widths[0],
                Op::Dup => widths.fill(argument_widths[0]),
                Op::Yield => {
                    widths[0] = declared_width(analysis.plan.sites[ci][&ip].1.reply);
                    if ci == entry && chunk.block_type == BlockType::Stream && chunk.param_count > 0
                    {
                        state.extents.locals[0] = widths[0];
                    }
                }
                Op::Call(target, _) => {
                    widths[0] = declared_width(m.signatures[*target as usize].ret);
                    if ci == entry
                        && chunk.block_type == BlockType::Stream
                        && chunk.param_count > 0
                        && m.chunks[*target as usize].block_type != BlockType::Func
                    {
                        let reply = declared_width(m.signatures[entry].params[0]);
                        if state.extents.locals[0] != reply {
                            state.extents.locals[0] = Width::Unknown;
                        }
                    }
                }
                Op::CallVerifiedNative(index, _) | Op::CallExternalNative(index, _) => {
                    widths[0] = m
                        .native_return_shapes
                        .get(*index as usize)
                        .copied()
                        .map_or(Width::Unknown, declared_width);
                }
                Op::NewComposite(keleusma::bytecode::NewCompositeOperand::Flat {
                    byte_size,
                    ..
                }) => widths[0] = Width::Body(u32::from(*byte_size)),
                Op::GetField(keleusma::bytecode::StructField::FlatNested { size, .. })
                | Op::GetTupleField(keleusma::bytecode::TupleField::FlatNested { size, .. })
                | Op::GetEnumField(keleusma::bytecode::EnumField::FlatNested { size, .. })
                | Op::GetIndex(keleusma::bytecode::ArrayElem::FlatNested { size, .. }) => {
                    widths[0] = Width::Body(u32::from(*size))
                }
                Op::IsEnum(..) | Op::IsStruct(..) => widths[0] = argument_widths[0],
                Op::GetData(index) => widths[0] = data_width(m, *index, float_bytes, private_kinds),
                Op::GetDataIndexed(index, count) => {
                    let first = data_width(m, *index, float_bytes, private_kinds);
                    widths[0] = if (*index..index + count)
                        .all(|index| data_width(m, index, float_bytes, private_kinds) == first)
                    {
                        first
                    } else {
                        Width::Unknown
                    };
                }
                Op::Reset => {
                    state.extents.stack.clear();
                    state.extents.locals.fill(Width::Scalar(0));
                    if ci == entry && chunk.block_type == BlockType::Stream && chunk.param_count > 0
                    {
                        state.extents.locals[0] = declared_width(m.signatures[entry].params[0]);
                    }
                }
                _ => {}
            }
            state.extents.stack.extend(widths);
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
        if !validate {
            continue;
        }
        // Only converged states justify folding. An early visit can miss a
        // later loop iteration that brings a different kind to the same site.
        for (ip, state) in states.iter().enumerate() {
            let Some(state) = state else { continue };
            analysis.extents[ci].insert(ip, state.extents.clone());
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
    Ok(writes)
}
