//! Shared host-side driver for the self-hosted `.kel` stages.
//!
//! The loop that drives `parse.kel` and reads its emitted records was historically copied
//! into every consumer — the self-host integration tests and the detached `compiler/`
//! subproject — six times over. That duplication is a drift hazard: it is the same class
//! that let the `compiler/src/selfhost.rs` decoder fall behind and ship `unknown op tag 62`
//! into v0.2.3 (process audit item 4). This module holds ONE copy of the record-reading
//! loop, so a change to the parse-to-host record transport — notably the P11 Option E
//! two-word `(tag, payload)` encoding — is a single edit here rather than six lockstep ones.
//!
//! Gated behind `compile` + `verify` (the features the self-host path already requires); it
//! is a harness helper for the self-host tooling, not part of the stable runtime API.

use crate::bytecode::Value;
use crate::vm::{Vm, VmState};
use core::ops::ControlFlow;

/// Drive a running `parse.kel` coroutine, invoking `on_record(code, val)` for each record it
/// emits until the callback returns [`ControlFlow::Break`] — which the caller signals on the
/// DONE record — or until `budget` iterations elapse (a divergence guard; `parse.kel` must
/// reach DONE first).
///
/// `state` is the coroutine state immediately after the caller's `call_with_shared`. This
/// function owns the record transport: today each record is one yielded word `code + val*64`;
/// the P11 Option E change to a two-word `(tag, payload)` pair lands here and nowhere else.
///
/// The caller keeps ownership of its accumulator state by capturing it in `on_record`; this
/// driver borrows only `vm` and `shared`, so a callback must not itself touch them.
pub fn drive_parse_records<F>(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    state: VmState,
    budget: usize,
    on_record: F,
) where
    F: FnMut(i64, i64) -> ControlFlow<()>,
{
    drive_parse_records_with(vm, shared, state, budget, on_record, |_, _| {});
}

/// [`drive_parse_records`] with a hook run before every resume.
///
/// **ONE LOOP, NOT TWO.** A fused driver feeding tokens through a sliding window
/// must correct the window BEFORE the parser is resumed, because the parser reads
/// at its cursor the moment it runs -- a window fixed afterwards is fixed too
/// late. That needs a hook inside this loop, and the alternative was a second copy
/// of the Option E transport in the caller.
///
/// This file already records what a second copy costs: the class table duplicated
/// into a test had drifted, keeping a catch-all after this one became exhaustive,
/// so the differential that was supposed to be the oracle ran against the
/// unrepaired table. The transport here is subtler than that table.
pub fn drive_parse_records_with<F, B>(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    state: VmState,
    budget: usize,
    mut on_record: F,
    mut before_resume: B,
) where
    F: FnMut(i64, i64) -> ControlFlow<()>,
    B: FnMut(&mut Vm<'_, '_>, &mut [u8]),
{
    let mut state = state;
    for _ in 0..budget {
        if let VmState::Yielded(Value::Int(t)) = state {
            // Option E two-word transport: the tag word `t` is followed by its payload word on
            // the next yield. Read it now.
            let arg = loop {
                before_resume(vm, shared);
                state = vm
                    .resume_with_shared(shared, Value::Int(0))
                    .expect("resume parse.kel");
                match state {
                    VmState::Yielded(Value::Int(a)) => break a,
                    // The productive `loop main` RESETs between its per-iteration yields, so the
                    // payload word is the next Yielded after any intervening RESETs.
                    VmState::Reset => {}
                    other => panic!("parse.kel: expected a record payload word, got {other:?}"),
                }
            };
            // The -1 sentinel marks an un-migrated (still packed) record: recover the classic
            // tag = w % 64, payload = w / 64 split. A migrated emit site supplies its full-word
            // payload directly with a raw (possibly >= 64) tag.
            let (code, val) = if arg == -1 {
                (t.rem_euclid(64), t.div_euclid(64))
            } else {
                (t, arg)
            };
            if on_record(code, val).is_break() {
                return;
            }
        }
        before_resume(vm, shared);
        state = vm
            .resume_with_shared(shared, Value::Int(0))
            .expect("resume parse.kel");
    }
    panic!("parse.kel did not reach DONE within its iteration budget");
}

/// Control-flow class and target for one opcode, as `analyze.kel` consumes them.
///
/// **Public so there is ONE class table, not two.** A second copy lived in
/// `tests/selfhost_codegen.rs` and had already drifted from this one: it kept a
/// `_ => (0, 0)` catch-all after this function was made exhaustive, and passed
/// `0` where this passes the real `EndLoop`/`Break`/`BreakIf` target. The
/// differential that is supposed to be the oracle was therefore running against
/// the unrepaired table. Same reasoning as the per-item seed accessors: a
/// reconstruction of an encoding is free to drift from the encoding.
pub fn analyze_class(op: &crate::bytecode::Op) -> (i64, i64) {
    use crate::bytecode::Op;
    match op {
        Op::If(t) => (1, *t as i64),
        Op::Else(e) => (2, *e as i64),
        Op::EndIf => (3, 0),
        Op::Loop(x) => (4, *x as i64),
        Op::EndLoop(t) => (5, *t as i64),
        Op::Break(t) => (6, *t as i64),
        Op::BreakIf(t) => (7, *t as i64),
        // Class 8 is PATH EXIT, not "trap". Both `Trap` and `Return` end the
        // current path without transferring control to an enclosing loop, so
        // they share it and no tenth class is needed.
        //
        // `Return` was in the plain group below until 2026-08-16, mirroring a
        // reference that let it fall through its own catch-all. That costed and
        // sized a multiheaded dispatch as though every head ran in sequence.
        Op::Trap(_) | Op::Return => (8, 0),
        Op::Call(_, _) => (9, 0),
        // EVERY REMAINING OPCODE IS LISTED, and the list is the point.
        //
        // This was `_ => (0, 0)`. A control-flow opcode added later and not
        // classified above would have fallen through it and become "plain"
        // SILENTLY: `analyze.kel` rebuilds the control-flow graph by following
        // the `If`/`Loop`/`EndLoop`/`Break` targets this function returns, so a
        // missing arm is a graph missing an edge, and a bound extracted from
        // that graph is finite and WRONG rather than absent. Nothing downstream
        // can distinguish "plain opcode" from "unclassified opcode".
        //
        // A test cannot close that hole, because it cannot fail for an opcode
        // nobody has written yet. The compiler can: adding a variant to `Op`
        // now fails to build here until someone decides which class it belongs
        // to. That is the whole change -- the classification is unaltered and
        // every opcode below still maps to `(0, 0)`, exactly as the catch-all
        // did.
        Op::Const(..)
        | Op::GetLocal(..)
        | Op::SetLocal(..)
        | Op::GetData(..)
        | Op::SetData(..)
        | Op::GetDataIndexed(..)
        | Op::SetDataIndexed(..)
        | Op::BoundsCheck(..)
        | Op::Add
        | Op::Sub
        | Op::Mul
        | Op::Div
        | Op::Mod
        | Op::Neg
        | Op::CmpEq
        | Op::CmpNe
        | Op::CmpLt
        | Op::CmpGt
        | Op::CmpLe
        | Op::CmpGe
        | Op::Not
        | Op::Stream
        | Op::Reset
        | Op::Yield
        | Op::Dup
        | Op::NewComposite(..)
        | Op::GetField(..)
        | Op::GetIndex(..)
        | Op::GetTupleField(..)
        | Op::GetEnumField(..)
        | Op::Len
        | Op::IsEnum(..)
        | Op::IsStruct(..)
        | Op::IntToFloat
        | Op::FloatToInt
        | Op::WordToByte
        | Op::ByteToWord
        | Op::WordToFixed(..)
        | Op::FixedToWord(..)
        | Op::FixedMul(..)
        | Op::FixedDiv(..)
        | Op::CheckedAdd
        | Op::CheckedSub
        | Op::CheckedMul(..)
        | Op::CheckedNeg
        | Op::CheckedDiv(..)
        | Op::CheckedMod
        | Op::PushImmediate(..)
        | Op::PopN(..)
        | Op::BitAnd
        | Op::BitOr
        | Op::BitXor
        | Op::Shl
        | Op::Shr
        | Op::CallVerifiedNative(..)
        | Op::CallExternalNative(..) => (0, 0),
    }
}

/// Fine-grained op detail for analyze.kel's loop-bound extraction: `(opk, slot, cval, cint)`.
/// `opk` tags the opcode (1 GetLocal, 2 SetLocal, 3 Const, 4 CmpGe, 5 BreakIf, 6 CheckedAdd,
/// 7 PopN, 8 EndLoop, 9 Loop, 0 other); `slot` the GetLocal/SetLocal slot; `cval` the Const
/// integer value or PopN count; `cint` 1 if a Const resolves to an integer.
/// Fine-grained op detail for `analyze.kel`'s loop-bound extraction, as
/// `(opk, slot, cval, cint)`. Public for the same one-encoding reason as
/// [`analyze_class`].
pub fn analyze_opk(
    op: &crate::bytecode::Op,
    chunk: &crate::bytecode::Chunk,
) -> (i64, i64, i64, i64) {
    use crate::bytecode::{ConstValue, Op};
    match op {
        Op::GetLocal(s) => (1, *s as i64, 0, 0),
        Op::SetLocal(s) => (2, *s as i64, 0, 0),
        Op::Const(idx) => match chunk.constants.get(*idx as usize) {
            Some(ConstValue::Int(v)) => (3, 0, *v, 1),
            _ => (3, 0, 0, 0),
        },
        Op::CmpGe => (4, 0, 0, 0),
        Op::BreakIf(_) => (5, 0, 0, 0),
        Op::CheckedAdd => (6, 0, 0, 0),
        Op::PopN(n) => (7, 0, *n as i64, 0),
        Op::EndLoop(_) => (8, 0, 0, 0),
        Op::Loop(_) => (9, 0, 0, 0),
        // EXHAUSTIVE FOR THE SAME REASON AS `analyze_class`, though the failure
        // mode differs and the difference is worth stating.
        //
        // Every `opk` use in `analyze.kel` is a POSITIVE pattern requirement
        // (`wa.opk[ip] == 2`, `== 3`, `== 8`), so an untagged opcode fails to
        // match and the loop-bound shape is simply not recognised -- a bound is
        // not extracted, which is CONSERVATIVE. That is the opposite of
        // `analyze_class`, where a missing arm drops a control-flow edge and
        // yields a bound that is finite and wrong.
        //
        // It is exhaustive anyway, because that argument is REASONING and the
        // compiler can make it unnecessary. A new opcode should be considered
        // for bound extraction as deliberately as for classification, and a
        // catch-all here decides that question by default and silently.
        Op::GetData(..)
        | Op::SetData(..)
        | Op::GetDataIndexed(..)
        | Op::SetDataIndexed(..)
        | Op::BoundsCheck(..)
        | Op::Add
        | Op::Sub
        | Op::Mul
        | Op::Div
        | Op::Mod
        | Op::Neg
        | Op::CmpEq
        | Op::CmpNe
        | Op::CmpLt
        | Op::CmpGt
        | Op::CmpLe
        | Op::Not
        | Op::If(..)
        | Op::Else(..)
        | Op::EndIf
        | Op::Break(..)
        | Op::Stream
        | Op::Reset
        | Op::Call(..)
        | Op::Return
        | Op::Yield
        | Op::Dup
        | Op::NewComposite(..)
        | Op::GetField(..)
        | Op::GetIndex(..)
        | Op::GetTupleField(..)
        | Op::GetEnumField(..)
        | Op::Len
        | Op::IsEnum(..)
        | Op::IsStruct(..)
        | Op::IntToFloat
        | Op::FloatToInt
        | Op::WordToByte
        | Op::ByteToWord
        | Op::WordToFixed(..)
        | Op::FixedToWord(..)
        | Op::FixedMul(..)
        | Op::FixedDiv(..)
        | Op::Trap(..)
        | Op::CheckedSub
        | Op::CheckedMul(..)
        | Op::CheckedNeg
        | Op::CheckedDiv(..)
        | Op::CheckedMod
        | Op::PushImmediate(..)
        | Op::BitAnd
        | Op::BitOr
        | Op::BitXor
        | Op::Shl
        | Op::Shr
        | Op::CallVerifiedNative(..)
        | Op::CallExternalNative(..) => (0, 0, 0, 0),
    }
}
