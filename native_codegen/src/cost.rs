//! The bytecode cost bound composed through calls (V0.3.x Workstream E).
//!
//! The verifier bounds a chunk's cost in the nominal cost model's units, and
//! its bound is shallow with respect to calls between scripts. An `Op::Call`
//! is charged its dispatch only, not the callee's body. That figure therefore
//! bounds no whole program, which is one of the two reasons Workstream D
//! exported none.
//!
//! [`composed_bounds`] closes that half. The call graph is acyclic, because the
//! type checker rejects recursion, so the chunks can be bounded callees first.
//! Each chunk is then bounded again with a cost model whose `Op::Call` cost is
//! its dispatch plus the callee's own composed bound. The verifier multiplies
//! that per-call cost by each loop's iteration bound like any other
//! instruction's, so a call inside a loop is charged once per iteration.
//!
//! # What the figure is, and what it is not
//!
//! It is a bound on the nominal cost of the bytecode path one step of the
//! entry takes, per iteration for a `Stream` entry and per resume for a
//! `Reentrant` one. `tests/native_cost_path.rs` shows that the native code
//! executes exactly the bytecode instructions the virtual machine executes, so
//! the same figure bounds the nominal cost of the native path.
//!
//! **It is not a bound on native execution time.** The nominal costs are
//! relative weights calibrated against the interpreter, and native code runs
//! none of the interpreter's work. Roadmap open decision 1, whether native WCET
//! is hard or best effort, belongs to the operator. Until it is decided this
//! figure is the best-effort claim the native codegen strategy recommends for
//! V0.4.0, labelled as bytecode cost.
//!
//! # When no figure is established
//!
//! A chunk calling a host native has no bound here, because the native's cost
//! is the host's attestation and the library has no host. A chunk calling a
//! `Stream` or `Reentrant` chunk has none either, because a suspending callee's
//! cost per call spans several host steps. Any verifier refusal propagates.
//!
//! A `Func` or `Reentrant` chunk is bounded with each callee's text-ness known
//! from its declared return shape, through the verifier's
//! `wcet_whole_chunk_with_callees`. The verifier offers no such form for a
//! `Stream` iteration, so a `Stream` chunk is still bounded treating each call
//! result as possibly text.

use keleusma::bytecode::{BlockType, CostModel, Module, NOMINAL_COST_MODEL, Op};
use std::cell::RefCell;

thread_local! {
    /// Composed bounds of the chunks already bounded, indexed by chunk.
    ///
    /// `CostModel::op_cycles` is a plain function pointer and cannot capture
    /// state, so the per-callee figures reach it through this cell. It is set
    /// for the duration of one verifier call and is local to the thread.
    static CALLEE_BOUNDS: RefCell<Vec<Option<u32>>> = const { RefCell::new(Vec::new()) };
}

/// The nominal cost, with a call charged its callee's composed bound as well.
///
/// A callee without a bound costs `u32::MAX`, which saturates the chunk's
/// figure. [`composed_bounds`] refuses such a chunk before using its figure,
/// so the saturated value is never reported.
fn composed_cycles(op: &Op) -> u32 {
    let own = (NOMINAL_COST_MODEL.op_cycles)(op);
    match op {
        Op::Call(target, _) => CALLEE_BOUNDS.with(|bounds| {
            let callee = bounds.borrow().get(*target as usize).copied().flatten();
            own.saturating_add(callee.unwrap_or(u32::MAX))
        }),
        _ => own,
    }
}

const COMPOSED_COST_MODEL: CostModel = CostModel {
    value_slot_bytes: NOMINAL_COST_MODEL.value_slot_bytes,
    op_cycles: composed_cycles,
    text_byte_cycles: NOMINAL_COST_MODEL.text_byte_cycles,
};

/// The composed bound of every chunk, or why it has none.
///
/// Chunks are visited callees first by an explicit post-order, and a cycle,
/// which a verified module cannot contain, is reported rather than followed.
pub fn composed_bounds(program: &Module) -> Vec<Result<u32, String>> {
    let n = program.chunks.len();
    let callees: Vec<Vec<usize>> = program
        .chunks
        .iter()
        .map(|c| {
            let mut v: Vec<usize> = c
                .ops
                .iter()
                .filter_map(|op| match op {
                    Op::Call(t, _) => Some(*t as usize),
                    _ => None,
                })
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        })
        .collect();
    let mut order: Vec<usize> = Vec::with_capacity(n);
    let mut state = vec![0u8; n]; // 0 unvisited, 1 on the path, 2 done
    let mut cycle = None;
    for root in 0..n {
        let mut work = vec![(root, false)];
        while let Some((c, expanded)) = work.pop() {
            if expanded {
                state[c] = 2;
                order.push(c);
                continue;
            }
            if state[c] != 0 {
                continue;
            }
            state[c] = 1;
            work.push((c, true));
            for &t in &callees[c] {
                if t >= n {
                    cycle.get_or_insert(format!("chunk {c} calls chunk {t}, which does not exist"));
                } else if state[t] == 1 {
                    cycle.get_or_insert(format!("the call graph has a cycle through chunk {t}"));
                } else if state[t] == 0 {
                    work.push((t, false));
                }
            }
        }
    }
    if let Some(why) = cycle {
        return vec![Err(why); n];
    }
    // Operator question 5. A chunk-only query treats every call result as
    // possibly unbounded text, so a comparison of two `Word` call results was
    // refused. Given each callee's declared return shape it bears no text term.
    let returns_text = match keleusma::verify::module_chunk_returns_text(program) {
        Ok(r) => r,
        Err(e) => return vec![Err(e.message); n],
    };
    let mut bounds: Vec<Option<Result<u32, String>>> = vec![None; n];
    for c in order {
        let chunk = &program.chunks[c];
        let result = (|| {
            if chunk
                .ops
                .iter()
                .any(|op| matches!(op, Op::CallVerifiedNative(..) | Op::CallExternalNative(..)))
            {
                return Err(String::from(
                    "it calls a host native, whose cost is the host's attestation",
                ));
            }
            for &t in &callees[c] {
                if program.chunks[t].block_type != BlockType::Func {
                    return Err(format!(
                        "it calls chunk {t}, a suspending chunk whose cost spans host steps"
                    ));
                }
                if let Some(Err(why)) = &bounds[t] {
                    return Err(format!("its callee chunk {t} has no bound: {why}"));
                }
            }
            let known: Vec<Option<u32>> = bounds
                .iter()
                .map(|b| b.as_ref().and_then(|r| r.as_ref().ok().copied()))
                .collect();
            CALLEE_BOUNDS.with(|cell| *cell.borrow_mut() = known);
            let figure = match chunk.block_type {
                BlockType::Stream => keleusma::verify::wcet_stream_iteration_with_cost_model(
                    chunk,
                    &COMPOSED_COST_MODEL,
                    &[],
                ),
                _ => keleusma::verify::wcet_whole_chunk_with_callees(
                    chunk,
                    &COMPOSED_COST_MODEL,
                    &[],
                    &returns_text,
                ),
            };
            CALLEE_BOUNDS.with(|cell| cell.borrow_mut().clear());
            let figure = figure.map_err(|e| e.message)?;
            if figure == u32::MAX {
                return Err(String::from("the composed figure saturated"));
            }
            Ok(figure)
        })();
        bounds[c] = Some(result);
    }
    bounds
        .into_iter()
        .map(|b| b.unwrap_or_else(|| Err(String::from("not visited"))))
        .collect()
}

/// The composed bound of `program`'s entry, or why it has none.
pub fn entry_bound(program: &Module) -> Result<u32, String> {
    let entry = program
        .entry_point
        .ok_or_else(|| String::from("the module has no entry point"))?;
    composed_bounds(program)
        .into_iter()
        .nth(entry)
        .unwrap_or_else(|| Err(String::from("the entry is out of range")))
}
