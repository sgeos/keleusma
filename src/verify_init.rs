//! Definite initialization. No read may observe an empty slot.
//!
//! The virtual machine marks an unwritten local, and a private data slot that
//! has no load-time initializer, with its `Unit` value. A read of such a slot
//! yields that marker, which no well-typed value equals. The operator ruled on
//! 2026-10-28 that the verifier rejects any module in which such a read is
//! possible, so that a lowering, native code in particular, may treat every
//! slot it reads as holding a value of its static type.
//!
//! # The two rules
//!
//! 1. **Locals.** A forward must-analysis over each chunk's control-flow edges
//!    computes the locals definitely written at every instruction. The
//!    parameters are written on entry. A `GetLocal` of a slot outside the set is
//!    rejected. `Reset` clears every local, the parameters included, and resumes
//!    after `Stream`, and the next resume writes a stream's input into slot 0,
//!    so at `Stream` the set becomes slot 0 alone, or empty for a stream
//!    without a parameter.
//! 2. **Private data.** A private slot whose load-time initializer is `Unit`, a
//!    composite or `Text` slot, must be definitely written before a `GetData` or
//!    `GetDataIndexed` reads it. Writes persist across `Reset`. A `Call` adds the
//!    callee's must-write summary, computed bottom-up, and each callee is
//!    checked with the intersection of the sets its call sites hold.
//!
//! # Soundness and its limits
//!
//! The analysis is a must-analysis in which an unreached instruction is the
//! lattice top, a join is intersection, and every set only shrinks, so the
//! worklist terminates. Instructions no path reaches are not checked, since
//! they never run. `SetDataIndexed` writes one element of a run-time index and
//! so definitely writes none. A chunk on a call cycle, which the surface
//! language forbids, gets an empty summary and is checked with the
//! initializer set alone, both of which are conservative. A native call writes
//! no private data. Control-flow targets are validated before this pass runs,
//! and a target outside the chunk contributes no edge here.

use crate::bytecode::{Chunk, ConstValue, Module, Op, SlotVisibility};
use crate::verify::VerifyError;
use alloc::collections::VecDeque;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

/// A fixed-width set of slot indices.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Bits(Vec<u64>);

impl Bits {
    fn empty(n: usize) -> Self {
        Bits(vec![0; n.div_ceil(64)])
    }
    fn full(n: usize) -> Self {
        let mut b = Bits(vec![u64::MAX; n.div_ceil(64)]);
        if n % 64 != 0
            && let Some(last) = b.0.last_mut()
        {
            *last = (1u64 << (n % 64)) - 1;
        }
        b
    }
    fn has(&self, i: usize) -> bool {
        self.0.get(i / 64).is_some_and(|w| w & (1 << (i % 64)) != 0)
    }
    fn set(&mut self, i: usize) {
        if let Some(w) = self.0.get_mut(i / 64) {
            *w |= 1 << (i % 64);
        }
    }
    fn union(&mut self, other: &Bits) {
        for (a, b) in self.0.iter_mut().zip(&other.0) {
            *a |= *b;
        }
    }
    /// Intersect in place, returning whether the set changed.
    fn meet(&mut self, other: &Bits) -> bool {
        let mut changed = false;
        for (a, b) in self.0.iter_mut().zip(&other.0) {
            let next = *a & *b;
            changed |= next != *a;
            *a = next;
        }
        changed
    }
}

/// The successors of `ip`, and whether it falls through. `Reset` is handled by
/// the caller, which knows the edge's set.
fn successors(ops: &[Op], ip: usize) -> (Option<usize>, Option<usize>) {
    let next = Some(ip + 1).filter(|&n| n < ops.len());
    let jump = |t: u16| Some(t as usize).filter(|&t| t < ops.len());
    match &ops[ip] {
        Op::If(t) | Op::BreakIf(t) => (next, jump(*t)),
        Op::Else(t) | Op::EndLoop(t) | Op::Break(t) => (None, jump(*t)),
        Op::Return | Op::Trap(_) | Op::Reset => (None, None),
        _ => (next, None),
    }
}

/// A forward must-analysis over one chunk. `transfer` applies an instruction's
/// writes and may report a read. Returns the set holding at each `Return`,
/// intersected, or `None` when no `Return` is reached.
///
/// `Reset` ends a path here. Its edge to the instruction after `Stream` would
/// carry, for private data, a superset of the set `Stream` itself carries,
/// since writes only add, so meeting it changes nothing. For locals the callers
/// clear the set at `Stream` instead, which is what every pass after the first
/// sees, and which is stricter on the first.
fn must_analysis(
    chunk: &Chunk,
    width: usize,
    entry: Bits,
    mut transfer: impl FnMut(usize, &Op, &mut Bits) -> Result<(), VerifyError>,
) -> Result<Option<Bits>, VerifyError> {
    let ops = &chunk.ops;
    if ops.is_empty() {
        return Ok(None);
    }
    let mut state: Vec<Option<Bits>> = vec![None; ops.len()];
    let mut queued = vec![false; ops.len()];
    let mut work = VecDeque::new();
    state[0] = Some(entry);
    work.push_back(0usize);
    queued[0] = true;
    let push = |state: &mut Vec<Option<Bits>>,
                work: &mut VecDeque<usize>,
                queued: &mut Vec<bool>,
                to: usize,
                set: &Bits| {
        let changed = match &mut state[to] {
            Some(old) => old.meet(set),
            slot @ None => {
                *slot = Some(set.clone());
                true
            }
        };
        if changed && !queued[to] {
            queued[to] = true;
            work.push_back(to);
        }
    };
    // The worklist only revisits an instruction whose set shrank, and each set
    // can shrink at most `width` times, so this is bounded.
    while let Some(ip) = work.pop_front() {
        queued[ip] = false;
        let mut set = state[ip].clone().unwrap_or_else(|| Bits::full(width));
        // Reads are checked against the set before the instruction's writes.
        transfer(ip, &ops[ip], &mut set)?;
        let (fall, jump) = successors(ops, ip);
        for to in [fall, jump].into_iter().flatten() {
            push(&mut state, &mut work, &mut queued, to, &set);
        }
    }
    // A `Return` writes nothing, so the set entering it is the set it returns.
    let mut at_return: Option<Bits> = None;
    for (ip, op) in ops.iter().enumerate() {
        if let (Op::Return, Some(set)) = (op, &state[ip]) {
            match &mut at_return {
                Some(acc) => {
                    acc.meet(set);
                }
                None => at_return = Some(set.clone()),
            }
        }
    }
    Ok(at_return)
}

fn local_error(chunk: &Chunk, ip: usize, slot: u16) -> VerifyError {
    VerifyError {
        chunk_name: chunk.name.clone(),
        message: format!(
            "GetLocal({slot}) at {ip} may read local {slot} before any write on some path; \
             a read may not observe an empty slot (definite-initialization rule)"
        ),
    }
}

fn data_error(chunk: &Chunk, ip: usize, slot: usize) -> VerifyError {
    VerifyError {
        chunk_name: chunk.name.clone(),
        message: format!(
            "instruction {ip} may read private data slot {slot}, which has no load-time \
             initializer, before any write on some path; a read may not observe an empty \
             slot (definite-initialization rule)"
        ),
    }
}

/// Rule 1 over one chunk.
fn check_locals(chunk: &Chunk) -> Result<(), VerifyError> {
    let width = chunk.local_count as usize;
    let mut entry = Bits::empty(width);
    for p in 0..usize::from(chunk.param_count).min(width) {
        entry.set(p);
    }
    must_analysis(chunk, width, entry, |ip, op, set| {
        match op {
            Op::GetLocal(i) if (*i as usize) < width && !set.has(*i as usize) => {
                return Err(local_error(chunk, ip, *i));
            }
            Op::SetLocal(i) => set.set(*i as usize),
            // `Reset` clears every local, parameters included, and resumes
            // after `Stream`. Each resume of a stream with a parameter
            // writes its input into slot 0, and only that slot, before the
            // next instruction runs, as `Vm::resume` does.
            Op::Stream => {
                *set = Bits::empty(width);
                if chunk.param_count > 0 && width > 0 {
                    set.set(0);
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(())
}

/// The data slots that hold `Unit` until written, and the set written at load.
fn unit_private_slots(module: &Module) -> (usize, Bits) {
    let Some(layout) = module.data_layout.as_ref() else {
        return (0, Bits::empty(0));
    };
    let width = layout.slots.len();
    let mut initialized = Bits::full(width);
    let mut private = 0usize;
    for (slot, s) in layout.slots.iter().enumerate() {
        if s.visibility == SlotVisibility::Private {
            // A module predating the initializer table holds `Unit` in every
            // private slot, as the VM loads it.
            let init = layout.private_init.get(private);
            if matches!(init, None | Some(ConstValue::Unit)) {
                initialized.0[slot / 64] &= !(1 << (slot % 64));
            }
            private += 1;
        }
    }
    (width, initialized)
}

/// The chunks in an order where every callee precedes its callers, with the
/// chunks on a call cycle omitted.
fn callees_first(module: &Module) -> (Vec<usize>, Vec<bool>) {
    let n = module.chunks.len();
    let mut callers: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut pending = vec![0usize; n];
    for (c, chunk) in module.chunks.iter().enumerate() {
        let mut seen = Vec::new();
        for op in &chunk.ops {
            if let Op::Call(callee, _) = op {
                let callee = *callee as usize;
                if callee < n && !seen.contains(&callee) {
                    seen.push(callee);
                    callers[callee].push(c);
                    pending[c] += 1;
                }
            }
        }
    }
    let mut order = Vec::with_capacity(n);
    let mut ready: VecDeque<usize> = (0..n).filter(|&c| pending[c] == 0).collect();
    while let Some(c) = ready.pop_front() {
        order.push(c);
        for &caller in &callers[c] {
            pending[caller] -= 1;
            if pending[caller] == 0 {
                ready.push_back(caller);
            }
        }
    }
    let mut ordered = vec![false; n];
    for &c in &order {
        ordered[c] = true;
    }
    (order, ordered)
}

/// Rule 2 over the module.
fn check_private_data(module: &Module) -> Result<(), VerifyError> {
    let (width, initialized) = unit_private_slots(module);
    if width == 0 || initialized == Bits::full(width) {
        return Ok(());
    }
    let n = module.chunks.len();
    let (order, ordered) = callees_first(module);
    let writes = |op: &Op, set: &mut Bits, summaries: &[Bits]| match op {
        Op::SetData(s) => set.set(*s as usize),
        Op::Call(c, _) => {
            if let Some(summary) = summaries.get(*c as usize) {
                set.union(summary);
            }
        }
        _ => {}
    };
    // Bottom-up must-write summaries, each relative to an empty entry. A chunk
    // that never returns writes nothing for its caller.
    let mut summaries = vec![Bits::empty(width); n];
    for &c in &order {
        let chunk = &module.chunks[c];
        let at_return = must_analysis(chunk, width, Bits::empty(width), |_, op, set| {
            writes(op, set, &summaries);
            Ok(())
        })?;
        summaries[c] = at_return.unwrap_or_else(|| Bits::empty(width));
    }
    // Top-down checks. A chunk no ordered chunk calls, an entry or an
    // unreferenced chunk, starts from the initializer set. A callee starts
    // from the intersection of the sets at its call sites.
    let mut entries: Vec<Option<Bits>> = vec![None; n];
    for &c in order.iter().rev() {
        let chunk = &module.chunks[c];
        let mut entry = entries[c].take().unwrap_or_else(|| initialized.clone());
        entry.union(&initialized);
        let mut sites: Vec<(usize, Bits)> = Vec::new();
        must_analysis(chunk, width, entry, |ip, op, set| {
            match op {
                Op::GetData(s) => {
                    let s = *s as usize;
                    if s < width && !set.has(s) {
                        return Err(data_error(chunk, ip, s));
                    }
                }
                Op::GetDataIndexed(base, len) => {
                    for s in (*base as usize)..(*base as usize).saturating_add(*len as usize) {
                        if s < width && !set.has(s) {
                            return Err(data_error(chunk, ip, s));
                        }
                    }
                }
                Op::Call(callee, _) => sites.push((*callee as usize, set.clone())),
                _ => {}
            }
            writes(op, set, &summaries);
            Ok(())
        })?;
        for (callee, set) in sites {
            if callee < n && ordered[callee] {
                match &mut entries[callee] {
                    Some(acc) => {
                        acc.meet(&set);
                    }
                    slot @ None => *slot = Some(set),
                }
            }
        }
    }
    // A chunk on a call cycle is checked from the initializer set alone.
    for c in (0..n).filter(|&c| !ordered[c]) {
        let chunk = &module.chunks[c];
        must_analysis(chunk, width, initialized.clone(), |ip, op, set| {
            match op {
                Op::GetData(s) if (*s as usize) < width && !set.has(*s as usize) => {
                    return Err(data_error(chunk, ip, *s as usize));
                }
                Op::GetDataIndexed(base, len) => {
                    for s in (*base as usize)..(*base as usize).saturating_add(*len as usize) {
                        if s < width && !set.has(s) {
                            return Err(data_error(chunk, ip, s));
                        }
                    }
                }
                Op::SetData(s) => set.set(*s as usize),
                _ => {}
            }
            Ok(())
        })?;
    }
    Ok(())
}

/// Reject a module in which some read may observe an empty slot.
///
/// # Errors
///
/// A [`VerifyError`] naming the chunk, the instruction and the slot of the
/// first such read found.
pub fn verify_definite_initialization(module: &Module) -> Result<(), VerifyError> {
    for chunk in &module.chunks {
        check_locals(chunk)?;
    }
    check_private_data(module)
}
