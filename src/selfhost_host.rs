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
use alloc::format;
use alloc::string::String;
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
            // A DIAGNOSTIC RECORD FROM THE STAGE, not a parse record. `parse.kel` reports
            // its own capacity limits here rather than letting them surface as raw virtual
            // machine traps: four separate causes used to arrive as `IndexOutOfBounds`
            // naming an array's size, and two unrelated limits (locals and operator nesting,
            // both 64) produced the byte-identical message `IndexOutOfBounds(64, 64)`.
            //
            // Panics rather than returning an error, matching the existing failure mode of
            // this driver and of `parse_functions`. Whether these should become a `Result`
            // is a real question and a separate one; it changes a signature that many tests
            // and both compile paths depend on.
            if code <= PARSE_DIAG_TAG_BASE {
                panic!("{}", describe_parse_diagnostic(code, val));
            }
            if on_record(code, val).is_break() {
                return;
            }
        }
        before_resume(vm, shared);
        state = vm
            .resume_with_shared(shared, Value::Int(0))
            .expect("resume parse.kel");
    }
    // Measured cause: an unterminated block. `fn f() -> Word { let x = 1; x` (no closing
    // brace) exhausts the budget rather than reporting anything, because the stage is still
    // waiting for the token that closes the body. The budget itself is almost never the
    // real fault, so it is named last rather than first.
    panic!(
        "parse.kel ran {budget} steps without reaching DONE. The usual cause is an \
         unterminated block, string, or bracket in the input: the parser is still waiting \
         for the token that closes it and never reaches the end of the declaration."
    );
}

/// The tag at or below which a record from `parse.kel` is a diagnostic rather than a parse
/// record. Record tags are non-negative, so no legitimate record can collide.
///
/// Restated here from `pe_tag_base()` in the stage; `the_parse_diagnostic_tag_base_matches`
/// checks the two agree, because a driver that learned the base by observing one would only
/// learn it from a program that had already failed.
pub const PARSE_DIAG_TAG_BASE: i64 = -900;

/// Render a diagnostic record from `parse.kel` as a message that names the cap, the count
/// that exceeded it, and the construct at fault.
///
/// The codes are `pe_opstack`/`pe_bracket`/`pe_locals`/`pe_stmts` in the stage. An unknown
/// code is reported as unknown rather than guessed at: a stage that grew a fifth cause and
/// a driver that silently mapped it onto a fourth would be the misdirecting-diagnostic
/// defect this whole path exists to remove.
pub fn describe_parse_diagnostic(code: i64, detail: i64) -> String {
    match PARSE_DIAG_TAG_BASE - code {
        1 => format!(
            "expression nesting is too deep for parse.kel: it reached {detail} pending \
             operators and `ops.opstack` holds {}. Nesting counts parentheses, calls, \
             struct and array literals, and pending binary operators. Split the expression.",
            PARSE_OPSTACK_CAP
        ),
        2 => format!(
            "unmatched closing bracket at token {detail}: parse.kel reached a `]` or a `)` \
             with no matching opening bracket pending."
        ),
        3 => format!(
            "too many local bindings in one function for parse.kel: it reached {detail} and \
             `stmt.let_names` holds {}. Every `let`, every `for` loop variable, and every \
             pattern binding takes a slot. Split the function.",
            PARSE_LOCALS_CAP
        ),
        4 => format!(
            "too many statements in one body for parse.kel: it reached {detail} and \
             `stmt.stmt_kind` holds {}. Split the body.",
            PARSE_STMTS_CAP
        ),
        5 => format!(
            "too many parameters on one function for parse.kel: it reached {detail} and \
             `ps.pnames` holds {PARSE_PARAMS_CAP}. Pass a data block or a composite instead."
        ),
        6 => format!(
            "`if` nesting is too deep for parse.kel: it reached {detail} and `branch.if_seq` \
             holds {PARSE_IF_DEPTH_CAP}. Flatten the branches or split the function."
        ),
        7 => format!(
            "`for` nesting is too deep for parse.kel: it reached {detail} and `forst.for_seq` \
             holds {PARSE_FOR_DEPTH_CAP}. Move an inner loop into its own function."
        ),
        8 => format!(
            "array-literal nesting is too deep for parse.kel: it reached {detail} and \
             `call.al_count` holds {PARSE_ARRAY_NEST_CAP}. Bind an inner array with `let` first."
        ),
        10 => format!(
            "call nesting is too deep for parse.kel: it reached {detail} and `call.call_chunk` \
             holds {PARSE_CALL_DEPTH_CAP}. Bind an inner call with `let` first."
        ),
        11 => format!(
            "too many data-block fields in one program for parse.kel: it reached {detail} and \
             `fields.ffield` holds {PARSE_FIELDS_CAP}. Like the enum bound this is a TOTAL \
             ACROSS THE WHOLE PROGRAM, not one block."
        ),
        9 => format!(
            "too many enum variants for parse.kel: it reached {detail} and `enums.evar` holds \
             {PARSE_VARIANTS_CAP}. This is a TOTAL ACROSS THE WHOLE PROGRAM, not one enum: 128 \
             enums of two variants reach it at the same point as one enum of 257."
        ),
        other => format!(
            "parse.kel reported diagnostic code {other} (detail {detail}), which this driver \
             does not know. The stage has grown a cause that `describe_parse_diagnostic` was \
             not taught."
        ),
    }
}

/// The usable capacities behind the three counted limits, restated from `pe_cap_op`,
/// `pe_cap_let` and `pe_cap_stmt` in the stage. `the_parse_guard_caps_match_their_arrays`
/// checks all three against the array declarations they guard.
pub const PARSE_OPSTACK_CAP: usize = 64;
/// See [`PARSE_OPSTACK_CAP`].
pub const PARSE_LOCALS_CAP: usize = 64;
/// See [`PARSE_OPSTACK_CAP`].
pub const PARSE_STMTS_CAP: usize = 256;
/// Parameters on one function. See [`PARSE_OPSTACK_CAP`].
pub const PARSE_PARAMS_CAP: usize = 32;
/// Nesting depth of `if`. See [`PARSE_OPSTACK_CAP`].
pub const PARSE_IF_DEPTH_CAP: usize = 32;
/// Nesting depth of `for`. See [`PARSE_OPSTACK_CAP`].
pub const PARSE_FOR_DEPTH_CAP: usize = 8;
/// Nesting depth of array literals. See [`PARSE_OPSTACK_CAP`].
pub const PARSE_ARRAY_NEST_CAP: usize = 8;
/// Enum variants **across the whole program**, not per enum. See [`PARSE_OPSTACK_CAP`].
pub const PARSE_VARIANTS_CAP: usize = 256;
/// Nesting depth of calls. See [`PARSE_OPSTACK_CAP`].
pub const PARSE_CALL_DEPTH_CAP: usize = 8;
/// Data-block fields **across the whole program**, not per block. See [`PARSE_OPSTACK_CAP`].
pub const PARSE_FIELDS_CAP: usize = 512;

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

// THE SHARED BLOCK IS ADDRESSED BY SLOT, AND THIS IS THE ONE PLACE THAT SAYS WHERE
// EACH FIELD SITS.
//
// These were seven independent arithmetic literals (`1 + 40960 + 2 + 256 + 7`), and
// THREE TEST HARNESSES CARRIED THEIR OWN COPIES of the same arithmetic. Widening
// `toks.chunks` moved the stage's fields and left all three copies seeding the
// keyword and type ids at the old slots, so `parse.kel` read zero for `word_id` and
// sized every field as one byte. Sixty-eight tests failed, none of them naming a
// slot: they reported struct byte sizes of 1 instead of 8 and a scalar kind of
// `Unit` instead of `Int`.
//
// So the offsets are PUBLIC and CHAINED. Public, so the harnesses use these rather
// than restating them -- the same reasoning as `analyze_class`, where a second copy
// of the class table had already drifted from the original. Chained, so a widened
// array moves everything after it automatically.
// `the_driver_shared_slots_match_the_stage` checks the chain against the stage's
// declaration order, and `no_other_file_restates_the_shared_layout` checks that the
// copies have not come back.

/// How many entries `toks.packed` in `parse.kel` holds, one token each.
///
/// Measured against the corpus: `parse.kel` itself is the largest at 32,907 tokens,
/// which is 80% of this. It is the next of the stage's arrays likely to bind.
pub const PARSE_TOKEN_CAP: usize = 40960;

/// How many entries `toks.chunks` in `parse.kel` holds.
///
/// Restated here rather than parsed out of the stage, because a driver that
/// discovered the cap by overflowing it would already have written past the array.
/// `the_driver_array_caps_match_the_stage` checks the two agree.
///
/// **Sized from measurement, not from the stage that needed it.** The corpus worst
/// case is `wire.kel` at 486 chunks; the next largest is `parse.kel` at 108, which
/// grew from 94 in a single increment. 512 would have left `wire` twenty-six chunks
/// of margin, which one increment can consume.
pub const PARSE_CHUNK_CAP: usize = 1024;

// THE LEXER'S SHARED BLOCK, ON THE SAME TERMS AS THE PARSER'S.
//
// `lexer.kel`'s `src` block was restated in FOUR places -- this driver, two test
// harnesses, and `compiler/src/main.rs` -- exactly as the parser's was in five. The
// parser's copies were found only because widening an array moved the block and
// sixty-eight tests failed; the lexer's block has not moved, so its copies had failed
// nothing and would have behaved identically the day it did.
//
// Fixing the instance leaves the class. Both layouts are published and chained here,
// and `no_other_file_restates_the_shared_layout` looks for restatements of either.

/// How many source bytes `src.bytes` in `lexer.kel` holds.
///
/// Sized to hold the largest stage source. `parse.kel` is the largest at roughly
/// 100 KB; the wire-format v2 twenty-four-bit data operands raised the shared-segment
/// ceiling from 64 KB to 16 MB, so this may exceed 64 KB.
pub const LEX_SOURCE_CAP: usize = 393216;

/// How many distinct identifiers `src.istart`/`src.ilen` in `lexer.kel` hold.
pub const LEX_INTERN_CAP: usize = 1280;

/// Slot of `src.len`, the source byte count.
pub const BR_LEX_LEN: usize = 0;

/// Slot of `src.bytes[0]`, the source bytes the host places for the lexer to scan.
pub const BR_LEX_BYTES: usize = BR_LEX_LEN + 1;

/// Slot of `src.istart[0]`, each interned identifier's byte offset into the source.
pub const BR_LEX_ISTART: usize = BR_LEX_BYTES + LEX_SOURCE_CAP;

/// Slot of `src.ilen[0]`, each interned identifier's byte length.
pub const BR_LEX_ILEN: usize = BR_LEX_ISTART + LEX_INTERN_CAP;

/// Slot of `src.icount`, how many identifiers the lexer interned.
pub const BR_LEX_ICOUNT: usize = BR_LEX_ILEN + LEX_INTERN_CAP;

/// Slot of `toks.len`, the token count `parse.kel` compares its cursor against to
/// find end of input.
pub const BR_P_LEN: usize = 0;

/// Slot of `toks.packed[0]`. Each entry is one `kind + payload * 256` token word.
pub const BR_P_PACKED: usize = BR_P_LEN + 1;

/// Slot of `toks.limit_id`, the interned id of the `limit` keyword.
pub const BR_P_LIMIT_ID: usize = BR_P_PACKED + PARSE_TOKEN_CAP;

/// Slot of `toks.chunk_count`, how many entries of the chunk table are seeded.
pub const BR_P_CHUNK_COUNT: usize = BR_P_LIMIT_ID + 1;

/// Slot of `toks.chunks[0]`, the call-resolution table in the module's chunk order.
pub const BR_P_CHUNKS: usize = BR_P_CHUNK_COUNT + 1;

/// Slot of `toks.require_id`, the interned id of the `require` keyword.
pub const BR_P_REQUIRE_ID: usize = BR_P_CHUNKS + PARSE_CHUNK_CAP;

/// Slot of `toks.word_id`, the interned id of the `Word` type name.
pub const BR_P_WORD_ID: usize = BR_P_REQUIRE_ID + 1;

/// Slot of `toks.byte_id`, the interned id of the `Byte` type name.
pub const BR_P_BYTE_ID: usize = BR_P_WORD_ID + 1;

/// Slot of `toks.bool_id`, the interned id of the `Bool` type name.
pub const BR_P_BOOL_ID: usize = BR_P_BYTE_ID + 1;

/// The interned id of the eager boolean operator `and`, which is seeded by a
/// harness rather than by this driver. Named here anyway, because the alternative
/// is a harness restating the arithmetic -- which is exactly what broke.
pub const BR_P_AND_ID: usize = BR_P_BOOL_ID + 1;

/// The interned id of the eager boolean operator `or`. See [`BR_P_AND_ID`].
pub const BR_P_OR_ID: usize = BR_P_AND_ID + 1;

/// The absolute token index that `packed[0]` holds. Zero means the whole stream
/// is seeded, which is what the ordinary driver does.
pub const BR_P_BASE: usize = BR_P_OR_ID + 1;

/// The cursor, written back by the stage on every token read, so a host feeding a
/// window knows where to slide it without the stage needing a protocol to ask.
pub const BR_P_AT: usize = BR_P_BASE + 1;

#[cfg(test)]
mod shared_layout_tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec::Vec;

    /// The slot offset and length of every field in `parse.kel`'s `shared data toks`
    /// block, derived by reading the stage.
    ///
    /// **The constants above are addressed BY SLOT, and the block is written in
    /// declaration order, so one miscounted field silently shifts every field after
    /// it.** That has already happened once in this file's history and broke four
    /// tests. Restating the layout as arithmetic is what makes it possible; deriving
    /// it here is what makes a restatement that has drifted impossible to land.
    fn stage_layout() -> Vec<(String, usize, usize)> {
        const STAGE: &str = include_str!("selfhost/kel/parse.kel");
        let body = STAGE
            .split_once("shared data toks {")
            .expect("parse.kel declares `shared data toks`")
            .1
            .split_once("\n}")
            .expect("the block is closed")
            .0;
        let mut out = Vec::new();
        let mut off = 0usize;
        for line in body.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with("//") {
                continue;
            }
            let Some((name, rest)) = t.split_once(':') else {
                panic!("unparsed line in `shared data toks`: {t:?}");
            };
            let rest = rest.trim().trim_end_matches(',').trim();
            let len = if let Some(inner) = rest.strip_prefix("[Word;") {
                inner
                    .trim_end_matches(']')
                    .trim()
                    .parse()
                    .unwrap_or_else(|_| panic!("unparsed array length in {t:?}"))
            } else if rest == "Word" {
                1
            } else {
                panic!("unparsed field type in {t:?}");
            };
            out.push((String::from(name.trim()), off, len));
            off += len;
        }
        out
    }

    fn slot_of(layout: &[(String, usize, usize)], name: &str) -> usize {
        layout
            .iter()
            .find(|(n, _, _)| n == name)
            .unwrap_or_else(|| panic!("`shared data toks` has no field `{name}`"))
            .1
    }

    fn len_of(layout: &[(String, usize, usize)], name: &str) -> usize {
        layout
            .iter()
            .find(|(n, _, _)| n == name)
            .unwrap_or_else(|| panic!("`shared data toks` has no field `{name}`"))
            .2
    }

    /// **Every `BR_P_*` constant must equal the slot the stage actually puts that
    /// field at.**
    ///
    /// Verified by mutation: changing any array length in `shared data toks`, or
    /// reordering two fields, fails this by name. Without it, widening `toks.chunks`
    /// means editing seven arithmetic literals consistently and finding out later.
    #[test]
    fn the_driver_shared_slots_match_the_stage() {
        let layout = stage_layout();
        let cases: &[(&str, &str, usize)] = &[
            ("len", "BR_P_LEN", BR_P_LEN),
            ("packed", "BR_P_PACKED", BR_P_PACKED),
            ("limit_id", "BR_P_LIMIT_ID", BR_P_LIMIT_ID),
            ("chunk_count", "BR_P_CHUNK_COUNT", BR_P_CHUNK_COUNT),
            ("chunks", "BR_P_CHUNKS", BR_P_CHUNKS),
            ("require_id", "BR_P_REQUIRE_ID", BR_P_REQUIRE_ID),
            ("word_id", "BR_P_WORD_ID", BR_P_WORD_ID),
            ("byte_id", "BR_P_BYTE_ID", BR_P_BYTE_ID),
            ("bool_id", "BR_P_BOOL_ID", BR_P_BOOL_ID),
            ("and_id", "BR_P_AND_ID", BR_P_AND_ID),
            ("or_id", "BR_P_OR_ID", BR_P_OR_ID),
            ("base", "BR_P_BASE", BR_P_BASE),
            ("at", "BR_P_AT", BR_P_AT),
        ];
        for &(field, konst, value) in cases {
            assert_eq!(
                slot_of(&layout, field),
                value,
                "`{konst}` is {value} but the stage puts `toks.{field}` at slot {}. The \
                 block is addressed by slot, so every field after the first mismatch is \
                 also wrong.",
                slot_of(&layout, field)
            );
        }
    }

    /// **A CAP IS A FAMILY, AND IT APPEARS IN THREE PLACES.**
    ///
    /// Raising `toks.chunks` from 256 to 1024 did not admit `wire.kel`. The chunk
    /// index also addresses the six `chunkret.ret_*` arrays, which were 256 as well,
    /// and it also bounds two `for i in 0..toks.chunk_count limit 256` loops. Widening
    /// the one array named "the chunk table" moved the wall from an index trap to a
    /// loop-limit trap and then to a different index trap, each naming a size and
    /// none naming the cap.
    ///
    /// **This is the second family in two increments.** The eight local-binding arrays
    /// were the first. So the check derives the family from the stage rather than
    /// listing it: anything the stage indexes with a chunk index, and any loop it
    /// bounds by the chunk count, must admit as many chunks as the driver accepts.
    #[test]
    fn every_chunk_indexed_array_admits_the_chunk_cap() {
        const STAGE: &str = include_str!("selfhost/kel/parse.kel");
        let layout = stage_layout();
        assert_eq!(
            len_of(&layout, "chunks"),
            PARSE_CHUNK_CAP,
            "the family below is checked against the driver's cap, so this must hold first"
        );

        // Arrays addressed by a chunk index. `ps.cur_chunk` is the writing site and
        // `call.call_chunk[..]` the reading one; both are chunk numbers.
        let mut checked = 0usize;
        for idx in ["ps.cur_chunk", "call.call_chunk["] {
            for (i, _) in STAGE.match_indices(idx) {
                // The index expression is preceded by the `[` that opens the subscript,
                // and the array name is the identifier before THAT. Reading the
                // identifier straight back from `i` finds the `[` first and yields the
                // empty string, which is what the first version of this did: it found
                // zero arrays and the vacuity assertion below caught it.
                let Some(before) = STAGE[..i].strip_suffix('[') else {
                    continue;
                };
                let name: alloc::string::String = before
                    .chars()
                    .rev()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                if name.is_empty() {
                    continue;
                }
                let Some(len) = declared_array_len(STAGE, &name) else {
                    continue;
                };
                assert!(
                    len >= PARSE_CHUNK_CAP,
                    "`{name}` is indexed by a chunk number but holds only {len}, and the \
                     driver accepts {PARSE_CHUNK_CAP} chunks. Widening `toks.chunks` alone \
                     leaves the wall exactly where it was."
                );
                checked += 1;
            }
        }
        assert!(
            checked >= 6,
            "only {checked} chunk-indexed arrays were found; the derivation has stopped \
             seeing the family and this check has gone vacuous"
        );

        // Loops bounded by the chunk COUNT. A `limit` below the cap turns an admitted
        // program into `LoopLimitExceeded`, which names neither the chunk table nor the
        // program's function count.
        let mut loops = 0usize;
        for line in STAGE.lines() {
            let t = line.trim();
            let Some(rest) = t.strip_prefix("for i in 0..toks.chunk_count limit ") else {
                continue;
            };
            let n: usize = rest
                .trim_end_matches(" {")
                .trim()
                .parse()
                .unwrap_or_else(|_| panic!("unparsed loop limit in {t:?}"));
            assert!(
                n >= PARSE_CHUNK_CAP,
                "a loop over the chunk table is bounded at {n} while the driver accepts \
                 {PARSE_CHUNK_CAP} chunks; a program between the two traps with \
                 `LoopLimitExceeded`"
            );
            loops += 1;
        }
        assert!(
            loops >= 2,
            "only {loops} chunk-count loops were found; the derivation has gone vacuous"
        );
    }

    /// The declared length of `name` as an array in `src`, or `None`.
    fn declared_array_len(src: &str, name: &str) -> Option<usize> {
        src.lines()
            .map(str::trim)
            .find(|l| l.starts_with(&alloc::format!("{name}: [Word;")))
            .and_then(|l| l.split(';').nth(1))
            .and_then(|t| t.split(']').next())
            .and_then(|n| n.trim().parse().ok())
    }

    /// **THE LEXER'S BLOCK ON THE SAME TERMS**, because the parser's copies were found
    /// only by a change that moved the block, and the lexer's block has not moved yet.
    ///
    /// Four files restated it: this driver, two harnesses, and `compiler/src/main.rs`.
    /// None had failed anything, which is exactly the state the parser's five copies were
    /// in the day before the chunk table was widened.
    #[test]
    fn the_lexer_shared_slots_match_the_stage() {
        const STAGE: &str = include_str!("selfhost/kel/lexer.kel");
        let body = STAGE
            .split_once("shared data src {")
            .expect("lexer.kel declares `shared data src`")
            .1
            .split_once("\n}")
            .expect("the block is closed")
            .0;
        let mut off = 0usize;
        let mut at: alloc::vec::Vec<(alloc::string::String, usize, usize)> = Vec::new();
        for line in body.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with("//") {
                continue;
            }
            let (name, rest) = t
                .split_once(':')
                .unwrap_or_else(|| panic!("unparsed: {t:?}"));
            let rest = rest.trim().trim_end_matches(',').trim();
            // `bytes` is `[Byte; N]`; every other field is `Word`-shaped. Both occupy one
            // slot per element in the shared segment.
            let len = if let Some(i) = rest.strip_prefix("[Byte;").or(rest.strip_prefix("[Word;")) {
                i.trim_end_matches(']').trim().parse().expect("length")
            } else {
                1
            };
            at.push((alloc::string::String::from(name.trim()), off, len));
            off += len;
        }
        let slot = |n: &str| at.iter().find(|(x, _, _)| x == n).expect("field").1;
        let len = |n: &str| at.iter().find(|(x, _, _)| x == n).expect("field").2;

        assert_eq!(
            len("bytes"),
            LEX_SOURCE_CAP,
            "`src.bytes` and LEX_SOURCE_CAP differ"
        );
        assert_eq!(
            len("istart"),
            LEX_INTERN_CAP,
            "`src.istart` and LEX_INTERN_CAP differ"
        );
        assert_eq!(
            len("ilen"),
            LEX_INTERN_CAP,
            "`src.ilen` and LEX_INTERN_CAP differ"
        );
        for (field, konst, value) in [
            ("len", "BR_LEX_LEN", BR_LEX_LEN),
            ("bytes", "BR_LEX_BYTES", BR_LEX_BYTES),
            ("istart", "BR_LEX_ISTART", BR_LEX_ISTART),
            ("ilen", "BR_LEX_ILEN", BR_LEX_ILEN),
            ("icount", "BR_LEX_ICOUNT", BR_LEX_ICOUNT),
        ] {
            assert_eq!(
                slot(field),
                value,
                "`{konst}` is {value} but the stage puts `src.{field}` at slot {}",
                slot(field)
            );
        }
    }

    /// The two capacities the driver restates are the stage's own array lengths.
    ///
    /// A driver that learned either by overflowing it would already have written past
    /// the array, which is why they are restated rather than discovered.
    #[test]
    fn the_driver_array_caps_match_the_stage() {
        let layout = stage_layout();
        assert_eq!(
            len_of(&layout, "chunks"),
            PARSE_CHUNK_CAP,
            "`toks.chunks` holds {} entries and the driver refuses at {PARSE_CHUNK_CAP}",
            len_of(&layout, "chunks")
        );
        assert_eq!(
            len_of(&layout, "packed"),
            PARSE_TOKEN_CAP,
            "`toks.packed` holds {} entries and the driver's cap is {PARSE_TOKEN_CAP}",
            len_of(&layout, "packed")
        );
    }
}
