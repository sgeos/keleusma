//! Checked arithmetic selects the actual numeric kind after control-flow joins.
//! Integer intermediates fit in i128. Divisors are made nonzero before LLVM
//! evaluates a division. Fixed shifts remain below the 64-bit word width.
use super::types::{BYTE, FIXED, FLOAT, WORD};
use crate::LowerError;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::values::IntValue;
use inkwell::{FloatPredicate, IntPredicate};
use keleusma::bytecode::Op;

pub(crate) fn checked<'ctx>(
    b: &Builder<'ctx>,
    values: &[IntValue<'ctx>],
    tags: &[IntValue<'ctx>],
    float_bytes: u32,
    op: &Op,
    trap: BasicBlock<'ctx>,
) -> Result<(IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>), LowerError> {
    let i64t = values[0].get_type();
    let ctx = i64t.get_context();
    let i128t = ctx.i128_type();
    let zero = i64t.const_zero();
    let select = |condition, yes, no| {
        b.build_select(condition, yes, no, "arith_select")
            .unwrap()
            .into_int_value()
    };
    let is_kind = |kind| {
        b.build_int_compare(
            IntPredicate::EQ,
            tags[0],
            i64t.const_int(u64::from(kind), false),
            "arith_kind",
        )
        .unwrap()
    };
    if tags.len() == 2 {
        let same = b
            .build_int_compare(IntPredicate::EQ, tags[0], tags[1], "arith_same_kind")
            .unwrap();
        let accepted = ctx.append_basic_block(
            b.get_insert_block().unwrap().get_parent().unwrap(),
            "arith_kinds_checked",
        );
        // Analysis and consumer guards establish numeric membership. Agreement
        // is a separate obligation when two joined masks contain several kinds.
        b.build_conditional_branch(same, accepted, trap).unwrap();
        b.position_at_end(accepted);
    }
    let is_fixed = is_kind(FIXED);
    let is_byte = is_kind(BYTE);
    let is_word = is_kind(WORD);
    let a = b.build_int_s_extend(values[0], i128t, "arith_a").unwrap();
    let c = if values.len() == 2 {
        b.build_int_s_extend(values[1], i128t, "arith_b").unwrap()
    } else {
        i128t.const_zero()
    };
    let fraction = match op {
        Op::CheckedMul(bits) | Op::CheckedDiv(bits) => u64::from(*bits),
        _ => 0,
    };
    if fraction >= 64 {
        return Err(LowerError::UnsupportedShape(
            "checked arithmetic fraction count exceeds the word width".into(),
        ));
    }
    let is_zero = b
        .build_int_compare(
            IntPredicate::EQ,
            c,
            i128t.const_zero(),
            "arith_zero_divisor",
        )
        .unwrap();
    let wide = match op {
        Op::CheckedAdd => b.build_int_add(a, c, "arith_add").unwrap(),
        Op::CheckedSub => b.build_int_sub(a, c, "arith_sub").unwrap(),
        Op::CheckedNeg => b.build_int_neg(a, "arith_neg").unwrap(),
        Op::CheckedMul(_) => {
            let product = b.build_int_mul(a, c, "arith_product").unwrap();
            let shifted = b
                .build_right_shift(
                    product,
                    i128t.const_int(fraction, false),
                    true,
                    "arith_fixed_product",
                )
                .unwrap();
            select(is_fixed, shifted, product)
        }
        Op::CheckedDiv(_) | Op::CheckedMod => {
            // The largest shifted dividend has magnitude 2^126. Neither zero
            // division nor signed i128 minimum divided by -1 can reach LLVM.
            let divisor = select(is_zero, i128t.const_int(1, false), c);
            if matches!(op, Op::CheckedMod) {
                b.build_int_signed_rem(a, divisor, "arith_remainder")
                    .unwrap()
            } else {
                let shifted = b
                    .build_left_shift(a, i128t.const_int(fraction, false), "arith_fixed_dividend")
                    .unwrap();
                let dividend = select(is_fixed, shifted, a);
                b.build_int_signed_div(dividend, divisor, "arith_quotient")
                    .unwrap()
            }
        }
        _ => unreachable!("checked arithmetic opcode"),
    };
    let max = b
        .build_int_s_extend(i64t.const_int(i64::MAX as u64, false), i128t, "arith_max")
        .unwrap();
    let min = b
        .build_int_s_extend(i64t.const_int(i64::MIN as u64, true), i128t, "arith_min")
        .unwrap();
    let upper = select(is_byte, i128t.const_int(255, false), max);
    let lower = select(is_byte, i128t.const_zero(), min);
    let over = b
        .build_int_compare(IntPredicate::SGT, wide, upper, "arith_overflow")
        .unwrap();
    let under = b
        .build_int_compare(IntPredicate::SLT, wide, lower, "arith_underflow")
        .unwrap();
    let mut flag = select(
        over,
        i64t.const_int(1, false),
        select(under, i64t.const_int(2, false), zero),
    );
    let wrapped = b.build_int_truncate(wide, i64t, "arith_wrapped").unwrap();
    let byte = b
        .build_and(wrapped, i64t.const_int(255, false), "arith_byte")
        .unwrap();
    let mut low = select(is_byte, byte, wrapped);
    let upper_bits = b
        .build_right_shift(wide, i128t.const_int(64, false), true, "arith_upper_bits")
        .unwrap();
    let upper_word = b
        .build_int_truncate(upper_bits, i64t, "arith_upper_word")
        .unwrap();
    let mut high = select(is_word, upper_word, zero);
    if matches!(op, Op::CheckedDiv(_) | Op::CheckedMod) {
        low = select(is_zero, values[0], low);
        high = select(is_zero, zero, high);
        flag = select(is_zero, i64t.const_int(3, false), flag);
    }
    if matches!(
        op,
        Op::CheckedAdd | Op::CheckedSub | Op::CheckedMul(_) | Op::CheckedDiv(_)
    ) {
        let ft = if float_bytes == 8 {
            ctx.f64_type()
        } else {
            ctx.f32_type()
        };
        let x = crate::bits_to_float(b, values[0], ft, float_bytes);
        let y = crate::bits_to_float(b, values[1], ft, float_bytes);
        let result = match op {
            Op::CheckedAdd => b.build_float_add(x, y, "arith_float_add").unwrap(),
            Op::CheckedSub => b.build_float_sub(x, y, "arith_float_sub").unwrap(),
            Op::CheckedMul(_) => b.build_float_mul(x, y, "arith_float_mul").unwrap(),
            Op::CheckedDiv(_) => b.build_float_div(x, y, "arith_float_div").unwrap(),
            _ => unreachable!(),
        };
        // VM float_checked_flag classifies the rounded result, not operands.
        let nan = b
            .build_float_compare(FloatPredicate::UNO, result, result, "arith_nan")
            .unwrap();
        let positive = b
            .build_float_compare(
                FloatPredicate::OEQ,
                result,
                ft.const_float(f64::INFINITY),
                "arith_positive_infinity",
            )
            .unwrap();
        let negative = b
            .build_float_compare(
                FloatPredicate::OEQ,
                result,
                ft.const_float(f64::NEG_INFINITY),
                "arith_negative_infinity",
            )
            .unwrap();
        let float_flag = select(
            nan,
            i64t.const_int(4, false),
            select(
                positive,
                i64t.const_int(1, false),
                select(negative, i64t.const_int(2, false), zero),
            ),
        );
        let float_bits = crate::float_to_bits(b, result, i64t, float_bytes);
        let is_float = is_kind(FLOAT);
        low = select(is_float, float_bits, low);
        high = select(is_float, zero, high);
        flag = select(is_float, float_flag, flag);
    }
    Ok((low, high, flag))
}

/// Bare operations wrap. Division faults only for an integer zero divisor.
/// Float remainder and negation exist only in the bare opcode family.
pub(crate) fn plain<'ctx>(
    b: &Builder<'ctx>,
    values: &[IntValue<'ctx>],
    tags: &[IntValue<'ctx>],
    float_bytes: u32,
    op: &Op,
    trap: BasicBlock<'ctx>,
) -> Result<IntValue<'ctx>, LowerError> {
    let checked_op = match op {
        Op::Add => Op::CheckedAdd,
        Op::Sub => Op::CheckedSub,
        Op::Mul => Op::CheckedMul(0),
        Op::Div => Op::CheckedDiv(0),
        Op::Mod => Op::CheckedMod,
        Op::Neg => Op::CheckedNeg,
        _ => unreachable!("bare arithmetic opcode"),
    };
    let (low, _, _) = checked(b, values, tags, float_bytes, &checked_op, trap)?;
    let i64t = values[0].get_type();
    let ctx = i64t.get_context();
    let is_float = b
        .build_int_compare(
            IntPredicate::EQ,
            tags[0],
            i64t.const_int(u64::from(FLOAT), false),
            "plain_float",
        )
        .unwrap();
    if matches!(op, Op::Div | Op::Mod) {
        let nonzero = b
            .build_int_compare(
                IntPredicate::NE,
                values[1],
                i64t.const_zero(),
                "plain_nonzero",
            )
            .unwrap();
        let valid = b
            .build_or(is_float, nonzero, "plain_divisor_valid")
            .unwrap();
        let accepted = ctx.append_basic_block(
            b.get_insert_block().unwrap().get_parent().unwrap(),
            "plain_divisor_checked",
        );
        // checked() has already replaced a zero integer divisor before LLVM
        // evaluates it. Float zero division remains IEEE arithmetic.
        b.build_conditional_branch(valid, accepted, trap).unwrap();
        b.position_at_end(accepted);
    }
    if matches!(op, Op::Mod | Op::Neg) {
        let ft = if float_bytes == 8 {
            ctx.f64_type()
        } else {
            ctx.f32_type()
        };
        let x = crate::bits_to_float(b, values[0], ft, float_bytes);
        let result = if matches!(op, Op::Neg) {
            b.build_float_neg(x, "plain_float_neg").unwrap()
        } else {
            let y = crate::bits_to_float(b, values[1], ft, float_bytes);
            b.build_float_rem(x, y, "plain_float_rem").unwrap()
        };
        let bits = crate::float_to_bits(b, result, i64t, float_bytes);
        Ok(b.build_select(is_float, bits, low, "plain_value")
            .unwrap()
            .into_int_value())
    } else {
        Ok(low)
    }
}
