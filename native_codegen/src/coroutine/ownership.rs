//! Bounded copies for immutable host values, without copying region aliases.
//!
//! Each transfer has a fixed-size allocation. All bytecode callees are inlined,
//! then LLVM captures only storage live across suspension in the bounded frame.
use crate::Width;
use inkwell::builder::Builder;
use inkwell::values::{IntValue, PointerValue};

pub(crate) struct Flags<'ctx> {
    pub locals: Vec<PointerValue<'ctx>>,
    pub slots: Vec<PointerValue<'ctx>>,
}
impl<'ctx> Flags<'ctx> {
    pub fn new(b: &Builder<'ctx>, count: usize) -> Self {
        let i64t = b.get_insert_block().unwrap().get_context().i64_type();
        let locals = (0..count)
            .map(|_| {
                let slot = b.build_alloca(i64t, "owned_local").unwrap();
                b.build_store(slot, i64t.const_zero()).unwrap();
                slot
            })
            .collect();
        Self {
            locals,
            slots: Vec::new(),
        }
    }
}

pub(crate) fn field<'ctx>(
    b: &Builder<'ctx>,
    context: PointerValue<'ctx>,
    index: usize,
) -> PointerValue<'ctx> {
    let i64t = context.get_type().get_context().i64_type();
    // The context has one flag per declared argument and one return flag.
    unsafe {
        b.build_in_bounds_gep(
            i64t,
            context,
            &[i64t.const_int(index as u64, false)],
            "ownership_field",
        )
        .unwrap()
    }
}

pub(crate) fn load<'ctx>(b: &Builder<'ctx>, slot: PointerValue<'ctx>) -> IntValue<'ctx> {
    let i64t = slot.get_type().get_context().i64_type();
    b.build_load(i64t, slot, "ownership_flag")
        .unwrap()
        .into_int_value()
}

pub(crate) fn copy<'ctx>(
    b: &Builder<'ctx>,
    value: IntValue<'ctx>,
    width: Width,
    owned: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let Width::Body(bytes) = width else {
        return value;
    };
    if owned.get_zero_extended_constant() == Some(0) {
        return value;
    }
    let ctx = value.get_type().get_context();
    let destination = b
        .build_alloca(ctx.i8_type().array_type(bytes.max(1)), "owned_body")
        .unwrap();
    copy_to(b, value, bytes, owned, destination)
}

/// Copy owned bodies to storage whose lifetime is supplied by the caller.
/// Borrowed region aliases retain their identity and are never relocated.
pub(crate) fn copy_to<'ctx>(
    b: &Builder<'ctx>,
    value: IntValue<'ctx>,
    bytes: u32,
    owned: IntValue<'ctx>,
    destination: PointerValue<'ctx>,
) -> IntValue<'ctx> {
    if owned.get_zero_extended_constant() == Some(0) {
        return value;
    }
    let i64t = value.get_type();
    let ctx = i64t.get_context();
    let ptrt = ctx.ptr_type(inkwell::AddressSpace::default());
    let source_block = b.get_insert_block().unwrap();
    let function = source_block.get_parent().unwrap();
    let copying = ctx.append_basic_block(function, "copy_host_body");
    let done = ctx.append_basic_block(function, "body_transfer_done");
    let condition = b
        .build_int_compare(
            inkwell::IntPredicate::NE,
            owned,
            i64t.const_zero(),
            "host_owned",
        )
        .unwrap();
    b.build_conditional_branch(condition, copying, done)
        .unwrap();
    b.position_at_end(copying);
    let source = b.build_int_to_ptr(value, ptrt, "host_body_source").unwrap();
    // Exact signature/operand extent, unaligned packed bytes. memmove also
    // defines repeated transfers when LLVM coalesces storage after inlining.
    b.build_memmove(
        destination,
        1,
        source,
        1,
        i64t.const_int(u64::from(bytes), false),
    )
    .unwrap();
    let address = b
        .build_ptr_to_int(destination, i64t, "owned_body_address")
        .unwrap();
    b.build_unconditional_branch(done).unwrap();
    b.position_at_end(done);
    let result = b.build_phi(i64t, "body_value").unwrap();
    result.add_incoming(&[(&value, source_block), (&address, copying)]);
    result.as_basic_value().into_int_value()
}
