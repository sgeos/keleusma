//! Persistent scalar kinds in the unused portion of the native private region.
//! Zero means the load-time kind. A write installs the actual nonzero kind.
//! Metadata outlives coroutine frames and is shared by the instance's callees.
use crate::{DataCtx, LowerError, PRIVATE_SLOT_BYTES};
use inkwell::IntPredicate;
use inkwell::builder::Builder;
use inkwell::module::{Linkage, Module};
use inkwell::values::{IntValue, PointerValue};

pub(crate) fn pointer<'ctx>(
    b: &Builder<'ctx>,
    base: PointerValue<'ctx>,
    data: &DataCtx<'_>,
    index: IntValue<'ctx>,
) -> Result<PointerValue<'ctx>, LowerError> {
    let fail = || {
        LowerError::UnsupportedShape("private kind metadata exceeds persistent reservation".into())
    };
    let count = data
        .slot_count
        .checked_sub(data.shared_count)
        .ok_or_else(fail)?;
    if data.private_init.len() != count as usize {
        return Err(fail());
    }
    let words = count.checked_mul(PRIVATE_SLOT_BYTES).ok_or_else(fail)?;
    let offset = words
        .checked_add(data.persistent_composite_bytes)
        .ok_or_else(fail)?;
    if offset
        .checked_add(words)
        .is_none_or(|end| end > data.stream_state_off)
    {
        return Err(fail());
    }
    // The caller has bounded the relative slot index before forming this address.
    let i64t = index.get_type();
    let relative = b
        .build_int_mul(
            index,
            i64t.const_int(u64::from(PRIVATE_SLOT_BYTES), false),
            "private_kind_stride",
        )
        .unwrap();
    let offset = b
        .build_int_add(
            relative,
            i64t.const_int(u64::from(offset), false),
            "private_kind_offset",
        )
        .unwrap();
    Ok(unsafe {
        b.build_in_bounds_gep(
            i64t.get_context().i8_type(),
            base,
            &[offset],
            "private_kind_pointer",
        )
        .unwrap()
    })
}

pub(crate) fn load<'ctx>(
    b: &Builder<'ctx>,
    module: &Module<'ctx>,
    pointer: PointerValue<'ctx>,
    data: &DataCtx<'_>,
    index: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let i64t = index.get_type();
    let name = "kel_private_initial_kinds";
    let table = module.get_global(name).unwrap_or_else(|| {
        let values: Vec<_> = data
            .private_init
            .iter()
            .map(|value| i64t.const_int(u64::from(super::types::initial_kind(value)), false))
            .collect();
        let array = i64t.const_array(&values);
        let global = module.add_global(array.get_type(), None, name);
        global.set_initializer(&array);
        global.set_constant(true);
        global.set_linkage(Linkage::Private);
        global
    });
    let initial_pointer = unsafe {
        b.build_in_bounds_gep(
            i64t,
            table.as_pointer_value(),
            &[index],
            "private_initial_kind_pointer",
        )
        .unwrap()
    };
    let initial = b
        .build_load(i64t, initial_pointer, "private_initial_kind")
        .unwrap()
        .into_int_value();
    let stored = b
        .build_load(i64t, pointer, "private_stored_kind")
        .unwrap()
        .into_int_value();
    // A packed body pool can end at an odd address. The metadata follows it.
    stored.as_instruction().unwrap().set_alignment(1).unwrap();
    let untouched = b
        .build_int_compare(
            IntPredicate::EQ,
            stored,
            i64t.const_zero(),
            "private_kind_unwritten",
        )
        .unwrap();
    b.build_select(untouched, initial, stored, "private_actual_scalar_kind")
        .unwrap()
        .into_int_value()
}
