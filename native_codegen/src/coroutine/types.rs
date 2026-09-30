//! Coroutine scalar admission with the actual host reply type.
//!
//! The bytecode verifier deliberately defers unknown reply types to VM guards.
//! Untagged native operands need a stronger admission condition. This pass joins
//! possible tags at control-flow edges, including back edges and Reset. A state
//! can only gain bits, so the finite tag lattice makes the worklist terminate.
//! Body extents have finite producer-derived endpoints. Joins widen these
//! ranges and discard conflicting predicate provenance. A successful enum test
//! refines its unchanged receiver binding. Writes invalidate that provenance.
//! The emitter tracks dynamic kinds and lengths where joined facts need them.

use crate::{LowerError, Width};
use keleusma::bytecode::{BlockType, ConstValue, Module, Op, TypeTag, WireShape};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(crate) const UNIT: u16 = 1;
const FALSE: u16 = 2;
const TRUE: u16 = 4096;
const BOOL: u16 = FALSE | TRUE;
const BYTE: u16 = 4;
const WORD: u16 = 8;
const FIXED: u16 = 16;
pub(crate) const FLOAT: u16 = 32;
pub(crate) const TUPLE: u16 = 256;
pub(crate) const ARRAY: u16 = 512;
pub(crate) const STRUCT: u16 = 1024;
pub(crate) const ENUM: u16 = 2048;
const BODY: u16 = TUPLE | ARRAY | STRUCT | ENUM;
const UNKNOWN: u16 = 8191;

pub(crate) struct Analysis {
    pub(crate) plan: super::dialogue::Plan,
    pub(crate) facts: Vec<BTreeMap<usize, Facts>>,
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
pub(crate) fn kind(mask: u16) -> u16 {
    (mask & !TRUE) | if mask & TRUE != 0 { FALSE } else { 0 }
}

pub(crate) fn operand_kind(mask: u16) -> crate::OperandKind {
    match kind(mask) {
        FLOAT => crate::OperandKind::Float,
        FIXED => crate::OperandKind::Fixed,
        one if one.count_ones() == 1 => crate::OperandKind::Int,
        _ => crate::OperandKind::Unknown,
    }
}

pub(crate) fn guarded_width(mask: u16, flow: &Flow, float_bytes: u32) -> Width {
    match kind(mask) {
        UNIT => Width::Scalar(0),
        FALSE | BYTE => Width::Scalar(1),
        WORD | FIXED => Width::Scalar(8),
        FLOAT => Width::Scalar(float_bytes),
        one if one.count_ones() == 1 && one & BODY != 0 => flow.bodies
            [(one.trailing_zeros() - 8) as usize]
            .exact()
            .map_or(Width::Unknown, Width::Body),
        _ => Width::Unknown,
    }
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
/// Body size evidence survives joins with values that are not bodies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum BodyExtent {
    #[default]
    Absent,
    Range {
        min: u32,
        max: u32,
    },
    Unknown,
}
impl BodyExtent {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Absent, b) => b,
            (a, Self::Absent) => a,
            (Self::Range { min: a, max: b }, Self::Range { min: c, max: d }) => Self::Range {
                min: a.min(c),
                max: b.max(d),
            },
            _ => Self::Unknown,
        }
    }
    pub fn minimum(self) -> Option<u32> {
        match self {
            Self::Range { min, .. } => Some(min),
            _ => None,
        }
    }
    pub fn exact(self) -> Option<u32> {
        match self {
            Self::Range { min, max } if min == max => Some(min),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Predicate {
    receiver: usize,
    when: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Flow {
    pub bodies: [BodyExtent; 4],
    pub owned: bool,
    origin: Option<usize>,
    predicate: Option<Predicate>,
}
impl Flow {
    fn new(mask: u16, width: Width, owned: bool) -> Self {
        let mut value = Self {
            owned,
            ..Self::default()
        };
        for (index, extent) in value.bodies.iter_mut().enumerate() {
            if mask & (TUPLE << index) != 0 {
                *extent = match width {
                    Width::Body(n) => BodyExtent::Range { min: n, max: n },
                    _ => BodyExtent::Unknown,
                };
            }
        }
        value
    }
    pub fn minimum(&self, mask: u16) -> Option<u32> {
        let index = match mask {
            TUPLE => 0,
            ARRAY => 1,
            STRUCT => 2,
            ENUM => 3,
            _ => return None,
        };
        self.bodies[index].minimum()
    }
    pub fn maximum(&self) -> Option<u32> {
        let mut maximum = 0;
        for extent in self.bodies {
            match extent {
                BodyExtent::Absent => {}
                BodyExtent::Range { max, .. } => maximum = maximum.max(max),
                BodyExtent::Unknown => return None,
            }
        }
        Some(maximum)
    }
    fn join(&mut self, other: Self) -> bool {
        let old = *self;
        for (a, b) in self.bodies.iter_mut().zip(other.bodies) {
            *a = a.join(b);
        }
        self.owned |= other.owned;
        if self.origin != other.origin {
            self.origin = None;
        }
        if self.predicate != other.predicate {
            self.predicate = None;
        }
        *self != old
    }
    fn invalidate(&mut self, local: usize) {
        if self.origin == Some(local) {
            self.origin = None;
        }
        if self.predicate.is_some_and(|p| p.receiver == local) {
            self.predicate = None;
        }
    }
}
#[derive(Clone)]
pub(crate) struct Output {
    pub slot: usize,
    pub tag: u16,
    pub width: Width,
    pub flow: Flow,
}
#[derive(Clone)]
pub(crate) struct Facts {
    pub extents: Extents,
    pub stack: Vec<u16>,
    pub locals: Vec<u16>,
    pub flow_stack: Vec<Flow>,
    pub flow_locals: Vec<Flow>,
    pub outputs: Vec<Output>,
    pub guards: BTreeMap<usize, u16>,
}
impl Facts {
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
        for (a, b) in self
            .flow_stack
            .iter_mut()
            .chain(&mut self.flow_locals)
            .zip(other.flow_stack.iter().chain(&other.flow_locals))
        {
            changed |= a.join(*b);
        }
        changed
    }
    fn invalidate(&mut self, local: usize) {
        for flow in self.flow_stack.iter_mut().chain(&mut self.flow_locals) {
            flow.invalidate(local);
        }
    }
    fn narrow_enum(&mut self, local: usize) -> bool {
        if self.locals[local] & ENUM == 0 {
            return false;
        }
        self.locals[local] = ENUM;
        self.extents.locals[local] = self.flow_locals[local].bodies[3]
            .exact()
            .map_or(Width::Unknown, Width::Body);
        for (index, flow) in self.flow_stack.iter().enumerate() {
            if flow.origin == Some(local) {
                self.stack[index] = ENUM;
                self.extents.stack[index] =
                    flow.bodies[3].exact().map_or(Width::Unknown, Width::Body);
            }
        }
        true
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
        facts: vec![BTreeMap::new(); m.chunks.len()],
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
        let mut initial = Facts {
            extents: Extents {
                stack: Vec::new(),
                locals: vec![Width::Scalar(0); chunk.local_count as usize],
            },
            stack: Vec::new(),
            locals: vec![UNIT; chunk.local_count as usize],
            flow_stack: Vec::new(),
            outputs: Vec::new(),
            guards: BTreeMap::new(),
            flow_locals: vec![Flow::default(); chunk.local_count as usize],
        };
        for (i, p) in sig.params.iter().enumerate() {
            initial.locals[i] = shape(*p);
            initial.extents.locals[i] = declared_width(*p);
            initial.flow_locals[i] = Flow::new(
                shape(*p),
                declared_width(*p),
                analysis.plan.owns_bodies && matches!(p, WireShape::Flat { .. }),
            );
        }
        let mut states: Vec<Option<Facts>> = vec![None; chunk.ops.len()];
        states[0] = Some(initial);
        let mut work = VecDeque::from([0]);
        let mut outputs = BTreeMap::new();
        let mut guards = BTreeMap::new();
        while let Some(ip) = work.pop_front() {
            let mut state = states[ip].clone().unwrap();
            let op = &chunk.ops[ip];
            let fail = || {
                LowerError::UnsupportedShape(format!(
                    "coroutine scalar types are not proven in {} at {ip}: {op:?}",
                    chunk.name
                ))
            };
            let checks = std::cell::RefCell::new(BTreeMap::new());
            let require = |actual: u16, allowed: u16, index: Option<usize>| {
                if !validate || (actual != 0 && actual & !allowed == 0) {
                    return Ok(());
                }
                // Reset can add Unit to an otherwise proven kind. Check that
                // alternative at the consuming operation, not at GetLocal,
                // because inspecting Unit as an enum is a valid false result.
                let narrowed = actual & !UNIT;
                if actual & UNIT != 0
                    && narrowed != 0
                    && narrowed & !allowed == 0
                    && let Some(index) = index
                {
                    checks
                        .borrow_mut()
                        .entry(index)
                        .and_modify(|mask| *mask &= narrowed)
                        .or_insert(narrowed);
                    return Ok(());
                }
                Err(fail())
            };
            let (n, delta) = keleusma::verify::op_depth_effect(op, chunk);
            let n = n as usize;
            let start = state.stack.len().checked_sub(n).ok_or_else(fail)?;
            let args = state.stack.split_off(start);
            let argument_widths = state.extents.stack.split_off(start);
            let argument_flows = state.flow_stack.split_off(start);
            let produced = (n as i32 + delta) as usize;
            let mut out = vec![UNKNOWN; produced];
            let require_arg =
                |index: usize, allowed: u16| require(args[index], allowed, Some(start + index));
            let same = |allowed: u16| -> Result<u16, LowerError> {
                let common = args
                    .iter()
                    .fold(kind(allowed), |mask, actual| mask & kind(*actual));
                if common.count_ones() != 1 {
                    if validate {
                        return Err(fail());
                    }
                    return Ok(kind(args[0]));
                }
                for (index, actual) in args.iter().enumerate() {
                    require(kind(*actual), common, Some(start + index))?;
                }
                Ok(common)
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
                    require_arg(0, shape(contract.yielded))?;
                    out[0] = reply;
                    if ci == entry && chunk.block_type == BlockType::Stream && chunk.param_count > 0
                    {
                        state.locals[0] = reply;
                    }
                }
                Op::Call(target, _) => {
                    let callee = &m.signatures[*target as usize];
                    for (index, p) in callee.params.iter().enumerate() {
                        require_arg(index, shape(*p))?;
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
                    require(
                        *state.stack.last().ok_or_else(fail)?,
                        shape(sig.ret),
                        state.stack.len().checked_sub(1),
                    )?;
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
                        require(k, if *frac == 0 { WORD | BYTE } else { FIXED }, None)?;
                    }
                    out.copy_from_slice(&[k, k, WORD]);
                }
                Op::Add | Op::Sub | Op::Mul | Op::Neg => out[0] = same(BYTE | FIXED | FLOAT)?,
                Op::Div | Op::Mod => out[0] = same(WORD | BYTE | FLOAT)?,
                Op::CmpEq | Op::CmpNe => {
                    // PartialEq accepts unlike kinds and Unit. It must not
                    // acquire the consuming type guards used by arithmetic.
                    for index in 0..2 {
                        require_arg(
                            index,
                            UNIT | BOOL
                                | BYTE
                                | WORD
                                | FIXED
                                | FLOAT
                                | TUPLE
                                | ARRAY
                                | STRUCT
                                | ENUM,
                        )?;
                    }
                    out[0] = BOOL;
                }
                Op::CmpLt | Op::CmpGt | Op::CmpLe | Op::CmpGe => {
                    same(BYTE | WORD | FIXED | FLOAT)?;
                    out[0] = BOOL;
                }
                Op::Not => {
                    require_arg(0, BOOL)?;
                    out[0] = if args[0] & TRUE != 0 { FALSE } else { 0 }
                        | if args[0] & FALSE != 0 { TRUE } else { 0 };
                }
                Op::BitAnd | Op::BitOr | Op::BitXor => out[0] = same(WORD)?,
                Op::Shl | Op::Shr => {
                    require_arg(0, WORD)?;
                    require_arg(1, WORD)?;
                    out[0] = args[0];
                }
                Op::FixedMul(_) | Op::FixedDiv(_) => {
                    same(FIXED)?;
                    out[0] = FIXED;
                }
                Op::FixedToWord(_) => {
                    require_arg(0, FIXED)?;
                    out[0] = WORD;
                }
                Op::WordToFixed(_) => {
                    require_arg(0, WORD)?;
                    out[0] = FIXED;
                }
                Op::IntToFloat => {
                    require_arg(0, WORD)?;
                    out[0] = FLOAT;
                }
                Op::FloatToInt => {
                    require_arg(0, FLOAT)?;
                    out[0] = WORD;
                }
                Op::WordToByte => {
                    require_arg(0, WORD)?;
                    out[0] = BYTE;
                }
                Op::ByteToWord => {
                    require_arg(0, BYTE)?;
                    out[0] = WORD;
                }
                Op::BoundsCheck(_) => {
                    require_arg(0, WORD)?;
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
                        require_arg(args.len().checked_sub(1).ok_or_else(fail)?, WORD)?;
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
                                require_arg(0, BODY)?;
                                writes[slot_index as usize] |= args[0];
                            } else {
                                require_arg(0, slot(m, slot_index, private_kinds))?;
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
                    require_arg(
                        0,
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
                    require_arg(
                        0,
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
                    require_arg(0, ARRAY)?;
                    require_arg(1, WORD)?;
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
                Op::If(_) | Op::BreakIf(_) => require_arg(0, BOOL)?,
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
            let mut flows: Vec<_> = out
                .iter()
                .zip(&widths)
                .map(|(tag, width)| Flow::new(*tag, *width, false))
                .collect();
            match op {
                Op::GetLocal(index) => {
                    flows[0] = state.flow_locals[*index as usize];
                    flows[0].origin = Some(*index as usize);
                }
                Op::SetLocal(index) => {
                    state.invalidate(*index as usize);
                    let mut flow = argument_flows[0];
                    flow.invalidate(*index as usize);
                    flow.origin = None;
                    state.flow_locals[*index as usize] = flow;
                }
                Op::Dup => flows.fill(argument_flows[0]),
                Op::Not => {
                    flows[0].predicate = argument_flows[0]
                        .predicate
                        .map(|p| Predicate { when: !p.when, ..p })
                }
                Op::IsEnum(..) | Op::IsStruct(..) => {
                    flows[0] = argument_flows[0];
                    if matches!(op, Op::IsEnum(..)) {
                        flows[1].predicate = argument_flows[0].origin.map(|receiver| Predicate {
                            receiver,
                            when: true,
                        });
                    }
                }
                Op::Yield => {
                    flows[0].owned = matches!(widths[0], Width::Body(_));
                    if ci == entry && chunk.block_type == BlockType::Stream && chunk.param_count > 0
                    {
                        state.invalidate(0);
                        state.flow_locals[0] = flows[0];
                    }
                }
                Op::Call(target, _) => {
                    flows[0].owned =
                        analysis.plan.owns_bodies && matches!(widths[0], Width::Body(_));
                    if ci == entry
                        && chunk.block_type == BlockType::Stream
                        && chunk.param_count > 0
                        && m.chunks[*target as usize].block_type != BlockType::Func
                    {
                        state.invalidate(0);
                        let shape = m.signatures[entry].params[0];
                        let width = declared_width(shape);
                        state.flow_locals[0].join(Flow::new(
                            reply,
                            width,
                            matches!(width, Width::Body(_)),
                        ));
                    }
                }
                Op::CallVerifiedNative(index, _) | Op::CallExternalNative(index, _) => {
                    flows[0].owned = analysis.plan.native_body_returns.get(index)
                        == Some(&super::NativeBodyReturn::Snapshot);
                }
                Op::GetField(keleusma::bytecode::StructField::FlatNested { .. })
                | Op::GetTupleField(keleusma::bytecode::TupleField::FlatNested { .. })
                | Op::GetEnumField(keleusma::bytecode::EnumField::FlatNested { .. })
                | Op::GetIndex(keleusma::bytecode::ArrayElem::FlatNested { .. }) => {
                    flows[0].owned = argument_flows[0].owned
                }
                Op::Reset => {
                    state.flow_stack.clear();
                    state.flow_locals.fill(Flow::default());
                    if ci == entry && chunk.block_type == BlockType::Stream && chunk.param_count > 0
                    {
                        let width = declared_width(m.signatures[entry].params[0]);
                        state.flow_locals[0] =
                            Flow::new(reply, width, matches!(width, Width::Body(_)));
                    }
                }
                _ => {}
            }
            guards.insert(ip, checks.into_inner());
            outputs.insert(
                ip,
                out.iter()
                    .enumerate()
                    .map(|(index, tag)| Output {
                        slot: start + index,
                        tag: *tag,
                        width: widths[index],
                        flow: flows[index],
                    })
                    .collect(),
            );
            state.flow_stack.extend(flows);
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
                let mut outgoing = state.clone();
                if let Op::If(target) | Op::BreakIf(target) = op {
                    let taken = (next == *target as usize) == matches!(op, Op::BreakIf(_));
                    if let Some(predicate) = argument_flows[0].predicate
                        && taken == predicate.when
                        && !outgoing.narrow_enum(predicate.receiver)
                    {
                        continue;
                    }
                }
                if next >= states.len() {
                    continue;
                }
                let changed = if let Some(old) = &mut states[next] {
                    old.join(&outgoing)
                } else {
                    states[next] = Some(outgoing);
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
            let mut facts = state.clone();
            facts.outputs = outputs.remove(&ip).unwrap_or_default();
            facts.guards = guards.remove(&ip).unwrap_or_default();
            analysis.facts[ci].insert(ip, facts);
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
                    } else if matches!(chunk.ops[ip], Op::IsEnum(..)) {
                        if state.flow_stack.last().unwrap().bodies[3]
                            .minimum()
                            .is_none_or(|bytes| bytes < 8)
                        {
                            return Err(LowerError::UnsupportedShape(
                                "coroutine body extent does not prove an enum discriminant".into(),
                            ));
                        }
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
