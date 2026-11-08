//! Where a `Text` value can reach a comparison (V0.3.x Order 6).
//!
//! The virtual machine compares two `Text` values by their contents. This
//! backend carries a string literal as the address of a length-prefixed record
//! it lays out itself, and a `Text` from anywhere else as a word whose layout it
//! does not own. A comparison of two such words as integers compares addresses,
//! which gives a wrong answer silently. Order 6's construct census found that
//! `"ab" == "ab"` was false natively.
//!
//! [`comparisons_reached_by_text`] is a forward dataflow over one chunk's
//! structured control flow. It marks a stack slot or local as possibly `Text`
//! when its value can come from a string constant, a `Text`-returning call, a
//! `Text` parameter, a `Text` native result, a `Text` field or element read,
//! or a boxed read whose kind is unknown. Marks join by union, so the
//! analysis terminates, and an unmarked slot is proven not to hold `Text`. The
//! successor rules are the coroutine type analysis's, the structured jumps of
//! verified bytecode.
//!
//! The emitter compares a marked pair by contents only when both operands are
//! literal records it laid out, and refuses every other marked comparison.

use keleusma::bytecode::{
    ArrayElem, BlockType, Chunk, ChunkSignature, ConstValue, EnumField, Op, StructField,
    TupleField, TypeTag, WireShape,
};
use keleusma::value_layout::ScalarKind;
use std::collections::{BTreeSet, VecDeque};

fn is_text(shape: &WireShape) -> bool {
    matches!(shape, WireShape::Scalar { kind } if ScalarKind::from_tag(*kind) == Some(ScalarKind::Text))
}

#[derive(Clone, PartialEq, Eq)]
struct State {
    stack: Vec<bool>,
    locals: Vec<bool>,
}

impl State {
    /// Union `other` into `self`, reporting whether anything changed.
    fn join(&mut self, other: &State) -> bool {
        let mut changed = false;
        if self.stack.len() < other.stack.len() {
            self.stack.resize(other.stack.len(), false);
            changed = true;
        }
        for (mine, theirs) in self.stack.iter_mut().zip(&other.stack) {
            if *theirs && !*mine {
                *mine = true;
                changed = true;
            }
        }
        for (mine, theirs) in self.locals.iter_mut().zip(&other.locals) {
            if *theirs && !*mine {
                *mine = true;
                changed = true;
            }
        }
        changed
    }
}

/// Whether the first value `op` pushes may be `Text`. Every later push of a
/// multi-push instruction is a flag or a diagnostic word and never `Text`.
fn produces_text(
    op: &Op,
    chunk: &Chunk,
    state: &State,
    signatures: &[ChunkSignature],
    native_shapes: &[WireShape],
    reply_text: bool,
) -> bool {
    let text_kind = |k: &ScalarKind| *k == ScalarKind::Text;
    match op {
        Op::Const(c) => matches!(
            chunk.constants.get(*c as usize),
            Some(ConstValue::StaticStr(_))
        ),
        Op::GetLocal(n) => state.locals.get(*n as usize).copied().unwrap_or(true),
        Op::Call(t, _) => signatures.get(*t as usize).is_none_or(|s| is_text(&s.ret)),
        Op::CallVerifiedNative(i, _) | Op::CallExternalNative(i, _) => {
            native_shapes.get(usize::from(*i)).is_some_and(is_text)
        }
        Op::GetField(StructField::Flat { kind, .. })
        | Op::GetTupleField(TupleField::Flat { kind, .. })
        | Op::GetEnumField(EnumField::Flat { kind, .. })
        | Op::GetIndex(ArrayElem::Flat { kind }) => text_kind(kind),
        Op::GetField(StructField::FlatNested { .. })
        | Op::GetTupleField(TupleField::FlatNested { .. })
        | Op::GetEnumField(EnumField::FlatNested { .. })
        | Op::GetIndex(ArrayElem::FlatNested { .. }) => false,
        // A boxed read's kind is not in the instruction, so it may be `Text`.
        Op::GetField(_) | Op::GetTupleField(_) | Op::GetEnumField(_) | Op::GetIndex(_) => true,
        // The value a suspension resumes with is the host's reply.
        Op::Yield => reply_text,
        _ => false,
    }
}

/// The instruction indices of comparisons in `chunk` an operand of which may
/// be `Text`.
pub(crate) fn comparisons_reached_by_text(
    chunk: &Chunk,
    own: Option<&ChunkSignature>,
    signatures: &[ChunkSignature],
    native_shapes: &[WireShape],
) -> BTreeSet<usize> {
    let n = chunk.ops.len();
    let locals = usize::from(chunk.local_count).max(usize::from(chunk.param_count));
    let mut entry = State {
        stack: Vec::new(),
        locals: vec![false; locals],
    };
    for i in 0..usize::from(chunk.param_count) {
        let declared = own.and_then(|s| s.params.get(i)).is_some_and(is_text);
        let tagged = chunk.param_types.get(i) == Some(&TypeTag::Text);
        entry.locals[i] = declared || tagged;
    }
    let reply_text = chunk.block_type != BlockType::Func && entry.locals.first() == Some(&true);
    let mut states: Vec<Option<State>> = vec![None; n];
    let mut found = BTreeSet::new();
    if n == 0 {
        return found;
    }
    states[0] = Some(entry);
    let mut work: VecDeque<usize> = VecDeque::from([0]);
    // Each state only gains marks, and the marks are finite, so the queue
    // empties. The bound is a second guarantee, generous by a wide margin.
    let mut budget = n.saturating_mul(locals + 64).saturating_mul(8).max(1024);
    while let Some(ip) = work.pop_front() {
        budget = budget.saturating_sub(1);
        if budget == 0 {
            // Never observed. Treating every comparison as reached is the
            // conservative answer if the bound is ever met.
            return (0..n)
                .filter(|&i| {
                    matches!(
                        chunk.ops[i],
                        Op::CmpEq | Op::CmpNe | Op::CmpLt | Op::CmpGt | Op::CmpLe | Op::CmpGe
                    )
                })
                .collect();
        }
        let Some(state) = states[ip].clone() else {
            continue;
        };
        let op = &chunk.ops[ip];
        if matches!(
            op,
            Op::CmpEq | Op::CmpNe | Op::CmpLt | Op::CmpGt | Op::CmpLe | Op::CmpGe
        ) {
            let d = state.stack.len();
            let reached = (d >= 1 && state.stack[d - 1]) || (d >= 2 && state.stack[d - 2]);
            // A stack shallower than two cannot occur in verified code. If it
            // does, the comparison is treated as reached.
            if reached || d < 2 {
                found.insert(ip);
            }
        }
        let mut next = state.clone();
        if let Op::SetLocal(l) = op {
            let top = next.stack.last().copied().unwrap_or(true);
            if let Some(slot) = next.locals.get_mut(*l as usize) {
                *slot = *slot || top;
            }
        }
        let first = produces_text(op, chunk, &state, signatures, native_shapes, reply_text);
        if !matches!(op, Op::Trap(_)) {
            let (required, delta) = keleusma::verify::op_depth_effect(op, chunk);
            for _ in 0..required.max(0) {
                next.stack.pop();
            }
            for k in 0..(required + delta).max(0) {
                next.stack.push(k == 0 && first);
            }
        }
        // Both copies `Dup` pushes carry the original's mark, where the rule
        // above would mark only the first.
        if matches!(op, Op::Dup) {
            let top = state.stack.last().copied().unwrap_or(true);
            let d = next.stack.len();
            for slot in &mut next.stack[d.saturating_sub(2)..] {
                *slot = top;
            }
        }
        if matches!(op, Op::Reset) {
            next.stack.clear();
        }
        let successors: Vec<usize> = match op {
            Op::Return | Op::Trap(_) => Vec::new(),
            Op::Else(t) | Op::EndLoop(t) | Op::Break(t) => vec![*t as usize],
            Op::If(t) | Op::BreakIf(t) => vec![ip + 1, *t as usize],
            Op::Reset => vec![1],
            _ => vec![ip + 1],
        };
        for s in successors {
            if s >= n {
                continue;
            }
            let changed = match &mut states[s] {
                Some(old) => old.join(&next),
                slot @ None => {
                    *slot = Some(next.clone());
                    true
                }
            };
            if changed {
                work.push_back(s);
            }
        }
    }
    found
}
