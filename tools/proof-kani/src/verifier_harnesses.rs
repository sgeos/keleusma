//! Harnesses over the verifier core in `keleusma::verify`.
//!
//! Overflow checks are on in every harness, because Kani checks arithmetic
//! overflow, division by zero, out-of-bounds indexing and every explicit panic
//! by default. A harness therefore fails on any of those, and also on any
//! assertion it states.

use crate::generators::{
    OP_CONSTRUCTORS, any_op, chunk_of, module_of, op_constructor_index, with_close_op,
    with_plain_op, with_target, with_walk_op,
};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use keleusma::bytecode::{BlockType, CostModel, NOMINAL_COST_MODEL, Op, VALUE_SLOT_SIZE_BYTES};
use keleusma::verify;

/// Largest per-instruction cycle cost the bounded cost models return.
///
/// Below half of `u32::MAX`, so the sum of the `Stream` and `Reset` costs
/// cannot overflow. Finding F1 in `README.md` explains why that matters.
const HALF_CYCLES: u32 = u32::MAX / 2;

/// A cost model whose cost for every call is an independent symbolic value.
///
/// Two calls on the same instruction may return different values, so this
/// over-approximates every deterministic cost model. It suits a harness that
/// checks only for the absence of panics.
fn any_cycles(_: &Op) -> u32 {
    kani::any()
}

/// As [`any_cycles`], with every value at most [`HALF_CYCLES`].
fn half_cycles(_: &Op) -> u32 {
    let c: u32 = kani::any();
    kani::assume(c <= HALF_CYCLES);
    c
}

/// A cost model of one cycle for every instruction.
fn unit_cycles(_: &Op) -> u32 {
    1
}

fn model(op_cycles: fn(&Op) -> u32, text_byte_cycles: u32) -> CostModel {
    CostModel {
        value_slot_bytes: VALUE_SLOT_SIZE_BYTES,
        op_cycles,
        text_byte_cycles,
    }
}

// Two deterministic cost models, a lower and a higher one, for the monotonicity
// harness. Each charges by an instruction class, from a table of four symbolic
// entries fixed before the analysis runs. Statics are the only way to give a
// plain function pointer symbolic but fixed state.
static LOW: [AtomicU32; 4] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static HIGH: [AtomicU32; 4] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];

fn op_class(op: &Op) -> usize {
    match op {
        Op::Stream | Op::Reset | Op::Yield => 0,
        Op::If(_) | Op::Else(_) | Op::EndIf | Op::Loop(_) | Op::EndLoop(_) => 1,
        Op::Break(_) | Op::BreakIf(_) | Op::Return | Op::Trap(_) => 2,
        _ => 3,
    }
}

fn low_cycles(op: &Op) -> u32 {
    LOW[op_class(op)].load(Ordering::Relaxed)
}

fn high_cycles(op: &Op) -> u32 {
    HIGH[op_class(op)].load(Ordering::Relaxed)
}

/// `op_depth_effect`, `stack_growth`, `stack_shrink` and the nominal cost
/// table are total over every instruction, and the two operand-stack models
/// agree on the net effect.
///
/// Property. For every constructor of `Op` with every field symbolic, none of
/// the four functions panics or overflows, the required operand count is not
/// negative, the net effect never removes more operands than are required, and
/// the net effect equals `stack_growth - stack_shrink`. The nominal cost is at
/// least one cycle.
#[kani::proof]
fn op_tables_are_total_and_agree() {
    let op = any_op();
    assert!(op_constructor_index(&op) < OP_CONSTRUCTORS);
    let empty = chunk_of(Vec::new(), BlockType::Func, 0);
    let (required, net) = verify::op_depth_effect(&op, &empty);
    assert!(required >= 0);
    assert!(net >= -required);
    let growth = i64::from(op.stack_growth());
    let shrink = i64::from(op.stack_shrink());
    assert_eq!(i64::from(net), growth - shrink);
    assert!(NOMINAL_COST_MODEL.cycles(&op) >= 1);
}

/// Chunk length used by the walk harnesses, and so the largest jump target
/// that [`with_walk_op`] produces for them.
const WALK_LEN: u16 = 4;

/// Calls `f` with each walk-harness chunk of the single-instruction template,
/// `Stream, a, Yield, Reset`, with `a` any instruction of [`with_walk_op`].
///
/// This template reaches every instruction of the set once, including `If`,
/// `Else` and `Loop` with every target from `0` to the chunk length, so it
/// includes every backward, self and out-of-range target at position one.
fn with_single_op_body<F: FnMut(Vec<Op>)>(f: &mut F) {
    with_walk_op(WALK_LEN, &mut |a| f(vec![Op::Stream, a, Op::Yield, Op::Reset]));
}

/// Calls `f` with each walk-harness chunk of the nested template,
/// `Stream, open, b, Reset`, where `open` is `If(t)` when `is_loop` is false
/// and `Loop(t)` when it is true, `t` is a forward target from `2` to the
/// chunk length, and `b` is any instruction of [`with_close_op`] with an
/// `Else` target from `3` to the chunk length.
///
/// This is the template that puts a structured region around a second
/// instruction, so that the walks recurse. Backward and self targets of the
/// opener are reached by [`with_single_op_body`], and a backward or self
/// `Else` target at position two is refused by the same forward-target check
/// that refuses them at position one.
fn with_nested_body<F: FnMut(Vec<Op>)>(is_loop: bool, f: &mut F) {
    with_target(WALK_LEN - 2, &mut |k| {
        let t = k + 2;
        let open = if is_loop { Op::Loop(t) } else { Op::If(t) };
        with_close_op(3, WALK_LEN, &mut |b| f(vec![Op::Stream, open, b, Op::Reset]));
    });
}

/// Checks one Stream chunk with the Stream iteration WCET walk.
///
/// Property. Under any cost model whose per-instruction costs stay at or below
/// half of `u32::MAX` and with any text-byte cost,
/// `wcet_stream_iteration_with_cost_model` returns without panicking or
/// overflowing.
fn check_wcet_stream(ops: Vec<Op>) -> bool {
    let chunk = chunk_of(ops, BlockType::Stream, 1);
    let cm = model(half_cycles, kani::any());
    verify::wcet_stream_iteration_with_cost_model(&chunk, &cm, &[]).is_ok()
}

/// Checks one Stream chunk with the Stream iteration WCMU walk.
///
/// Property. For any `local_count`, `wcmu_stream_iteration` returns without
/// panicking or overflowing, and on success the stack bound is a multiple of
/// the slot width and at least `local_count` slots.
fn check_wcmu_stream(ops: Vec<Op>) -> bool {
    let local_count: u16 = kani::any();
    let chunk = chunk_of(ops, BlockType::Stream, local_count);
    match verify::wcmu_stream_iteration(&chunk) {
        Ok((stack, _heap)) => {
            assert_eq!(stack % VALUE_SLOT_SIZE_BYTES, 0);
            assert!(stack >= u32::from(local_count) * VALUE_SLOT_SIZE_BYTES);
            true
        }
        Err(_) => false,
    }
}

/// [`check_wcet_stream`] over every chunk of [`with_single_op_body`].
#[kani::proof]
#[kani::unwind(6)]
fn wcet_stream_single_op() {
    with_single_op_body(&mut |ops| {
        let ok = check_wcet_stream(ops);
        kani::cover!(ok, "an accepted chunk is reached");
        kani::cover!(!ok, "a rejected chunk is reached");
    });
}

/// [`check_wcet_stream`] over every chunk of [`with_nested_body`] with `If`.
#[kani::proof]
#[kani::unwind(6)]
fn wcet_stream_if_region() {
    with_nested_body(false, &mut |ops| {
        let ok = check_wcet_stream(ops);
        kani::cover!(ok, "an accepted chunk is reached");
    });
}

/// [`check_wcet_stream`] over every chunk of [`with_nested_body`] with `Loop`.
#[kani::proof]
#[kani::unwind(6)]
fn wcet_stream_loop_region() {
    with_nested_body(true, &mut |ops| {
        let ok = check_wcet_stream(ops);
        kani::cover!(ok, "an accepted chunk is reached");
    });
}

/// [`check_wcmu_stream`] over every chunk of [`with_single_op_body`].
#[kani::proof]
#[kani::unwind(6)]
fn wcmu_stream_single_op() {
    with_single_op_body(&mut |ops| {
        let ok = check_wcmu_stream(ops);
        kani::cover!(ok, "an accepted chunk is reached");
        kani::cover!(!ok, "a rejected chunk is reached");
    });
}

/// [`check_wcmu_stream`] over every chunk of [`with_nested_body`] with `If`.
#[kani::proof]
#[kani::unwind(6)]
fn wcmu_stream_if_region() {
    with_nested_body(false, &mut |ops| {
        let ok = check_wcmu_stream(ops);
        kani::cover!(ok, "an accepted chunk is reached");
    });
}

/// [`check_wcmu_stream`] over every chunk of [`with_nested_body`] with `Loop`.
#[kani::proof]
#[kani::unwind(6)]
fn wcmu_stream_loop_region() {
    with_nested_body(true, &mut |ops| {
        let ok = check_wcmu_stream(ops);
        kani::cover!(ok, "an accepted chunk is reached");
    });
}

/// The whole-chunk WCET walk does not panic for `Reentrant` chunks, under the
/// same cost-model bound.
///
/// Property. For every `Reentrant` chunk `a, Yield, Return`, with `a` any
/// instruction of [`with_walk_op`] and jump targets up to three,
/// `wcet_whole_chunk_with_cost_model` returns without panicking or
/// overflowing. `Reentrant` is chosen because its entry has the most logic,
/// the segmented per-resume bound. `Func` chunks reach the same region walk
/// without that logic and are not enumerated separately, which keeps the run
/// short. A `Stream` chunk is refused by this entry and is covered by the
/// Stream walk harnesses.
#[kani::proof]
#[kani::unwind(6)]
fn wcet_whole_chunk_walk_does_not_panic() {
    with_walk_op(3, &mut |a| {
        let chunk = chunk_of(vec![a, Op::Yield, Op::Return], BlockType::Reentrant, 1);
        let cm = model(half_cycles, kani::any());
        let r = verify::wcet_whole_chunk_with_cost_model(&chunk, &cm, &[]);
        kani::cover!(r.is_ok(), "an accepted chunk is reached");
    });
}

/// On straight-line Stream bodies the WCET bound is exact.
///
/// Property. For every chunk `Stream, b1, b2, Reset` with `b1` an instruction
/// of [`with_plain_op`] and `b2` either `Dup` or `Yield`, under the unit cost
/// model, the call succeeds and returns four. Every executed instruction is
/// charged once and nothing else is charged.
#[kani::proof]
#[kani::unwind(6)]
fn wcet_straight_line_is_exact() {
    with_plain_op(&mut |b1| {
        // Each branch calls the check with a concrete chunk, for the reason
        // given at the top of `generators.rs`.
        let mut check = |b2: Op| {
            let chunk = chunk_of(vec![Op::Stream, b1, b2, Op::Reset], BlockType::Stream, 0);
            let w =
                verify::wcet_stream_iteration_with_cost_model(&chunk, &model(unit_cycles, 1), &[]);
            assert_eq!(w.ok(), Some(4));
        };
        if kani::any() {
            check(Op::Dup)
        } else {
            check(Op::Yield)
        }
    });
}

/// The Stream iteration WCET bound is monotone in the cost model.
///
/// Property. For two deterministic cost models that charge by instruction
/// class, with every low entry at most the corresponding high entry and every
/// high entry at most `2^20`, and every chunk `Stream, a, Yield, Reset` with
/// `a` any instruction of [`with_walk_op`], acceptance does not depend on the
/// model, and the bound under the low model is at most the bound under the
/// high model.
#[kani::proof]
#[kani::unwind(6)]
fn wcet_is_monotone_in_the_cost_model() {
    let mut i = 0;
    while i < 4 {
        let lo: u32 = kani::any();
        let hi: u32 = kani::any();
        kani::assume(lo <= hi && hi <= 1 << 20);
        LOW[i].store(lo, Ordering::Relaxed);
        HIGH[i].store(hi, Ordering::Relaxed);
        i += 1;
    }
    let text: u32 = kani::any();
    kani::assume(text <= 1 << 20);
    with_walk_op(WALK_LEN, &mut |a| {
        let chunk = chunk_of(vec![Op::Stream, a, Op::Yield, Op::Reset], BlockType::Stream, 1);
        let w_low =
            verify::wcet_stream_iteration_with_cost_model(&chunk, &model(low_cycles, text), &[]);
        let w_high =
            verify::wcet_stream_iteration_with_cost_model(&chunk, &model(high_cycles, text), &[]);
        assert_eq!(w_low.is_ok(), w_high.is_ok());
        if let (Ok(lo), Ok(hi)) = (w_low, w_high) {
            assert!(lo <= hi);
            kani::cover!(lo < hi, "the two models give different bounds");
        }
    });
}

/// Calls `f` with one instruction for the static call-depth harness, a
/// `Return` or a `Call` to callee `0`, `1`, `2` or the out-of-range `3`.
fn with_call_or_return<F: FnMut(Op)>(f: &mut F) {
    if kani::any() {
        f(Op::Return)
    } else {
        with_target(3, &mut |c| f(Op::Call(c, kani::any())))
    }
}

/// The static call-depth computation does not panic and returns a depth no
/// greater than the number of chunks.
///
/// Property. For a module of three chunks, the first two each holding one
/// instruction of [`with_call_or_return`] and the third holding `Return`,
/// `module_call_depth` returns without panicking, rejects exactly the modules
/// whose call graph has a cycle, and otherwise returns a depth between one
/// and three. A module with no `Call` has depth one.
#[kani::proof]
#[kani::unwind(5)]
fn module_call_depth_is_bounded() {
    with_call_or_return(&mut |a| {
        with_call_or_return(&mut |b| {
            let ops = [a, b, Op::Return];
            // The callee of each chunk, when it calls an in-range chunk.
            let callee = |op: &Op| match op {
                Op::Call(t, _) if *t < 3 => Some(*t as usize),
                _ => None,
            };
            // With one instruction per chunk the graph has out-degree at most
            // one, so it has a cycle exactly when following the edges from
            // some chunk for three steps stays on an edge.
            let mut cyclic = false;
            let mut start = 0;
            while start < 3 {
                let mut at = Some(start);
                let mut steps = 0;
                while steps < 3 {
                    at = at.and_then(|i| callee(&ops[i]));
                    steps += 1;
                }
                cyclic |= at.is_some();
                start += 1;
            }
            let has_call = ops.iter().any(|op| matches!(op, Op::Call(_, _)));
            let module = module_of(vec![
                chunk_of(vec![a], BlockType::Func, 0),
                chunk_of(vec![b], BlockType::Func, 0),
                chunk_of(vec![Op::Return], BlockType::Func, 0),
            ]);
            let depth = verify::module_call_depth(&module);
            assert_eq!(depth.is_err(), cyclic);
            if let Ok(d) = depth {
                assert!((1..=3).contains(&d));
                kani::cover!(d == 3, "the deepest chain is reached");
            }
            if !has_call {
                assert_eq!(depth.ok(), Some(1));
            }
        })
    });
}

/// The module WCMU pass does not panic at the default slot width.
///
/// Property. For a module of two `Func` chunks, the first `a, Return` with `a`
/// any instruction of [`with_walk_op`] and jump targets up to two, the second
/// `Return`, and a native table of one symbolic entry, `module_wcmu` returns
/// without panicking or overflowing, and on success reports one entry per
/// chunk. A `Call` at `a` with callee index one reaches the transitive
/// callee contribution.
#[kani::proof]
#[kani::unwind(6)]
fn module_wcmu_does_not_panic() {
    with_walk_op(2, &mut |a| {
        let module = module_of(vec![
            chunk_of(vec![a, Op::Return], BlockType::Func, 1),
            chunk_of(vec![Op::Return], BlockType::Func, 1),
        ]);
        let natives = [kani::any::<u32>()];
        let r = verify::module_wcmu(&module, &natives);
        kani::cover!(r.is_ok(), "an accepted module is reached");
        if let Ok(v) = r {
            assert_eq!(v.len(), 2);
        }
    });
}

/// The full structural verifier does not panic on small modules.
///
/// Property. For a module of the one `Stream` chunk `Stream, a, Yield, Reset`
/// with `a` any instruction of [`with_walk_op`] and jump targets up to four,
/// `verify` returns without panicking or overflowing. The bound is small
/// because `verify` runs every pass, including definite initialization and
/// the typed operand-stack pass.
#[kani::proof]
#[kani::unwind(6)]
fn verify_does_not_panic() {
    with_walk_op(WALK_LEN, &mut |a| {
        let ops = vec![Op::Stream, a, Op::Yield, Op::Reset];
        let module = module_of(vec![chunk_of(ops, BlockType::Stream, 1)]);
        let r = verify::verify(&module);
        kani::cover!(r.is_ok(), "an accepted module is reached");
    });
}

// ---------------------------------------------------------------------------
// Finding harnesses. Each reproduces a defect recorded under "Findings for the
// V0.2.X line" in README.md and is expected to FAIL. `check.sh` requires the
// failure and excludes these harnesses from the pass count.
// ---------------------------------------------------------------------------

/// Finding F1. The Stream iteration WCET sums the `Stream` and `Reset` costs
/// with an unchecked addition.
///
/// Expected to fail with an arithmetic overflow at the `overhead` sum in
/// `wcet_stream_iteration_with_cost_model`, for a cost model that charges more
/// than half of `u32::MAX` for those two instructions. The chunk is the
/// smallest well-formed Stream chunk, so the reproduction is minimal.
#[kani::proof]
#[kani::unwind(5)]
fn finding_f1_wcet_overhead_overflows() {
    let chunk = chunk_of(vec![Op::Stream, Op::Yield, Op::Reset], BlockType::Stream, 1);
    let cm = model(any_cycles, 1);
    let _ = verify::wcet_stream_iteration_with_cost_model(&chunk, &cm, &[]);
}

/// Finding F2. The Stream iteration WCMU divides by and multiplies by a
/// host-supplied slot width that is never validated.
///
/// Expected to fail twice over. A slot width of zero divides by zero at the
/// `Op::Call` arm of the memory walk, and a large slot width overflows the
/// unchecked product `stack_slots * value_slot_bytes`.
#[kani::proof]
#[kani::unwind(5)]
fn finding_f2_wcmu_slot_width_unvalidated() {
    let local_count: u16 = kani::any();
    let chunk = chunk_of(
        vec![Op::Stream, Op::Call(0, 0), Op::Yield, Op::Reset],
        BlockType::Stream,
        local_count,
    );
    let _ = verify::wcmu_stream_iteration_with_value_slot_bytes(&chunk, kani::any());
}

// ---------------------------------------------------------------------------
// Negative control. Expected to FAIL. See README.md.
// ---------------------------------------------------------------------------

/// Negative control. States a false property about the verifier.
///
/// A straight-line chunk `Stream, b, Reset`, with `b` any instruction of
/// [`with_plain_op`], costs three cycles under the unit cost model, as
/// [`wcet_straight_line_is_exact`] establishes. This harness asserts that it
/// costs at most two, through the same enumeration that the passing harnesses
/// use. Kani must report the assertion as failed. If it does not, the
/// toolchain or the enumeration is not checking the code it is pointed at, and
/// no passing result in this package is evidence.
#[kani::proof]
#[kani::unwind(5)]
fn negative_control_false_wcet_bound() {
    with_plain_op(&mut |b| {
        let chunk = chunk_of(vec![Op::Stream, b, Op::Reset], BlockType::Stream, 0);
        let w = verify::wcet_stream_iteration_with_cost_model(&chunk, &model(unit_cycles, 1), &[]);
        assert!(w.unwrap_or(0) <= 2, "negative control: this bound is false");
    });
}
