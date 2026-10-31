//! Lowering of the bundled text operations over `Text<N>` to existing opcodes.
//!
//! # The representation every lowering relies on
//!
//! A `Text<N>` is the flat body `Tuple(Int length, Array[Byte; N])`, so its
//! length is the scalar word at offset zero and its byte `i` is the scalar byte
//! at offset `word_bytes + i`. Both are read with a constant-offset
//! `GetTupleField`, which costs one instruction, allocates nothing, and is
//! bounds-checked by the virtual machine. A byte at a position computed at run
//! time is read from the nested array with the total `GetIndex` form, whose
//! out-of-range result is a zero byte and an outcome flag that is discarded.
//!
//! # The invariant every lowering preserves and assumes
//!
//! **The live length is at most `N`, and every byte at or past it is zero.**
//! The literal cast establishes it, the zero value has it, and each operation
//! below re-establishes it for its result. Concatenation and equality depend
//! on it. Concatenation sums the two operands' bytes position by position,
//! relying on each operand being zero outside its live range, and equality
//! compares whole bodies up to the shorter capacity. A host that passes a
//! `Text<N>` violating it, as an entry argument for example, gets a defined
//! but unspecified result. Every read stays bounds-checked, so the violation
//! cannot reach memory outside the body.
//!
//! # Why the code grows with `N`
//!
//! No opcode writes one byte of a flat body, so each result is built whole,
//! one pushed value per byte followed by one `NewComposite` for the byte array
//! and one for the enclosing tuple. The instruction count is therefore linear
//! in the capacity, and so is the static worst-case execution time. The
//! instruction count of each operation is stated at its lowering and recorded
//! in `docs/decisions/TEXT_CAPACITY_TYPE.md`.

use super::*;
use crate::text_intrinsic::TextOp;
use crate::value_layout::{CompositeKind, ScalarKind};

/// The largest capacity an intrinsic result may have on this target.
///
/// Three limits apply and the smallest wins. A flat body is at most
/// `u16::MAX` bytes because `NewComposite` carries a 16-bit size. Its byte
/// array holds one pushed value per byte under a 16-bit count. The length and
/// every byte position are words, so a capacity must be representable as a
/// non-negative word, which matters on a narrow-word target.
fn max_capacity(word_bytes: usize) -> usize {
    let body_limit = usize::from(u16::MAX).saturating_sub(word_bytes);
    let word_limit = if word_bytes >= 8 {
        usize::MAX
    } else {
        (1usize << (word_bytes * 8 - 1)) - 1
    };
    body_limit.min(word_limit)
}

/// The capacity of a `Text<N>` operand as the compiler infers it.
fn operand_capacity(
    fc: &FuncCompiler,
    op: TextOp,
    name: &str,
    index: usize,
    arg: &Expr,
) -> Result<usize, CompileError> {
    let refuse = |detail: String| CompileError {
        message: format!("`{name}` ({}) argument {index} {detail}", op.name()),
        span: arg.span(),
    };
    let ty = infer_expr_type(fc, arg).map(|t| strip_type_labels(&t));
    match ty {
        Some(TypeExpr::TextN(cap, _)) => match cap.as_lit() {
            Some(n) if n > 0 => usize::try_from(n)
                .map_err(|_| refuse(format!("has capacity {n}, which this host cannot address"))),
            Some(n) => Err(refuse(format!("has the illegal capacity {n}"))),
            None => Err(refuse(format!(
                "has the capacity {cap}, which is unresolved after monomorphization"
            ))),
        },
        _ => Err(refuse(String::from(
            "has no statically known `Text<N>` type, so its capacity cannot be read",
        ))),
    }
}

/// The capacity of the value an intrinsic call produces, when it produces a
/// `Text<N>`, without emitting anything.
fn result_capacity(
    fc: &FuncCompiler,
    op: TextOp,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<Option<usize>, CompileError> {
    let cap = match op {
        TextOp::Len | TextOp::Capacity | TextOp::Eq => return Ok(None),
        TextOp::Slice => operand_capacity(fc, op, name, 0, &args[0])?,
        TextOp::Concat => {
            let a = operand_capacity(fc, op, name, 0, &args[0])?;
            let b = operand_capacity(fc, op, name, 1, &args[1])?;
            a.checked_add(b).ok_or_else(|| CompileError {
                message: format!("`{name}` result capacity overflows"),
                span,
            })?
        }
    };
    let max = max_capacity(fc.type_info.word_bytes);
    if cap > max {
        return Err(CompileError {
            message: format!(
                "`{name}` would produce Text<{cap}>, larger than the largest capacity Text<{max}> \
                 a flat body can hold on this target"
            ),
            span,
        });
    }
    Ok(Some(cap))
}

/// The type of an intrinsic call's result, for the compiler's type inference.
pub(super) fn result_type(fc: &FuncCompiler, name: &str, args: &[Expr]) -> Option<TypeExpr> {
    let op = crate::text_intrinsic::resolve(name).ok()??;
    if args.len() != op.arity() {
        return None;
    }
    let span = Span::default();
    match op {
        TextOp::Len | TextOp::Capacity => Some(TypeExpr::Prim(PrimType::Word, span)),
        TextOp::Eq => Some(TypeExpr::Prim(PrimType::Bool, span)),
        TextOp::Concat | TextOp::Slice => {
            let cap = result_capacity(fc, op, name, args, span).ok()??;
            let lit = i64::try_from(cap).ok()?;
            Some(TypeExpr::TextN(ConstExpr::Lit(lit, span), span))
        }
    }
}

/// Push a non-negative word constant, inline when it fits an immediate.
fn push_word(fc: &mut FuncCompiler, value: usize) {
    if value <= 15 {
        // Immediates 4 through 19 encode `Int(0)` through `Int(15)`.
        fc.emit(Op::PushImmediate(4 + value as u8));
    } else {
        let idx = fc.add_constant(Value::Int(value as i64));
        fc.emit(Op::Const(idx));
    }
}

/// Push the live length of the text held in `slot`.
fn push_length(fc: &mut FuncCompiler, slot: u16) {
    fc.emit(Op::GetLocal(slot));
    fc.emit(Op::GetTupleField(TupleField::Flat {
        offset: 0,
        kind: ScalarKind::Int,
    }));
}

/// Push byte `i` of the text held in `slot`, read at its constant offset.
fn push_byte_at(fc: &mut FuncCompiler, slot: u16, i: usize) {
    let offset = fc.type_info.word_bytes + i;
    fc.emit(Op::GetLocal(slot));
    fc.emit(Op::GetTupleField(TupleField::Flat {
        // Bounded by `max_capacity`, which keeps every body within `u16`.
        offset: offset as u16,
        kind: ScalarKind::Byte,
    }));
}

/// Store the byte array of a `Text<cap>` held in `slot` into a new local.
///
/// The nested read is a zero-copy view of the parent body, so it allocates
/// nothing.
fn stash_bytes(fc: &mut FuncCompiler, slot: u16, cap: usize, local: &str) -> u16 {
    fc.emit(Op::GetLocal(slot));
    fc.emit(Op::GetTupleField(TupleField::FlatNested {
        offset: fc.type_info.word_bytes as u16,
        size: cap as u16,
        variant: CompositeKind::Array,
    }));
    let bytes = fc.declare_local(local);
    fc.emit(Op::SetLocal(bytes));
    bytes
}

/// Read a byte of the array in `bytes` at the word on top of the stack. An
/// index outside the array, negative included, reads as a zero byte. The
/// outcome suffix is discarded, so the read is total and never traps.
fn emit_total_byte_read(fc: &mut FuncCompiler) {
    fc.emit(Op::GetIndex(ArrayElem::Flat {
        kind: ScalarKind::Byte,
    }));
    fc.emit(Op::PopN(6));
}

/// Pack the `cap` bytes and the length beneath them into a `Text<cap>`.
fn emit_pack(fc: &mut FuncCompiler, cap: usize) {
    fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
        kind: CompositeKind::Array,
        count: cap as u16,
        byte_size: cap as u16,
    }));
    fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
        kind: CompositeKind::Tuple,
        count: 2,
        byte_size: (fc.type_info.word_bytes + cap) as u16,
    }));
}

/// Compile the operand `arg` into a fresh local and return its slot.
fn compile_into_local(fc: &mut FuncCompiler, arg: &Expr, local: &str) -> Result<u16, CompileError> {
    compile_expr(fc, arg)?;
    let slot = fc.declare_local(local);
    fc.emit(Op::SetLocal(slot));
    Ok(slot)
}

/// Emit `if <slot> <cmp> <bound> { <slot> = <bound> }`, a one-sided clamp.
fn emit_clamp(fc: &mut FuncCompiler, slot: u16, bound: u16, cmp: Op) {
    fc.emit(Op::GetLocal(slot));
    fc.emit(Op::GetLocal(bound));
    fc.emit(cmp);
    let skip = fc.emit_jump(Op::If(0));
    fc.emit(Op::GetLocal(bound));
    fc.emit(Op::SetLocal(slot));
    fc.patch_jump(skip);
    fc.emit(Op::EndIf);
}

/// Compile a call to a bundled text operation.
pub(super) fn compile(
    fc: &mut FuncCompiler,
    op: TextOp,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<(), CompileError> {
    if args.len() != op.arity() {
        return Err(CompileError {
            message: format!(
                "`{name}` expects {} argument(s), got {}",
                op.arity(),
                args.len()
            ),
            span,
        });
    }
    if fc.type_info.word_bytes == 0 {
        return Err(CompileError {
            message: format!("`{name}` needs a target word width to lay out `Text<N>`"),
            span,
        });
    }
    match op {
        TextOp::Len => {
            operand_capacity(fc, op, name, 0, &args[0])?;
            // Two instructions after the operand. The length is the word at
            // offset zero, read where it lies.
            compile_expr(fc, &args[0])?;
            fc.emit(Op::GetTupleField(TupleField::Flat {
                offset: 0,
                kind: ScalarKind::Int,
            }));
        }
        TextOp::Capacity => {
            let cap = operand_capacity(fc, op, name, 0, &args[0])?;
            // The operand is evaluated for its effects and discarded. The
            // capacity is the literal the type already carries.
            compile_expr(fc, &args[0])?;
            fc.emit(Op::PopN(1));
            push_word(fc, cap);
        }
        TextOp::Concat => compile_concat(fc, op, name, args, span)?,
        TextOp::Eq => compile_eq(fc, op, name, args)?,
        TextOp::Slice => compile_slice(fc, op, name, args, span)?,
    }
    Ok(())
}

/// `text::concat(a: Text<A>, b: Text<B>) -> Text<A + B>`.
///
/// Result byte `i` is `a[i] + b[i - len(a)]` in wrapping byte arithmetic.
/// Under the invariant exactly one term can be nonzero, so the sum selects
/// without a branch. Positions below `len(a)` read `b` at a negative index,
/// which is zero. Positions from `len(a)` up read the zero padding of `a`.
/// Positions at `A` or past it have no `a` term at all. The result length is
/// `len(a) + len(b)`, at most `A + B`, so the result also satisfies the
/// invariant and no truncation is possible.
fn compile_concat(
    fc: &mut FuncCompiler,
    op: TextOp,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<(), CompileError> {
    let a_cap = operand_capacity(fc, op, name, 0, &args[0])?;
    let b_cap = operand_capacity(fc, op, name, 1, &args[1])?;
    let cap = result_capacity(fc, op, name, args, span)?.unwrap_or(a_cap + b_cap);
    let a = compile_into_local(fc, &args[0], "__text_concat_a")?;
    let b = compile_into_local(fc, &args[1], "__text_concat_b")?;
    push_length(fc, a);
    let a_len = fc.declare_local("__text_concat_alen");
    fc.emit(Op::SetLocal(a_len));
    let b_bytes = stash_bytes(fc, b, b_cap, "__text_concat_bbytes");
    // The result length, which `NewComposite` takes beneath the bytes.
    fc.emit(Op::GetLocal(a_len));
    push_length(fc, b);
    fc.emit(Op::CheckedAdd);
    fc.emit(Op::PopN(2));
    for i in 0..cap {
        // The `b` term, b[i - len(a)].
        fc.emit(Op::GetLocal(b_bytes));
        push_word(fc, i);
        fc.emit(Op::GetLocal(a_len));
        fc.emit(Op::CheckedSub);
        fc.emit(Op::PopN(2));
        emit_total_byte_read(fc);
        if i < a_cap {
            push_byte_at(fc, a, i);
            fc.emit(Op::Add);
        }
    }
    emit_pack(fc, cap);
    Ok(())
}

/// `text::eq(a: Text<A>, b: Text<B>) -> Bool`.
///
/// The lengths are combined with `BitXor` and each byte pair below the smaller
/// capacity with a wrapping byte difference widened to a word, all folded into
/// one accumulator with `BitOr`. The texts are equal exactly when the
/// accumulator is zero. Bytes past the smaller capacity need no comparison.
/// When the lengths differ the accumulator is already nonzero, and when they
/// agree the length is at most the smaller capacity, so under the invariant
/// both tails are zero. Two texts of different capacities may be equal.
fn compile_eq(
    fc: &mut FuncCompiler,
    op: TextOp,
    name: &str,
    args: &[Expr],
) -> Result<(), CompileError> {
    let a_cap = operand_capacity(fc, op, name, 0, &args[0])?;
    let b_cap = operand_capacity(fc, op, name, 1, &args[1])?;
    let a = compile_into_local(fc, &args[0], "__text_eq_a")?;
    let b = compile_into_local(fc, &args[1], "__text_eq_b")?;
    push_length(fc, a);
    push_length(fc, b);
    fc.emit(Op::BitXor);
    for i in 0..a_cap.min(b_cap) {
        push_byte_at(fc, a, i);
        push_byte_at(fc, b, i);
        fc.emit(Op::Sub);
        fc.emit(Op::ByteToWord);
        fc.emit(Op::BitOr);
    }
    push_word(fc, 0);
    fc.emit(Op::CmpEq);
    Ok(())
}

/// `text::slice(t: Text<N>, start: Word, end: Word) -> Text<N>`.
///
/// The half-open byte range `[start, end)` is clamped to the live text, so the
/// operation is total. `start` is clamped into `[0, len]` and then `end` into
/// `[start, len]`, so an empty or reversed range yields the empty text rather
/// than a fault. The bounds count bytes, as `N` does, so a range that splits a
/// multi-byte UTF-8 sequence yields the bytes it names.
///
/// Result byte `i` is `t[start + i]` masked to zero when `i` is at or past the
/// result length, which keeps the invariant. The mask is all ones exactly
/// when `i - rlen` is negative, taken from its sign by an arithmetic shift.
/// Neither `i - rlen` nor `start + i` can overflow, because both operands lie
/// in `[0, N]` after clamping.
fn compile_slice(
    fc: &mut FuncCompiler,
    op: TextOp,
    name: &str,
    args: &[Expr],
    span: Span,
) -> Result<(), CompileError> {
    let cap = operand_capacity(fc, op, name, 0, &args[0])?;
    result_capacity(fc, op, name, args, span)?;
    let t = compile_into_local(fc, &args[0], "__text_slice_t")?;
    let start = compile_into_local(fc, &args[1], "__text_slice_start")?;
    let end = compile_into_local(fc, &args[2], "__text_slice_end")?;
    push_length(fc, t);
    let len = fc.declare_local("__text_slice_len");
    fc.emit(Op::SetLocal(len));
    push_word(fc, 0);
    let zero = fc.declare_local("__text_slice_zero");
    fc.emit(Op::SetLocal(zero));
    // start = clamp(start, 0, len), then end = clamp(end, start, len).
    emit_clamp(fc, start, zero, Op::CmpLt);
    emit_clamp(fc, start, len, Op::CmpGt);
    emit_clamp(fc, end, start, Op::CmpLt);
    emit_clamp(fc, end, len, Op::CmpGt);
    fc.emit(Op::GetLocal(end));
    fc.emit(Op::GetLocal(start));
    fc.emit(Op::CheckedSub);
    fc.emit(Op::PopN(2));
    let rlen = fc.declare_local("__text_slice_rlen");
    fc.emit(Op::SetLocal(rlen));
    let bytes = stash_bytes(fc, t, cap, "__text_slice_bytes");
    let sign_shift = fc.add_constant(Value::Int((fc.type_info.word_bytes * 8 - 1) as i64));
    fc.emit(Op::GetLocal(rlen));
    for i in 0..cap {
        // t[start + i], zero past the array.
        fc.emit(Op::GetLocal(bytes));
        fc.emit(Op::GetLocal(start));
        push_word(fc, i);
        fc.emit(Op::CheckedAdd);
        fc.emit(Op::PopN(2));
        emit_total_byte_read(fc);
        fc.emit(Op::ByteToWord);
        // mask = (i - rlen) asr (word_bits - 1), all ones when i < rlen.
        push_word(fc, i);
        fc.emit(Op::GetLocal(rlen));
        fc.emit(Op::CheckedSub);
        fc.emit(Op::PopN(2));
        fc.emit(Op::Const(sign_shift));
        fc.emit(Op::Shr);
        fc.emit(Op::BitAnd);
        fc.emit(Op::WordToByte);
    }
    emit_pack(fc, cap);
    Ok(())
}
