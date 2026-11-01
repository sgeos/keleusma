//! Run grouping and record splitting are selected by Keleusma requests.
use super::wire_stream_auxiliary::Machine;
use super::*;
use crate::bytecode::{DataSlot, SharedSlotLayout, SlotVisibility};

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire run encoding: {detail}"),
    }
}
fn size(n: usize) -> Result<i64, SelfHostError> {
    i64::try_from(n).map_err(|_| error("size exceeds transport word"))
}
fn index(n: i64) -> Result<usize, SelfHostError> {
    usize::try_from(n).map_err(|_| error("invalid response index"))
}
fn visibility(slot: &DataSlot) -> i64 {
    match slot.visibility {
        SlotVisibility::Shared => 0,
        SlotVisibility::Private => 1,
    }
}
fn slice<T>(values: &[T], first: i64, count: i64) -> Result<&[T], SelfHostError> {
    let first = index(first)?;
    let end = first
        .checked_add(index(count)?)
        .ok_or_else(|| error("response range overflow"))?;
    values
        .get(first..end)
        .ok_or_else(|| error("stage requested an unavailable range"))
}
/// A stage-selected group and the record sizes chosen by its splitter.
pub(super) struct SlotRun {
    pub(super) first: usize,
    pub(super) visibility: i64,
    pub(super) pieces: Vec<i64>,
}

pub(super) fn slot_runs(
    schema: &mut Machine<'_>,
    slots: &[DataSlot],
) -> Result<Vec<SlotRun>, SelfHostError> {
    // Every comparison window can be charged to an input name. The multiplier
    // covers bounded lookahead replay when a group ends within a batch.
    let budget = slots
        .iter()
        .try_fold(4usize, |budget, s| {
            (s.name.len() / 64 + 1)
                .checked_mul(8)
                .and_then(|n| n.checked_add(14))
                .and_then(|n| budget.checked_add(n))
        })
        .ok_or_else(|| error("data-run request budget overflow"))?;
    schema.args(&[size(slots.len())?])?;
    let mut event = schema.command(89)?;
    let mut groups: Vec<SlotRun> = Vec::new();
    let mut remaining = 0usize;
    let mut next_first = 0usize;
    for _ in 0..budget {
        match event {
            0 => {
                if remaining != 0
                    || next_first != slots.len()
                    || index(schema.output::<1>()?[0])? != groups.len()
                {
                    return Err(error("data-run completion disagrees"));
                }
                return Ok(groups);
            }
            1 => {
                let at = index(schema.output::<1>()?[0])?;
                if at != next_first || remaining != 0 {
                    return Err(error("group heads did not advance monotonically"));
                }
                let slot = slots.get(at).ok_or_else(|| error("missing group head"))?;
                schema.args(&[size(slot.name.len())?, visibility(slot)])?;
                event = schema.command(90)?;
            }
            2 => {
                let [first, count] = schema.output::<2>()?;
                let batch = slice(slots, first, count)?;
                if batch.is_empty() || batch.len() > 8 {
                    return Err(error("invalid data metadata batch"));
                }
                let mut fields = [0; 16];
                for (i, s) in batch.iter().enumerate() {
                    fields[i * 2] = size(s.name.len())?;
                    fields[i * 2 + 1] = visibility(s);
                }
                schema.fields(&fields[..batch.len() * 2])?;
                schema.args(&[size(batch.len())?])?;
                event = schema.command(91)?;
            }
            3 => {
                let [head, first, count, offset, width] = schema.output::<5>()?;
                if !(1..=8).contains(&count) || !(1..=64).contains(&width) {
                    return Err(error("invalid name comparison window"));
                }
                let reference = slots
                    .get(index(head)?)
                    .ok_or_else(|| error("missing reference name"))?;
                schema.write(0, slice(reference.name.as_bytes(), offset, width)?)?;
                for (i, slot) in slice(slots, first, count)?.iter().enumerate() {
                    schema.write((i + 1) * 64, slice(slot.name.as_bytes(), offset, width)?)?;
                }
                schema.args(&[width])?;
                event = schema.command(92)?;
            }
            4 => {
                let [first, count, vis, ordinal] = schema.output::<4>()?;
                let first = index(first)?;
                let count = index(count)?;
                if remaining != 0
                    || first != next_first
                    || count == 0
                    || index(ordinal)? != groups.len()
                    || !matches!(vis, 1 | 2)
                {
                    return Err(error("invalid group descriptor"));
                }
                next_first = first
                    .checked_add(count)
                    .filter(|&n| n <= slots.len())
                    .ok_or_else(|| error("group exceeds input"))?;
                remaining = count;
                groups
                    .try_reserve(1)
                    .map_err(|_| error("group allocation failed"))?;
                groups.push(SlotRun {
                    first,
                    visibility: vis,
                    pieces: Vec::new(),
                });
                event = schema.command(93)?;
            }
            5 => {
                let take = schema.output::<1>()?[0];
                if !(1..=65535).contains(&take) || index(take)? > remaining {
                    return Err(error("invalid data-run piece"));
                }
                let group = groups
                    .last_mut()
                    .ok_or_else(|| error("piece arrived without a group"))?;
                group
                    .pieces
                    .try_reserve(1)
                    .map_err(|_| error("piece allocation failed"))?;
                group.pieces.push(take);
                remaining -= index(take)?;
                event = schema.command(93)?;
            }
            _ => return Err(error("unknown data-run request")),
        }
    }
    Err(error("data-run request budget exhausted"))
}

pub(super) fn shared_runs(
    schema: &mut Machine<'_>,
    slots: &[SharedSlotLayout],
) -> Result<Vec<[i64; 7]>, SelfHostError> {
    let budget = slots
        .len()
        .checked_mul(4)
        .and_then(|n| n.checked_add(4))
        .ok_or_else(|| error("shared-run request budget overflow"))?;
    schema.args(&[size(slots.len())?])?;
    let mut event = schema.command(94)?;
    let mut rows = Vec::new();
    let mut emitted = 0usize;
    for _ in 0..budget {
        match event {
            0 => {
                if emitted != slots.len() {
                    return Err(error("shared-run completion disagrees"));
                }
                return Ok(rows);
            }
            1 => {
                let at = index(schema.output::<1>()?[0])?;
                if at != emitted {
                    return Err(error("shared heads did not advance monotonically"));
                }
                let s = slots.get(at).ok_or_else(|| error("missing shared head"))?;
                schema.args(&[s.offset.into(), s.kind.into(), s.len.into()])?;
                event = schema.command(95)?;
            }
            2 => {
                let [first, count] = schema.output::<2>()?;
                let batch = slice(slots, first, count)?;
                if batch.is_empty() || batch.len() > 5 {
                    return Err(error("invalid shared metadata batch"));
                }
                let mut fields = [0; 15];
                for (i, s) in batch.iter().enumerate() {
                    fields[i * 3] = s.offset.into();
                    fields[i * 3 + 1] = s.kind.into();
                    fields[i * 3 + 2] = s.len.into();
                }
                schema.fields(&fields[..batch.len() * 3])?;
                schema.args(&[size(batch.len())?])?;
                event = schema.command(96)?;
            }
            3 => {
                let row = schema.output::<7>()?;
                if index(row[4])? != emitted || !(1..=65535).contains(&row[5]) {
                    return Err(error("invalid shared run descriptor"));
                }
                emitted = emitted
                    .checked_add(index(row[5])?)
                    .filter(|&n| n <= slots.len())
                    .ok_or_else(|| error("shared record exceeds input"))?;
                rows.try_reserve(1)
                    .map_err(|_| error("shared record allocation failed"))?;
                rows.push(row);
                event = schema.command(97)?;
            }
            _ => return Err(error("unknown shared-run request")),
        }
    }
    Err(error("shared-run request budget exhausted"))
}
