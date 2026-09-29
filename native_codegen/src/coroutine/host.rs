//! Provisional host operations on a stable caller-owned arena slot.
//!
//! The first eight bytes reserve the current continuation pointer, the next
//! eight hold reply bits, and the remaining bytes are the retcon frame. All
//! offsets are constants within `slot_bytes(frame_bytes)`. Start initializes
//! the continuation before either operation may read it. Released slots retain
//! a null continuation until reused by start.

use inkwell::AddressSpace;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, StructType};
use inkwell::values::{FunctionValue, PointerValue, StructValue};

pub(super) const HEADER_BYTES: u32 = 16;

fn offset<'ctx>(
    ctx: &'ctx Context,
    builder: &Builder<'ctx>,
    slot: PointerValue<'ctx>,
    bytes: u32,
) -> PointerValue<'ctx> {
    // The public slot contract reserves the header plus the admitted frame.
    unsafe {
        builder
            .build_in_bounds_gep(
                ctx.i8_type(),
                slot,
                &[ctx.i64_type().const_int(u64::from(bytes), false)],
                "slot_field",
            )
            .unwrap()
    }
}

fn finish_step<'ctx>(
    ctx: &'ctx Context,
    builder: &Builder<'ctx>,
    slot: PointerValue<'ctx>,
    raw: StructValue<'ctx>,
    result: StructType<'ctx>,
) {
    let next = builder
        .build_extract_value(raw, 0, "next")
        .unwrap()
        .into_pointer_value();
    let value = builder
        .build_extract_value(raw, 1, "value")
        .unwrap()
        .into_int_value();
    builder.build_store(slot, next).unwrap();
    let live = builder.build_is_not_null(next, "live").unwrap();
    let status = builder
        .build_int_z_extend(live, ctx.i64_type(), "status")
        .unwrap();
    // An abnormal or completed continuation has no defined payload. Do not
    // expose that field when the status is zero.
    let value = builder
        .build_select(live, value, ctx.i64_type().const_zero(), "payload")
        .unwrap();
    let answer = builder
        .build_insert_value(result.const_zero(), status, 0, "answer")
        .unwrap();
    let answer = builder
        .build_insert_value(answer, value, 1, "answer")
        .unwrap();
    builder.build_return(Some(&answer)).unwrap();
}

pub(super) fn emit<'ctx>(
    ctx: &'ctx Context,
    module: &Module<'ctx>,
    entry: FunctionValue<'ctx>,
    index: usize,
) {
    let ptr = ctx.ptr_type(AddressSpace::default());
    let word = ctx.i64_type();
    let raw_result = entry
        .get_type()
        .get_return_type()
        .unwrap()
        .into_struct_type();
    let continuation = raw_result.fn_type(&[ptr.into(), ctx.bool_type().into()], false);
    let result = ctx.struct_type(&[word.into(), word.into()], false);
    let prefix = format!("kel_coroutine_{index}");
    // Replace the raw frame and reply parameters with the one stable slot.
    let mut parameters: Vec<BasicMetadataTypeEnum<'ctx>> = entry
        .get_type()
        .get_param_types()
        .into_iter()
        .take(4)
        .collect();
    parameters.push(ptr.into());
    let start = module.add_function(
        &format!("{prefix}_start"),
        result.fn_type(&parameters, false),
        None,
    );
    let builder = ctx.create_builder();
    builder.position_at_end(ctx.append_basic_block(start, "start"));
    let slot = start.get_last_param().unwrap().into_pointer_value();
    let mut args: Vec<_> = start.get_param_iter().take(4).map(Into::into).collect();
    args.push(offset(ctx, &builder, slot, HEADER_BYTES).into());
    args.push(offset(ctx, &builder, slot, 8).into());
    let raw = builder
        .build_call(entry, &args, "first")
        .unwrap()
        .try_as_basic_value()
        .unwrap_basic()
        .into_struct_value();
    finish_step(ctx, &builder, slot, raw, result);

    for release in [false, true] {
        let name = format!("{prefix}_{}", if release { "release" } else { "resume" });
        let ty = if release {
            ctx.void_type().fn_type(&[ptr.into()], false)
        } else {
            result.fn_type(&[ptr.into(), word.into()], false)
        };
        let function = module.add_function(&name, ty, None);
        let begin = ctx.append_basic_block(function, "begin");
        let active = ctx.append_basic_block(function, "active");
        let inactive = ctx.append_basic_block(function, "inactive");
        builder.position_at_end(begin);
        let slot = function.get_first_param().unwrap().into_pointer_value();
        let next = builder
            .build_load(ptr, slot, "current")
            .unwrap()
            .into_pointer_value();
        let live = builder.build_is_not_null(next, "live").unwrap();
        builder
            .build_conditional_branch(live, active, inactive)
            .unwrap();
        builder.position_at_end(inactive);
        if release {
            builder.build_return(None).unwrap();
        } else {
            builder.build_return(Some(&result.const_zero())).unwrap();
        }
        builder.position_at_end(active);
        if !release {
            let reply = offset(ctx, &builder, slot, 8);
            builder
                .build_store(reply, function.get_nth_param(1).unwrap())
                .unwrap();
        }
        let frame = offset(ctx, &builder, slot, HEADER_BYTES);
        let raw = builder
            .build_indirect_call(
                continuation,
                next,
                &[
                    frame.into(),
                    ctx.bool_type().const_int(u64::from(release), false).into(),
                ],
                "step",
            )
            .unwrap()
            .try_as_basic_value()
            .unwrap_basic()
            .into_struct_value();
        if release {
            builder.build_store(slot, ptr.const_null()).unwrap();
            builder.build_return(None).unwrap();
        } else {
            finish_step(ctx, &builder, slot, raw, result);
        }
    }
}
