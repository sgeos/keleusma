//! Text-size facts are stored by the host; transition and heap decisions run in Keleusma.
use super::analysis_machine::{AnalysisMachine, error};
use super::{ChunkTextBounds, VerificationError};
use crate::bytecode::{Chunk, ConstValue, Op};
use alloc::{vec, vec::Vec};

pub(super) const FIELDS: [&str; 15] = [
    "wa.tf_command",
    "wa.tf_kind",
    "wa.tf_required",
    "wa.tf_delta",
    "wa.tf_callee",
    "wa.tf_callee_known",
    "wa.tf_index",
    "wa.tf_locals",
    "wa.tf_pops",
    "wa.tf_pushes",
    "wa.tf_write",
    "wa.tb_tag",
    "wa.tb_hi",
    "wa.tb_lo",
    "wa.tb_heap_hi",
];
// The heap low limb immediately follows the high limb in the stage layout.
#[derive(Clone, Copy, Default)]
struct Bound {
    tag: i64,
    hi: i64,
    lo: i64,
}
impl Bound {
    fn literal(value: Option<&ConstValue>) -> Self {
        match value {
            Some(ConstValue::StaticStr(text)) => {
                let n = u32::try_from(text.len())
                    .unwrap_or(u32::MAX - 1)
                    .min(u32::MAX - 1);
                Self {
                    tag: 1,
                    hi: i64::from(n >> 16),
                    lo: i64::from(n & 65535),
                }
            }
            _ => Self::default(),
        }
    }
    fn write(self, m: &mut AnalysisMachine<'_, 15>, index: usize) -> Result<(), VerificationError> {
        for (field, value) in [(11, self.tag), (12, self.hi), (13, self.lo)] {
            m.set_element(field, index, value)?;
        }
        Ok(())
    }
    fn read(m: &AnalysisMachine<'_, 15>, index: usize) -> Result<Self, VerificationError> {
        let value = Self {
            tag: m.get_element(11, index)?,
            hi: m.get_element(12, index)?,
            lo: m.get_element(13, index)?,
        };
        if !(0..=2).contains(&value.tag)
            || !(0..=65535).contains(&value.hi)
            || !(0..=65535).contains(&value.lo)
        {
            return Err(error("invalid bound"));
        }
        Ok(value)
    }
}

pub(super) fn chunk_bounds(
    machine: &mut AnalysisMachine<'_, 15>,
    chunk: &Chunk,
    mut callee: impl FnMut(usize) -> Result<Option<bool>, VerificationError>,
) -> Result<ChunkTextBounds, VerificationError> {
    if machine.command(20)? != 0 {
        return Err(error("text reset refused"));
    }
    machine.set(7, i64::from(chunk.local_count))?;
    let mut locals = vec![Bound::default(); usize::from(chunk.local_count)];
    let mut stack: Vec<Bound> = Vec::new();
    for op in &chunk.ops {
        let (kind, index) = match op {
            Op::Const(i) => (1, usize::from(*i)),
            Op::GetLocal(i) => (2, usize::from(*i)),
            Op::SetLocal(i) => (3, usize::from(*i)),
            Op::Dup => (4, 0),
            Op::Add => (5, 0),
            Op::Call(i, _) => (6, usize::from(*i)),
            Op::CallVerifiedNative(_, n) | Op::CallExternalNative(_, n) => {
                (7, usize::from(*n & 0x7F))
            }
            Op::Return => (8, 0),
            Op::Yield => (9, 0),
            Op::If(_) => (10, 0),
            Op::EndIf => (11, 0),
            Op::Loop(_) => (12, 0),
            Op::EndLoop(_) => (13, 0),
            Op::Trap(_) => (14, 0),
            _ => (0, 0),
        };
        let summary = if kind == 6 { callee(index)? } else { None };
        let (required, delta) = crate::verify::op_depth_effect(op, chunk);
        for (field, value) in [
            (1, kind),
            (2, i64::from(required.max(0))),
            (3, i64::from(delta)),
            (4, i64::from(summary.unwrap_or(false))),
            (5, i64::from(summary.is_some())),
            (6, i64::try_from(index).map_err(error)?),
        ] {
            machine.set(field, value)?;
        }
        stack
            .last()
            .copied()
            .unwrap_or_default()
            .write(machine, 0)?;
        stack
            .len()
            .checked_sub(2)
            .and_then(|i| stack.get(i))
            .copied()
            .unwrap_or_default()
            .write(machine, 1)?;
        locals
            .get(index)
            .copied()
            .unwrap_or_default()
            .write(machine, 2)?;
        Bound::literal(chunk.constants.get(index)).write(machine, 3)?;
        let status = machine.command(21)?;
        if !(0..=1).contains(&status) {
            return Err(error("text transition refused"));
        }
        let pops = usize::try_from(machine.get(8)?).map_err(error)?;
        let pushes = usize::try_from(machine.get(9)?).map_err(error)?;
        if pops > 65535 || pushes > 7 {
            return Err(error("invalid stack transition"));
        }
        let value = Bound::read(machine, 4)?;
        match machine.get(10)? {
            0 => {}
            1 => {
                *locals
                    .get_mut(index)
                    .ok_or_else(|| error("invalid local write"))? = Bound::read(machine, 5)?
            }
            _ => return Err(error("invalid write marker")),
        }
        stack.truncate(stack.len().saturating_sub(pops));
        stack.try_reserve(pushes).map_err(error)?;
        stack.resize(
            stack
                .len()
                .checked_add(pushes)
                .ok_or_else(|| error("stack length overflow"))?,
            value,
        );
        if status == 1 {
            break;
        }
    }
    stack
        .last()
        .copied()
        .unwrap_or_default()
        .write(machine, 0)?;
    let flags = machine.command(22)?;
    if !(0..=3).contains(&flags) {
        return Err(error("invalid text summary"));
    }
    let hi = machine.get(14)?;
    let lo = machine.get_element(14, 1)?;
    if !(0..=65535).contains(&hi) || !(0..=65535).contains(&lo) {
        return Err(error("invalid heap limbs"));
    }
    Ok(ChunkTextBounds {
        heap_bytes: ((hi as u32) << 16) | (lo as u32),
        returns_text: flags & 1 != 0,
        yields_text: flags & 2 != 0,
    })
}
