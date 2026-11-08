//! Value-driven flat packing inside a producer-proven static reservation.
use super::types::{ARRAY, ENUM, FLOAT, STRUCT, TUPLE, UNIT};
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::values::{IntValue, PointerValue};
use inkwell::{AddressSpace, IntPredicate};

pub(crate) struct Operand<'ctx> {
    pub bits: IntValue<'ctx>,
    pub tag: IntValue<'ctx>,
    pub body_bytes: IntValue<'ctx>,
}

pub(crate) fn pack<'ctx>(
    b: &Builder<'ctx>,
    base: PointerValue<'ctx>,
    values: &[Operand<'ctx>],
    float_bytes: u32,
    padded: bool,
    capacity: u32,
    trap: BasicBlock<'ctx>,
) -> IntValue<'ctx> {
    let ctx = base.get_type().get_context();
    let i64t = ctx.i64_type();
    let i8t = ctx.i8_type();
    let function = b.get_insert_block().unwrap().get_parent().unwrap();
    let mut cursor = i64t.const_zero();
    let mut fields = Vec::with_capacity(values.len());
    for value in values {
        let body_kind = b
            .build_and(
                value.tag,
                i64t.const_int(u64::from(TUPLE | ARRAY | STRUCT | ENUM), false),
                "packed_body_kind",
            )
            .unwrap();
        let is_body = b
            .build_int_compare(
                IntPredicate::NE,
                body_kind,
                i64t.const_zero(),
                "packed_is_body",
            )
            .unwrap();
        let is_unit = b
            .build_int_compare(
                IntPredicate::EQ,
                value.tag,
                i64t.const_int(u64::from(UNIT), false),
                "packed_unit",
            )
            .unwrap();
        let is_small = b
            .build_int_compare(
                IntPredicate::ULE,
                value.tag,
                i64t.const_int(4, false),
                "packed_small",
            )
            .unwrap();
        let is_float = b
            .build_int_compare(
                IntPredicate::EQ,
                value.tag,
                i64t.const_int(u64::from(FLOAT), false),
                "packed_float",
            )
            .unwrap();
        let scalar = b
            .build_select(
                is_float,
                i64t.const_int(u64::from(float_bytes), false),
                i64t.const_int(8, false),
                "packed_numeric_width",
            )
            .unwrap()
            .into_int_value();
        let scalar = b
            .build_select(
                is_small,
                i64t.const_int(1, false),
                scalar,
                "packed_scalar_width",
            )
            .unwrap()
            .into_int_value();
        let scalar = b
            .build_select(is_unit, i64t.const_zero(), scalar, "packed_unit_width")
            .unwrap()
            .into_int_value();
        let bytes = b
            .build_select(is_body, value.body_bytes, scalar, "packed_width")
            .unwrap()
            .into_int_value();
        fields.push((cursor, bytes, is_body));
        cursor = b.build_int_add(cursor, bytes, "packed_next").unwrap();
    }
    // Check before deriving any field address or performing any store.
    let fits = b
        .build_int_compare(
            IntPredicate::ULE,
            cursor,
            i64t.const_int(u64::from(capacity), false),
            "packed_fits",
        )
        .unwrap();
    let accepted = ctx.append_basic_block(function, "packed_checked");
    b.build_conditional_branch(fits, accepted, trap).unwrap();
    b.position_at_end(accepted);
    for (value, (offset, bytes, is_body)) in values.iter().zip(fields) {
        let writing = ctx.append_basic_block(function, "packed_nonempty");
        let copying = ctx.append_basic_block(function, "packed_copy");
        let scalar = ctx.append_basic_block(function, "packed_scalar");
        let done = ctx.append_basic_block(function, "packed_field_done");
        let empty = b
            .build_int_compare(IntPredicate::EQ, bytes, i64t.const_zero(), "packed_empty")
            .unwrap();
        b.build_conditional_branch(empty, done, writing).unwrap();
        b.position_at_end(writing);
        let destination = unsafe {
            b.build_in_bounds_gep(i8t, base, &[offset], "packed_destination")
                .unwrap()
        };
        b.build_conditional_branch(is_body, copying, scalar)
            .unwrap();
        b.position_at_end(copying);
        let source = b
            .build_int_to_ptr(
                value.bits,
                ctx.ptr_type(AddressSpace::default()),
                "packed_source",
            )
            .unwrap();
        b.build_memcpy(destination, 1, source, 1, bytes).unwrap();
        b.build_unconditional_branch(done).unwrap();
        b.position_at_end(scalar);
        let arms =
            [1u32, 4, 8].map(|width| (width, ctx.append_basic_block(function, "packed_store")));
        let cases: Vec<_> = arms
            .iter()
            .map(|(width, block)| (i64t.const_int(u64::from(*width), false), *block))
            .collect();
        b.build_switch(bytes, trap, &cases).unwrap();
        for (width, block) in arms {
            b.position_at_end(block);
            let bits = match width {
                1 => b
                    .build_int_truncate(value.bits, i8t, "packed_byte")
                    .unwrap(),
                4 => b
                    .build_int_truncate(value.bits, ctx.i32_type(), "packed_float32")
                    .unwrap(),
                _ => value.bits,
            };
            b.build_store(destination, bits)
                .unwrap()
                .set_alignment(1)
                .unwrap();
            b.build_unconditional_branch(done).unwrap();
        }
        b.position_at_end(done);
    }
    if padded {
        let end = i64t.const_int(u64::from(capacity), false);
        let bytes = b.build_int_sub(end, cursor, "packed_padding").unwrap();
        let destination = unsafe {
            b.build_in_bounds_gep(i8t, base, &[cursor], "packed_padding_destination")
                .unwrap()
        };
        b.build_memset(destination, 1, i8t.const_zero(), bytes)
            .unwrap();
        end
    } else {
        cursor
    }
}

/// The slot retains the new view's length. Existing aliases keep their own
/// captured lengths. Copy only the actual bytes, leaving older trailing bytes.
pub(crate) fn store_private<'ctx>(
    b: &Builder<'ctx>,
    destination: PointerValue<'ctx>,
    metadata: PointerValue<'ctx>,
    scalar: PointerValue<'ctx>,
    value: Operand<'ctx>,
    capacity: u32,
    trap: BasicBlock<'ctx>,
) {
    let i64t = value.bits.get_type();
    let ctx = i64t.get_context();
    let function = b.get_insert_block().unwrap().get_parent().unwrap();
    let accepted = ctx.append_basic_block(function, "private_extent_checked");
    let fits = b
        .build_int_compare(
            IntPredicate::ULE,
            value.body_bytes,
            i64t.const_int(u64::from(capacity), false),
            "private_fits",
        )
        .unwrap();
    b.build_conditional_branch(fits, accepted, trap).unwrap();
    b.position_at_end(accepted);
    let copying = ctx.append_basic_block(function, "private_copy");
    let done = ctx.append_basic_block(function, "private_stored");
    let empty = b
        .build_int_compare(
            IntPredicate::EQ,
            value.body_bytes,
            i64t.const_zero(),
            "private_empty",
        )
        .unwrap();
    b.build_conditional_branch(empty, done, copying).unwrap();
    b.position_at_end(copying);
    let source = b
        .build_int_to_ptr(
            value.bits,
            ctx.ptr_type(AddressSpace::default()),
            "private_source",
        )
        .unwrap();
    b.build_memmove(destination, 1, source, 1, value.body_bytes)
        .unwrap();
    b.build_unconditional_branch(done).unwrap();
    b.position_at_end(done);
    // A scalar write leaves the body pool intact for existing body aliases.
    b.build_store(scalar, value.bits).unwrap();
    let length = b
        .build_left_shift(
            value.body_bytes,
            i64t.const_int(32, false),
            "private_length_bits",
        )
        .unwrap();
    let metadata_value = b.build_or(value.tag, length, "private_metadata").unwrap();
    b.build_store(metadata, metadata_value).unwrap();
}
