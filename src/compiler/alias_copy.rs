//! Which private composite reads must be copied before a write can reach them.
//!
//! # The defect this closes
//!
//! A private data slot holding a flat composite keeps its body at a fixed
//! offset in the persistent pool, and reading the slot with `GetData` or
//! `GetDataIndexed` pushes a handle to that body rather than a copy. A later
//! `SetData` or `SetDataIndexed` to the same slot copies the new body over the
//! old one in place. A value read before such a write and used after it
//! therefore observes the written value. A shared slot is unaffected, because
//! its read copies the bytes out of the host buffer.
//!
//! # The rule
//!
//! The operator ruled on 2026-10-31 that the compiler copies a private
//! composite read exactly where a write to an overlapping slot can follow the
//! read while the read value is still live, and that it copies whenever it
//! cannot prove otherwise. The copy is `emit_detach_data_element`, emitted
//! right after the read.
//!
//! The decision is made over the ops of one chunk compiled without any copy.
//! The self-hosted compiler emits the same ops for every program both compile,
//! so it reaches the same decision by running the same rule over its own op
//! buffer, in `codegen.kel`. That is why the rule is stated over ops rather
//! than over either compiler's syntax tree. Its terms are these.
//!
//! - A **view** consumes a composite and pushes a view of part of the same
//!   body. These are `GetField`, `GetTupleField` and `GetEnumField` in their
//!   `FlatNested` forms. A view keeps the alias alive.
//! - A **dead end** consumes the composite and leaves nothing that refers to
//!   its body. These are the `Flat` forms of the same three reads, which push
//!   a scalar, a flat `NewComposite`, which copies the bytes of a nested
//!   element into its new body, and a `SetData` to a slot outside the read's
//!   range, which copies the bytes into another slot or the host buffer.
//! - A **hazard** is an op that may write the read's slots. It is a `SetData`
//!   or `SetDataIndexed` whose slot range overlaps the read's range, any
//!   `Call`, `CallVerifiedNative` or `CallExternalNative`, since a callee may
//!   write private data, and `Yield`, since control leaves the chunk.
//!
//! Starting after the read, the views are skipped. If the next op is a dead
//! end, no copy is needed. If it is `SetLocal(k)`, the value lives in local
//! `k` and the local rule below decides. Anything else, a return, a yield, a
//! call argument, a boxed composite, a comparison, a branch, copies.
//!
//! The local rule copies unless all of the following hold. The `SetLocal(k)`
//! is the only write of `k` in the chunk, and no `GetLocal(k)` precedes it.
//! No forward jump from before the `SetLocal` lands after it and at or before
//! the last use. Every `GetLocal(k)` after it reaches a dead end through
//! views. And no hazard lies between the read and the last use, where a use
//! inside a loop that begins after the `SetLocal` counts as lasting to the
//! end of the outermost such loop, since a later iteration repeats the use
//! after the hazards of an earlier one.
//!
//! # Why the interval is sound
//!
//! Ops execute in increasing position except at a loop back-edge, `EndLoop`,
//! and at the stream `Reset`, and forward jumps only skip. Consider a use of
//! `k` and the latest run of its binding before it. A back-edge to a position
//! at or before the binding cannot lie between the two, because reaching the
//! use again from there means passing the binding, which no jump from before
//! it skips, and which would then be the latest run. Every op run between
//! the binding and the use therefore lies after the binding, either before
//! the use or inside a loop that began after the binding and contains the
//! use, which is exactly the interval with its extension. A hazard between
//! the read and its binding is impossible, since only views lie there.

//!
//! # Limits, which only ever copy more
//!
//! The rule does not follow a value through `Dup`, `IsEnum`, an indexed read
//! of a nested array, a match scrutinee, or a second local, and it treats
//! every call as a write. Each of these copies where a finer analysis might
//! not. A copy is always correct, and costs one `NewComposite` of the
//! element's byte size, which the worst-case memory bound already charges.

use crate::bytecode::{EnumField, NewCompositeOperand, Op, StructField, TupleField};
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

/// One private composite read, recorded on the copy-free compile.
#[derive(Debug, Clone, Copy)]
pub(super) struct Read {
    /// The position of the first op after the read, where the composite is on
    /// top of the operand stack and where a copy would be emitted.
    pub(super) at: usize,
    /// The first data slot the read covers.
    pub(super) lo: u32,
    /// One past the last data slot the read covers.
    pub(super) hi: u32,
}

/// The ordinals of `reads` that need a copy, given the copy-free `ops`.
pub(super) fn plan(ops: &[Op], reads: &[Read]) -> BTreeSet<usize> {
    reads
        .iter()
        .enumerate()
        .filter(|(_, read)| needs_copy(ops, read))
        .map(|(ordinal, _)| ordinal)
        .collect()
}

/// Whether one read needs a copy. See the module documentation.
fn needs_copy(ops: &[Op], read: &Read) -> bool {
    let q = skip_views(ops, read.at);
    match ops.get(q) {
        Some(op) if is_dead_end(op, read) => false,
        Some(Op::SetLocal(k)) => local_needs_copy(ops, read, q, *k),
        _ => true,
    }
}

/// The most views followed from one value. A deeper chain is classified at
/// the op after the last view followed, which is itself a view and so copies.
/// The self-hosted copy pass has the same bound.
const MAX_VIEWS: usize = 16;

/// The most loops that begin after a binding and are open at one position.
/// Deeper nesting copies. The self-hosted copy pass has the same bound.
const MAX_OPEN_LOOPS: usize = 32;

/// The first position at or after `from` that is not a view, skipping at most
/// [`MAX_VIEWS`].
fn skip_views(ops: &[Op], from: usize) -> usize {
    let mut q = from;
    // Terminates because `q` increases and is bounded by `from + MAX_VIEWS`.
    while q < from + MAX_VIEWS && ops.get(q).is_some_and(is_view) {
        q += 1;
    }
    q
}

fn is_view(op: &Op) -> bool {
    matches!(
        op,
        Op::GetField(StructField::FlatNested { .. })
            | Op::GetTupleField(TupleField::FlatNested { .. })
            | Op::GetEnumField(EnumField::FlatNested { .. })
    )
}

fn is_dead_end(op: &Op, read: &Read) -> bool {
    match op {
        Op::GetField(StructField::Flat { .. })
        | Op::GetTupleField(TupleField::Flat { .. })
        | Op::GetEnumField(EnumField::Flat { .. })
        | Op::NewComposite(NewCompositeOperand::Flat { .. }) => true,
        Op::SetData(slot) => !(read.lo..read.hi).contains(slot),
        _ => false,
    }
}

fn is_hazard(op: &Op, read: &Read) -> bool {
    match op {
        Op::SetData(slot) => (read.lo..read.hi).contains(slot),
        Op::SetDataIndexed(base, len) => {
            let end = base.saturating_add(*len);
            *base < read.hi && read.lo < end
        }
        Op::Call(_, _)
        | Op::CallVerifiedNative(_, _)
        | Op::CallExternalNative(_, _)
        | Op::Yield => true,
        _ => false,
    }
}

/// The target of a forward jump, which is every jump but the `EndLoop`
/// back-edge.
fn forward_target(op: &Op) -> Option<usize> {
    match op {
        Op::If(t) | Op::Else(t) | Op::Break(t) | Op::BreakIf(t) => Some(usize::from(*t)),
        _ => None,
    }
}

/// The local rule, for a read bound by the `SetLocal(k)` at position `def`.
fn local_needs_copy(ops: &[Op], read: &Read, def: usize, k: u16) -> bool {
    // The exits of the loops that began after `def` and are still open at the
    // current position, outermost first. Nesting is bounded by the program.
    let mut open: Vec<usize> = Vec::new();
    let mut last = def;
    // The nearest position after `def` that a jump from before `def` lands
    // on. Control can arrive there without running the binding, so a use at
    // or past it may read a value bound in an earlier pass.
    let mut skipped = usize::MAX;
    for (i, op) in ops.iter().enumerate() {
        if i < def {
            if matches!(op, Op::GetLocal(s) | Op::SetLocal(s) if *s == k) {
                return true;
            }
            if let Some(t) = forward_target(op)
                && t > def
            {
                skipped = skipped.min(t);
            }
            continue;
        }
        if i == def {
            continue;
        }
        // Loop exits are distinct positions, one past each `EndLoop`, so at
        // most one loop closes here. A second would only stay open longer and
        // extend the interval, which copies more, never less.
        if open.last().is_some_and(|&exit| exit <= i) {
            open.pop();
        }
        match op {
            Op::Loop(_) if open.len() >= MAX_OPEN_LOOPS => return true,
            Op::Loop(exit) => open.push(usize::from(*exit)),
            Op::SetLocal(s) if *s == k => return true,
            Op::GetLocal(s) if *s == k => {
                let q = skip_views(ops, i + 1);
                if !ops.get(q).is_some_and(|op| is_dead_end(op, read)) {
                    return true;
                }
                let end = open.first().map_or(q, |&exit| exit.saturating_sub(1));
                last = last.max(end).max(q);
            }
            _ => {}
        }
    }
    if skipped <= last {
        return true;
    }
    let end = last.min(ops.len().saturating_sub(1));
    ops.get(read.at..=end)
        .is_some_and(|span| span.iter().any(|op| is_hazard(op, read)))
}
