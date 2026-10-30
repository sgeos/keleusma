//! Definite initialization, narrowed. No read may observe a local that no path
//! writes.
//!
//! The virtual machine marks an unwritten local with its `Unit` value. The
//! operator ruled on 2026-10-28 that the verifier should keep a read from
//! observing that marker, and on 2026-10-30, "Adopt C. Then attempt to maintain
//! guarantees and widen to A and B together." Option (c) narrows the rule to
//! what the current compiler already guarantees. This module is that narrowed
//! rule. The widening, in which the compiler writes every path and composite
//! private fields receive initializers, is separate work.
//!
//! # The rule
//!
//! A forward may-analysis over each chunk's control-flow edges computes the
//! locals that some path writes before every instruction. The parameters are
//! written on entry. A `GetLocal` of a slot outside that set reads a local that
//! no path to it writes, and is rejected. `Reset` clears every local, the
//! parameters included, and resumes after `Stream`, and the next resume writes a
//! stream's input into slot 0, so at `Stream` the set becomes slot 0 alone, or
//! empty for a stream without a parameter. A local written only before `Stream`
//! and read after it is therefore rejected, since every pass after the first
//! reads the marker.
//!
//! # What the narrowed rule admits, deliberately
//!
//! A read that some path writes and another path does not is admitted. The
//! compiler's handled indexed-write lowering writes a scratch local and reads
//! it under the same comparison, which only a path-sensitive analysis could
//! prove, and the operator's ruling keeps that program valid. A native backend
//! must therefore keep a run-time kind for such a local.
//!
//! Private data is not checked. A composite or `Text` private slot has no
//! load-time initializer, and the persistence idiom writes it on the first pass
//! only, as in `if seed == 0 { d.p = ... }`. A hot swap also seeds every slot of
//! the incoming module from the host's `initial_data`, so even a slot the module
//! never writes may hold a value when it is read. Nothing short of initializers
//! for composite private fields, the widening's option (b), proves such a read
//! safe.
//!
//! # Soundness and termination
//!
//! An unreached instruction holds no set and is not checked, since it never
//! runs. A join is union, and every set only grows, at most once per local, so
//! the worklist terminates. Control-flow targets are validated before this pass
//! runs, and a target outside the chunk contributes no edge here.

use crate::bytecode::{Chunk, Module, Op};
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
    /// Union in place, returning whether the set changed.
    fn join(&mut self, other: &Bits) -> bool {
        let mut changed = false;
        for (a, b) in self.0.iter_mut().zip(&other.0) {
            let next = *a | *b;
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

/// A forward may-analysis over one chunk's locals. `transfer` checks an
/// instruction's reads against the set before its writes, then applies them.
fn may_analysis(
    chunk: &Chunk,
    width: usize,
    entry: Bits,
    mut transfer: impl FnMut(usize, &Op, &mut Bits) -> Result<(), VerifyError>,
) -> Result<(), VerifyError> {
    let ops = &chunk.ops;
    if ops.is_empty() {
        return Ok(());
    }
    let mut state: Vec<Option<Bits>> = vec![None; ops.len()];
    let mut queued = vec![false; ops.len()];
    let mut work = VecDeque::new();
    state[0] = Some(entry);
    work.push_back(0usize);
    queued[0] = true;
    // The worklist only revisits an instruction whose set grew, and each set
    // can grow at most `width` times, so this is bounded.
    while let Some(ip) = work.pop_front() {
        queued[ip] = false;
        let mut set = state[ip].clone().unwrap_or_else(|| Bits::empty(width));
        transfer(ip, &ops[ip], &mut set)?;
        let (fall, jump) = successors(ops, ip);
        for to in [fall, jump].into_iter().flatten() {
            let changed = match &mut state[to] {
                Some(old) => old.join(&set),
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
    Ok(())
}

fn local_error(chunk: &Chunk, ip: usize, slot: u16) -> VerifyError {
    VerifyError {
        chunk_name: chunk.name.clone(),
        message: format!(
            "GetLocal({slot}) at {ip} reads local {slot}, which no path to it writes; a read \
             may not observe an empty slot (definite-initialization rule)"
        ),
    }
}

/// The rule over one chunk.
fn check_locals(chunk: &Chunk) -> Result<(), VerifyError> {
    let width = chunk.local_count as usize;
    let mut entry = Bits::empty(width);
    for p in 0..usize::from(chunk.param_count).min(width) {
        entry.set(p);
    }
    may_analysis(chunk, width, entry, |ip, op, set| {
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
    })
}

/// Reject a module in which some read observes a local that no path writes.
///
/// # Errors
///
/// A [`VerifyError`] naming the chunk, the instruction and the slot of the
/// first such read found.
pub fn verify_definite_initialization(module: &Module) -> Result<(), VerifyError> {
    for chunk in &module.chunks {
        check_locals(chunk)?;
    }
    Ok(())
}
