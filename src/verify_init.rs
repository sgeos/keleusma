//! Definite initialization. No read may observe the virtual machine's
//! empty-slot `Unit`.
//!
//! The virtual machine marks an unwritten local, and a private data slot with
//! no load-time initializer, with its `Unit` value. The operator ruled on
//! 2026-10-28 that the verifier should keep a read from observing that marker,
//! and on 2026-10-30, "Adopt C. Then attempt to maintain guarantees and widen to
//! A and B together." Option (c) narrowed the rule to a local that no path
//! writes. Option (a) makes the compiler write every local on every path before
//! any read, option (b) gives every private slot its type's zero at load, and
//! this module then proves the stronger property.
//!
//! # Rule 1, locals
//!
//! A forward must-analysis over each chunk's control-flow edges computes the
//! locals that every path writes before each instruction. The parameters are
//! written on entry. A `GetLocal` of a slot outside that set may read a local
//! that some path leaves unwritten, and is rejected. `Reset` clears every local,
//! the parameters included, and resumes after `Stream`, and the next resume
//! writes a stream's input into slot 0, so at `Stream` the set becomes slot 0
//! alone, or empty for a stream without a parameter. A local written only before
//! `Stream` and read after it is therefore rejected, since every pass after the
//! first reads the marker.
//!
//! The narrowed guarantee is kept, since a local that no path writes is outside
//! every path's set. The rule is strictly stronger, so that a native backend may
//! treat every local it reads as holding a value of its static type.
//!
//! # Rule 2, private data
//!
//! A private slot whose load-time initializer is `Unit`, or that has none, must
//! be written on every path before a `GetData` or `GetDataIndexed` reads it. The
//! compiler now gives every private slot a non-`Unit` initializer except a slot
//! of type `()`, whose only value is `Unit`, a slot whose zero would violate a
//! refinement, and a slot of a type with no zero, so for every other module
//! this rule tracks nothing and costs nothing. It is not "every initializer is
//! non-`Unit`", because a module carries no slot types and a `()` slot's `Unit`
//! is its value.
//!
//! # What the compiler does to meet them
//!
//! The handled indexed-write lowering, `compile_data_index_store`, reads its
//! `old` and `new` slots in the arm dispatch under the same flag test that
//! guards the success branch that writes them. A path-insensitive analysis
//! cannot pair the two, so the lowering writes each such bound slot with the
//! element type's zero before the flag test. This is the only lowering the
//! light tier found to read a local that some path leaves unwritten. The
//! private initializers are `fill_private_zero_initializers` in the compiler.
//!
//! # Soundness and termination
//!
//! An unreached instruction is the lattice top and is not checked, since it
//! never runs. A join is intersection, and every set only shrinks, at most once
//! per slot, so each worklist terminates. Reads are checked against the final
//! sets after the fixpoint. Control-flow targets are validated before this pass
//! runs, and a target outside the chunk contributes no edge here. Rule 2
//! assumes nothing about a chunk's caller, since the host may enter any chunk,
//! and so a read that only a caller's write protects is refused.

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
    fn has(&self, i: usize) -> bool {
        self.0.get(i / 64).is_some_and(|w| w & (1 << (i % 64)) != 0)
    }
    fn set(&mut self, i: usize) {
        if let Some(w) = self.0.get_mut(i / 64) {
            *w |= 1 << (i % 64);
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

/// A forward must-analysis over one chunk. `transfer` applies an
/// instruction's writes to the set before it. Returns the set before each
/// instruction, or `None` for an instruction no path reaches.
///
/// The callers check reads only against these final sets, after the fixpoint,
/// so the verdict and the instruction reported do not depend on the order in
/// which the worklist visits the paths.
fn must_analysis(
    chunk: &Chunk,
    entry: Bits,
    transfer: impl Fn(&Op, &mut Bits),
) -> Vec<Option<Bits>> {
    let ops = &chunk.ops;
    let mut state: Vec<Option<Bits>> = vec![None; ops.len()];
    if ops.is_empty() {
        return state;
    }
    let mut queued = vec![false; ops.len()];
    let mut work = VecDeque::new();
    state[0] = Some(entry);
    work.push_back(0usize);
    queued[0] = true;
    // The worklist only revisits an instruction whose set shrank, and each set
    // can shrink at most once per slot after its first arrival, so this is
    // bounded.
    while let Some(ip) = work.pop_front() {
        queued[ip] = false;
        // Every queued instruction holds a set, since it was queued on arrival.
        let Some(mut set) = state[ip].clone() else {
            continue;
        };
        transfer(&ops[ip], &mut set);
        let (fall, jump) = successors(ops, ip);
        for to in [fall, jump].into_iter().flatten() {
            let changed = match &mut state[to] {
                Some(old) => old.meet(&set),
                slot @ None => {
                    *slot = Some(set.clone());
                    true
                }
            };
            if changed && !queued[to] {
                queued[to] = true;
                work.push_back(to);
            }
        }
    }
    state
}

fn local_error(chunk: &Chunk, ip: usize, slot: u16) -> VerifyError {
    VerifyError {
        chunk_name: chunk.name.clone(),
        message: format!(
            "GetLocal({slot}) at {ip} may read local {slot} before any write on some path; a \
             read may not observe an empty slot (definite-initialization rule)"
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
    let state = must_analysis(chunk, entry, |op, set| match op {
        Op::SetLocal(i) => set.set(*i as usize),
        // `Reset` clears every local, parameters included, and resumes after
        // `Stream`. Each resume of a stream with a parameter writes its input
        // into slot 0, and only that slot, before the next instruction runs,
        // as `Vm::resume` does.
        Op::Stream => {
            *set = Bits::empty(width);
            if chunk.param_count > 0 && width > 0 {
                set.set(0);
            }
        }
        _ => {}
    });
    for (ip, set) in state.iter().enumerate() {
        if let (Some(set), Op::GetLocal(i)) = (set, &chunk.ops[ip])
            && (*i as usize) < width
            && !set.has(*i as usize)
        {
            return Err(local_error(chunk, ip, *i));
        }
    }
    Ok(())
}

/// The private data slots whose load-time initializer is `Unit`, ascending.
///
/// After option (b) the compiler gives every private slot its type's zero, so
/// this is empty except for a slot of type `()`, a slot whose zero would violate
/// a refinement, a slot of a type with no zero, and a module from an older
/// compiler. A private slot with no initializer entry also holds `Unit`.
fn unit_private_slots(module: &Module) -> Vec<u32> {
    let Some(layout) = module.data_layout.as_ref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut private = 0usize;
    for (slot, s) in layout.slots.iter().enumerate() {
        if s.visibility == SlotVisibility::Private {
            if matches!(
                layout.private_init.get(private),
                None | Some(ConstValue::Unit)
            ) {
                out.push(slot as u32);
            }
            private += 1;
        }
    }
    out
}

/// The compact indices, into `unit`, of the slots in `[base, base + len)`.
fn unit_range(unit: &[u32], base: u32, len: u32) -> core::ops::Range<usize> {
    let end = base.saturating_add(len);
    unit.partition_point(|&s| s < base)..unit.partition_point(|&s| s < end)
}

fn data_error(chunk: &Chunk, ip: usize, slot: u32) -> VerifyError {
    VerifyError {
        chunk_name: chunk.name.clone(),
        message: format!(
            "instruction {ip} may read private data slot {slot}, whose load-time initializer \
             is the empty-slot Unit, before any write on some path; a read may not observe an \
             empty slot (definite-initialization rule)"
        ),
    }
}

/// The chunks in an order where every callee precedes its callers, with the
/// chunks on a call cycle omitted.
fn callees_first(module: &Module) -> Vec<usize> {
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
    // Each chunk enters `ready` once, when its last callee is ordered.
    while let Some(c) = ready.pop_front() {
        order.push(c);
        for &caller in &callers[c] {
            pending[caller] -= 1;
            if pending[caller] == 0 {
                ready.push_back(caller);
            }
        }
    }
    order
}

/// Apply one instruction's private-data writes to `set`.
fn data_transfer(unit: &[u32], summaries: &[Bits], op: &Op, set: &mut Bits) {
    match op {
        Op::SetData(s) => {
            if let Ok(i) = unit.binary_search(s) {
                set.set(i);
            }
        }
        Op::Call(c, _) => {
            if let Some(summary) = summaries.get(*c as usize) {
                for (a, b) in set.0.iter_mut().zip(&summary.0) {
                    *a |= *b;
                }
            }
        }
        _ => {}
    }
}

/// Rule 2 over the module.
///
/// Only the private slots whose initializer is `Unit` are tracked, by compact
/// index, so a module whose every private slot has an initializer, which is
/// every module the compiler now emits short of the residual cases above,
/// costs nothing here. Each chunk starts from the empty set, because the host
/// may enter any chunk through `Vm::call_function` and a hot swap may seed
/// `Unit`. A `Call` adds the callee's must-write summary, the meet of its sets
/// at every reached `Return`, computed bottom-up. A chunk on a call cycle,
/// which the surface language forbids, has an empty summary. Writes persist
/// across `Reset`, which ends a path here. `SetDataIndexed` writes one element
/// at a run-time index and so definitely writes none.
fn check_private_data(module: &Module) -> Result<(), VerifyError> {
    let unit = unit_private_slots(module);
    if unit.is_empty() {
        return Ok(());
    }
    let width = unit.len();
    let mut summaries = vec![Bits::empty(width); module.chunks.len()];
    for c in callees_first(module) {
        let chunk = &module.chunks[c];
        let state = must_analysis(chunk, Bits::empty(width), |op, set| {
            data_transfer(&unit, &summaries, op, set)
        });
        let mut at_return: Option<Bits> = None;
        for (ip, set) in state.iter().enumerate() {
            if let (Op::Return, Some(set)) = (&chunk.ops[ip], set) {
                match &mut at_return {
                    Some(acc) => {
                        acc.meet(set);
                    }
                    None => at_return = Some(set.clone()),
                }
            }
        }
        summaries[c] = at_return.unwrap_or_else(|| Bits::empty(width));
    }
    for chunk in &module.chunks {
        let state = must_analysis(chunk, Bits::empty(width), |op, set| {
            data_transfer(&unit, &summaries, op, set)
        });
        for (ip, set) in state.iter().enumerate() {
            let Some(set) = set else { continue };
            let read = match &chunk.ops[ip] {
                Op::GetData(s) => unit_range(&unit, *s, 1),
                Op::GetDataIndexed(base, len) => unit_range(&unit, *base, *len),
                _ => continue,
            };
            if let Some(i) = read.into_iter().find(|&i| !set.has(i)) {
                return Err(data_error(chunk, ip, unit[i]));
            }
        }
    }
    Ok(())
}

/// Reject a module in which some read may observe an empty slot, a local that
/// some path to it leaves unwritten, or a private data slot whose initializer
/// is `Unit` and that some path to the read leaves unwritten.
///
/// # Errors
///
/// A [`VerifyError`] naming the chunk, the instruction and the slot of the
/// first such read found, locals first.
pub fn verify_definite_initialization(module: &Module) -> Result<(), VerifyError> {
    for chunk in &module.chunks {
        check_locals(chunk)?;
    }
    check_private_data(module)
}
