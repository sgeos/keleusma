//! Lowering of the bundled text operations over `Text<N>` to existing opcodes.
//!
//! The operations are methods on the value, `t.len()`, `t.capacity()`,
//! `a.concat(b)`, `a.eq(b)` and `t.slice(start, end)`. Operand zero is the
//! receiver and the method arguments follow it.
//!
//! # The representation every lowering reads
//!
//! A `Text<N>` is the flat body `Tuple(Int length, Array[Byte; N])`, so its
//! length is the scalar word at offset zero and its byte `i` is the scalar byte
//! at offset `word_bytes + i`. Both are read with a constant-offset
//! `GetTupleField`, which costs one instruction, allocates nothing, and is
//! bounds-checked by the virtual machine. A byte at a position computed at run
//! time is read from the nested array with the total `GetIndex` form, whose
//! out-of-range result is a zero byte and an outcome flag that is discarded.
//!
//! # A defined result for every operand body
//!
//! **No lowering assumes its operands are well formed.** A `Text<N>` that
//! script code builds has a length in `[0, N]` and zero bytes past it, but a
//! host can pass any body, as an entry argument for example. Every operation
//! therefore reads its operand through two rules.
//!
//! - The effective length is `min(max(len, 0), N)`, computed by two one-sided
//!   clamps against constants.
//! - Every byte at or past the effective length reads as zero, by a mask that
//!   is all ones exactly when the position is below the effective length.
//!
//! The result of every operation is then specified for every input body, and
//! every result satisfies the invariant, length in `[0, N]` and zero padding,
//! whatever the operands were. The rules cost a fixed number of instructions
//! per operand and per byte, so each cost stays static. The cost of each is
//! stated at its lowering and recorded in `docs/decisions/TEXT_CAPACITY_TYPE.md`.
//!
//! # Why the code grows with `N`
//!
//! No opcode writes one byte of a flat body, so each result is built whole,
//! one pushed value per byte followed by one `NewComposite` for the byte array
//! and one for the enclosing tuple. The instruction count is therefore linear
//! in the capacity, and so is the static worst-case execution time.

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

/// How a diagnostic names operand `index` of an operation.
fn operand_label(index: usize) -> String {
    if index == 0 {
        String::from("the receiver")
    } else {
        format!("argument {}", index - 1)
    }
}

/// The capacity of a `Text<N>` operand as the compiler infers it.
fn operand_capacity(
    fc: &FuncCompiler,
    op: TextOp,
    index: usize,
    operand: &Expr,
) -> Result<usize, CompileError> {
    let refuse = |detail: String| CompileError {
        message: format!(
            "method `{}` on `Text<N>`, {} {detail}",
            op.name(),
            operand_label(index)
        ),
        span: operand.span(),
    };
    let ty = infer_expr_type(fc, operand).map(|t| strip_type_labels(&t));
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

/// The capacity of the value an operation produces, when it produces a
/// `Text<N>`, without emitting anything. `operands` holds the receiver first.
fn result_capacity(
    fc: &FuncCompiler,
    op: TextOp,
    operands: &[&Expr],
    span: Span,
) -> Result<Option<usize>, CompileError> {
    let cap = match op {
        TextOp::Len | TextOp::Capacity | TextOp::Eq => return Ok(None),
        TextOp::Slice => operand_capacity(fc, op, 0, operands[0])?,
        TextOp::Concat => {
            let a = operand_capacity(fc, op, 0, operands[0])?;
            let b = operand_capacity(fc, op, 1, operands[1])?;
            a.checked_add(b).ok_or_else(|| CompileError {
                message: String::from("method `concat` result capacity overflows"),
                span,
            })?
        }
    };
    let max = max_capacity(fc.type_info.word_bytes);
    if cap > max {
        return Err(CompileError {
            message: format!(
                "method `{}` would produce Text<{cap}>, larger than the largest capacity \
                 Text<{max}> a flat body can hold on this target",
                op.name()
            ),
            span,
        });
    }
    Ok(Some(cap))
}

/// The receiver followed by the arguments, when the receiver is a `Text<N>`
/// and the method is an operation of the right arity.
fn operation<'e>(
    fc: &FuncCompiler,
    receiver: &'e Expr,
    method: &str,
    args: &'e [Expr],
) -> Option<(TextOp, Vec<&'e Expr>)> {
    let receiver_ty = infer_expr_type(fc, receiver).map(|t| strip_type_labels(&t));
    if !matches!(receiver_ty, Some(TypeExpr::TextN(..))) {
        return None;
    }
    let op = crate::text_intrinsic::resolve_method(method).ok()??;
    if args.len() != op.arity() {
        return None;
    }
    let mut operands = Vec::with_capacity(args.len() + 1);
    operands.push(receiver);
    operands.extend(args.iter());
    Some((op, operands))
}

/// The type of an operation's result, for the compiler's type inference.
/// `None` when the method call is not an operation.
pub(super) fn result_type(
    fc: &FuncCompiler,
    receiver: &Expr,
    method: &str,
    args: &[Expr],
) -> Option<TypeExpr> {
    let (op, operands) = operation(fc, receiver, method, args)?;
    let span = Span::default();
    match op {
        TextOp::Len | TextOp::Capacity => Some(TypeExpr::Prim(PrimType::Word, span)),
        TextOp::Eq => Some(TypeExpr::Prim(PrimType::Bool, span)),
        TextOp::Concat | TextOp::Slice => {
            let cap = result_capacity(fc, op, &operands, span).ok()??;
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

/// Push the raw length word of the text held in `slot`, as stored.
fn push_raw_length(fc: &mut FuncCompiler, slot: u16) {
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
/// nothing. Three instructions.
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

/// Emit `if <slot> <cmp> <bound> { <slot> = <bound> }`, a one-sided clamp
/// against a local. Seven instructions.
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

/// Emit `if <slot> <cmp> <value> { <slot> = <value> }`, a one-sided clamp
/// against a constant. Seven instructions.
fn emit_clamp_const(fc: &mut FuncCompiler, slot: u16, value: usize, cmp: Op) {
    fc.emit(Op::GetLocal(slot));
    push_word(fc, value);
    fc.emit(cmp);
    let skip = fc.emit_jump(Op::If(0));
    push_word(fc, value);
    fc.emit(Op::SetLocal(slot));
    fc.patch_jump(skip);
    fc.emit(Op::EndIf);
}

/// Clamp the length word on top of the stack into `[0, cap]` and store it in
/// a new local, the effective length. Fifteen instructions.
fn store_effective_length(fc: &mut FuncCompiler, cap: usize, local: &str) -> u16 {
    let len = fc.declare_local(local);
    fc.emit(Op::SetLocal(len));
    emit_clamp_const(fc, len, 0, Op::CmpLt);
    emit_clamp_const(fc, len, cap, Op::CmpGt);
    len
}

/// The effective length of the `Text<cap>` held in `slot`, in a new local.
/// Seventeen instructions.
fn effective_length(fc: &mut FuncCompiler, slot: u16, cap: usize, local: &str) -> u16 {
    push_raw_length(fc, slot);
    store_effective_length(fc, cap, local)
}

/// Push a word that is all ones when `i` is below the word in local `bound`
/// and zero otherwise, the sign of `i - bound` spread by an arithmetic shift.
///
/// Both `i` and the bound lie in `[0, A + B]` wherever this is used, so the
/// difference cannot overflow. Six instructions.
fn push_mask_below(fc: &mut FuncCompiler, i: usize, bound: u16, sign_shift: u16) {
    push_word(fc, i);
    fc.emit(Op::GetLocal(bound));
    fc.emit(Op::CheckedSub);
    fc.emit(Op::PopN(2));
    fc.emit(Op::Const(sign_shift));
    fc.emit(Op::Shr);
}

/// The constant-pool index of the shift that spreads a word's sign bit.
fn sign_shift_constant(fc: &mut FuncCompiler) -> u16 {
    fc.add_constant(Value::Int((fc.type_info.word_bytes * 8 - 1) as i64))
}

/// Compile a method call to a bundled text operation.
pub(super) fn compile(
    fc: &mut FuncCompiler,
    op: TextOp,
    receiver: &Expr,
    args: &[Expr],
    span: Span,
) -> Result<(), CompileError> {
    if args.len() != op.arity() {
        return Err(CompileError {
            message: format!(
                "method `{}` on `Text<N>` expects {} argument(s), got {}",
                op.name(),
                op.arity(),
                args.len()
            ),
            span,
        });
    }
    if fc.type_info.word_bytes == 0 {
        return Err(CompileError {
            message: format!(
                "method `{}` needs a target word width to lay out `Text<N>`",
                op.name()
            ),
            span,
        });
    }
    let mut operands: Vec<&Expr> = Vec::with_capacity(args.len() + 1);
    operands.push(receiver);
    operands.extend(args.iter());
    match op {
        TextOp::Len => {
            let cap = operand_capacity(fc, op, 0, receiver)?;
            // Seventeen instructions after the operand. The length word is
            // read where it lies and clamped into `[0, N]`.
            compile_expr(fc, receiver)?;
            fc.emit(Op::GetTupleField(TupleField::Flat {
                offset: 0,
                kind: ScalarKind::Int,
            }));
            let len = store_effective_length(fc, cap, "__text_len");
            fc.emit(Op::GetLocal(len));
        }
        TextOp::Capacity => {
            let cap = operand_capacity(fc, op, 0, receiver)?;
            // The operand is evaluated for its effects and discarded. The
            // capacity is the literal the type already carries, so no body is
            // read and no malformed body can change it.
            compile_expr(fc, receiver)?;
            fc.emit(Op::PopN(1));
            push_word(fc, cap);
        }
        TextOp::Concat => compile_concat(fc, &operands, span)?,
        TextOp::Eq => compile_eq(fc, &operands)?,
        TextOp::Slice => compile_slice(fc, &operands, span)?,
    }
    Ok(())
}

/// `a.concat(b: Text<B>) -> Text<A + B>`, with `a: Text<A>`.
///
/// With `la` and `lb` the effective lengths and `r = la + lb`, result byte `i`
/// is `(a[i] & below(i, la)) | b[i - la]`, masked by `below(i, r)`. The `a`
/// term is the byte of `a` inside its effective length. The `b` term reads
/// `b` at `i - la`, which is zero for a negative index and past the array, and
/// the mask by `r` zeroes it at and past `lb`. At most one term is nonzero, so
/// the `BitOr` selects without a branch. A word `Add` is not used because the
/// virtual machine's `Add` does not take two words. The result length is
/// `r`, at most `A + B`, and every byte past it is zero, so the result
/// satisfies the invariant for any operand bodies and no truncation is
/// possible.
fn compile_concat(
    fc: &mut FuncCompiler,
    operands: &[&Expr],
    span: Span,
) -> Result<(), CompileError> {
    let op = TextOp::Concat;
    let a_cap = operand_capacity(fc, op, 0, operands[0])?;
    let b_cap = operand_capacity(fc, op, 1, operands[1])?;
    let cap = result_capacity(fc, op, operands, span)?.unwrap_or(a_cap + b_cap);
    let a = compile_into_local(fc, operands[0], "__text_concat_a")?;
    let b = compile_into_local(fc, operands[1], "__text_concat_b")?;
    let a_len = effective_length(fc, a, a_cap, "__text_concat_alen");
    let b_len = effective_length(fc, b, b_cap, "__text_concat_blen");
    let b_bytes = stash_bytes(fc, b, b_cap, "__text_concat_bbytes");
    let sign_shift = sign_shift_constant(fc);
    // The result length, which `NewComposite` takes beneath the bytes, kept
    // in a local for the masks.
    fc.emit(Op::GetLocal(a_len));
    fc.emit(Op::GetLocal(b_len));
    fc.emit(Op::CheckedAdd);
    fc.emit(Op::PopN(2));
    let r_len = fc.declare_local("__text_concat_rlen");
    fc.emit(Op::Dup);
    fc.emit(Op::SetLocal(r_len));
    for i in 0..cap {
        // The `b` term, b[i - la], zero outside the array.
        fc.emit(Op::GetLocal(b_bytes));
        push_word(fc, i);
        fc.emit(Op::GetLocal(a_len));
        fc.emit(Op::CheckedSub);
        fc.emit(Op::PopN(2));
        emit_total_byte_read(fc);
        fc.emit(Op::ByteToWord);
        if i < a_cap {
            // The `a` term, a[i] masked to zero at and past `la`.
            push_byte_at(fc, a, i);
            fc.emit(Op::ByteToWord);
            push_mask_below(fc, i, a_len, sign_shift);
            fc.emit(Op::BitAnd);
            fc.emit(Op::BitOr);
        }
        push_mask_below(fc, i, r_len, sign_shift);
        fc.emit(Op::BitAnd);
        fc.emit(Op::WordToByte);
    }
    emit_pack(fc, cap);
    Ok(())
}

/// `a.eq(b: Text<B>) -> Bool`, with `a: Text<A>`.
///
/// The effective lengths are combined with `BitXor`, and each byte pair below
/// the smaller capacity contributes its wrapping byte difference widened to a
/// word and masked by `below(i, la)`, all folded into one accumulator with
/// `BitOr`. The texts are equal exactly when the accumulator is zero. When the
/// effective lengths differ the accumulator is already nonzero. When they
/// agree at `l`, `l` is at most the smaller capacity, so the positions below
/// `l` are exactly the masked-in ones and every position at or past `l`, in
/// either operand, is content-free. Bytes past the smaller capacity therefore
/// need no comparison, whatever the operand bodies hold there. Two texts of
/// different capacities may be equal.
fn compile_eq(fc: &mut FuncCompiler, operands: &[&Expr]) -> Result<(), CompileError> {
    let op = TextOp::Eq;
    let a_cap = operand_capacity(fc, op, 0, operands[0])?;
    let b_cap = operand_capacity(fc, op, 1, operands[1])?;
    let a = compile_into_local(fc, operands[0], "__text_eq_a")?;
    let b = compile_into_local(fc, operands[1], "__text_eq_b")?;
    let a_len = effective_length(fc, a, a_cap, "__text_eq_alen");
    let b_len = effective_length(fc, b, b_cap, "__text_eq_blen");
    let sign_shift = sign_shift_constant(fc);
    fc.emit(Op::GetLocal(a_len));
    fc.emit(Op::GetLocal(b_len));
    fc.emit(Op::BitXor);
    for i in 0..a_cap.min(b_cap) {
        push_byte_at(fc, a, i);
        push_byte_at(fc, b, i);
        fc.emit(Op::Sub);
        fc.emit(Op::ByteToWord);
        push_mask_below(fc, i, a_len, sign_shift);
        fc.emit(Op::BitAnd);
        fc.emit(Op::BitOr);
    }
    push_word(fc, 0);
    fc.emit(Op::CmpEq);
    Ok(())
}

/// `t.slice(start: Word, end: Word) -> Text<N>`, with `t: Text<N>`.
///
/// The half-open byte range `[start, end)` is clamped to the effective length
/// `l`, so the operation is total. `start` is clamped into `[0, l]` and then
/// `end` into `[start, l]`, so an empty or reversed range yields the empty
/// text rather than a fault. The bounds count bytes, as `N` does, so a range
/// that splits a multi-byte UTF-8 sequence yields the bytes it names.
///
/// Result byte `i` is `t[start + i]` masked to zero when `i` is at or past the
/// result length `rlen`, which keeps the invariant. Every unmasked read lies
/// below `end`, which is at most `l`, so no byte at or past the effective
/// length reaches the result whatever the operand body holds there. Neither
/// `i - rlen` nor `start + i` can overflow, because both operands lie in
/// `[0, N]` after clamping.
fn compile_slice(
    fc: &mut FuncCompiler,
    operands: &[&Expr],
    span: Span,
) -> Result<(), CompileError> {
    let op = TextOp::Slice;
    let cap = operand_capacity(fc, op, 0, operands[0])?;
    result_capacity(fc, op, operands, span)?;
    let t = compile_into_local(fc, operands[0], "__text_slice_t")?;
    let start = compile_into_local(fc, operands[1], "__text_slice_start")?;
    let end = compile_into_local(fc, operands[2], "__text_slice_end")?;
    let len = effective_length(fc, t, cap, "__text_slice_len");
    push_word(fc, 0);
    let zero = fc.declare_local("__text_slice_zero");
    fc.emit(Op::SetLocal(zero));
    // start = clamp(start, 0, l), then end = clamp(end, start, l).
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
    let sign_shift = sign_shift_constant(fc);
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
        push_mask_below(fc, i, rlen, sign_shift);
        fc.emit(Op::BitAnd);
        fc.emit(Op::WordToByte);
    }
    emit_pack(fc, cap);
    Ok(())
}
