//! Differential checks on independently assembled modules.
//!
//! Exhaustive destructuring makes newly added metadata fields require an explicit decision.
use crate::bytecode::{Chunk, DataLayout, Module};
use alloc::format;
use alloc::string::String;

pub(super) fn difference(actual: &Module, expected: &Module) -> Option<String> {
    if actual.chunks.len() != expected.chunks.len()
        || actual.chunks.iter().zip(&expected.chunks).any(|(a, b)| {
            a.name != b.name
                || a.ops != b.ops
                || a.constants != b.constants
                || a.local_count != b.local_count
        })
    {
        return Some(super::describe_divergence(actual, expected));
    }
    let Module {
        chunks,
        native_names,
        entry_point,
        data_layout,
        word_bits_log2,
        addr_bits_log2,
        float_bits_log2,
        wcet_cycles,
        wcmu_bytes,
        aux_arena_bytes,
        persistent_composite_bytes,
        flags,
        shared_data_bytes,
        private_data_bytes,
        schema_hash,
        enum_layouts,
        signatures,
        native_return_shapes,
    } = expected;
    for (field, differs) in [
        ("native names", actual.native_names != *native_names),
        ("entry point", actual.entry_point != *entry_point),
        ("word width", actual.word_bits_log2 != *word_bits_log2),
        ("address width", actual.addr_bits_log2 != *addr_bits_log2),
        ("float width", actual.float_bits_log2 != *float_bits_log2),
        ("execution bound", actual.wcet_cycles != *wcet_cycles),
        ("memory bound", actual.wcmu_bytes != *wcmu_bytes),
        (
            "auxiliary arena bytes",
            actual.aux_arena_bytes != *aux_arena_bytes,
        ),
        (
            "persistent composite bytes",
            actual.persistent_composite_bytes != *persistent_composite_bytes,
        ),
        ("flags", actual.flags != *flags),
        (
            "shared data bytes",
            actual.shared_data_bytes != *shared_data_bytes,
        ),
        (
            "private data bytes",
            actual.private_data_bytes != *private_data_bytes,
        ),
        ("schema hash", actual.schema_hash != *schema_hash),
        ("enum layouts", actual.enum_layouts != *enum_layouts),
        ("signatures", actual.signatures != *signatures),
        (
            "native return shapes",
            actual.native_return_shapes != *native_return_shapes,
        ),
    ] {
        if differs {
            return Some(format!("module {field} diverge from reference"));
        }
    }
    match (&actual.data_layout, data_layout) {
        (None, None) => {}
        (Some(a), Some(b)) => {
            if let Some(field) = data_difference(a, b) {
                return Some(format!("module data {field} diverge from reference"));
            }
        }
        _ => return Some("module data layout presence diverges from reference".into()),
    }
    for (a, b) in actual.chunks.iter().zip(chunks) {
        if let Some(field) = chunk_difference(a, b) {
            return Some(format!("chunk `{}` {field} diverge from reference", a.name));
        }
    }
    None
}

fn data_difference(actual: &DataLayout, expected: &DataLayout) -> Option<&'static str> {
    let DataLayout {
        slots,
        shared_layout,
        private_composite_layout,
        private_init,
    } = expected;
    [
        ("slots", actual.slots != *slots),
        ("shared layout", actual.shared_layout != *shared_layout),
        (
            "private composite layout",
            actual.private_composite_layout != *private_composite_layout,
        ),
        ("private initializers", actual.private_init != *private_init),
    ]
    .into_iter()
    .find_map(|(field, differs)| differs.then_some(field))
}

fn chunk_difference(actual: &Chunk, expected: &Chunk) -> Option<&'static str> {
    let Chunk {
        name: _,
        ops: _,
        constants: _,
        local_count: _,
        struct_templates,
        param_count,
        block_type,
        param_types,
        debug_pool,
    } = expected;
    [
        (
            "struct templates",
            actual.struct_templates != *struct_templates,
        ),
        ("parameter count", actual.param_count != *param_count),
        ("block type", actual.block_type != *block_type),
        ("parameter types", actual.param_types != *param_types),
        ("debug pool", actual.debug_pool != *debug_pool),
    ]
    .into_iter()
    .find_map(|(field, differs)| differs.then_some(field))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::{
        BlockType, ConstValue, EnumLayout, PrivateCompositeSlot, StructTemplate, TypeTag, WireShape,
    };
    use alloc::vec;

    #[test]
    fn every_module_and_chunk_field_is_observable() {
        let reference = super::super::compile_src(
            "shared data s { n: Word } private data p { n: Word } fn main() -> Word { p.n = 1; 7 }",
        );
        assert!(difference(&reference, &reference.clone()).is_none());
        type Change = fn(&mut Module);
        let changes: &[(&str, Change)] = &[
            ("chunk count", |m| {
                m.chunks.pop();
            }),
            ("chunk name", |m| m.chunks[0].name.push('x')),
            ("operations", |m| {
                m.chunks[0].ops.pop();
            }),
            ("constants", |m| {
                m.chunks[0].constants.push(ConstValue::Int(9))
            }),
            ("local count", |m| m.chunks[0].local_count += 1),
            ("templates", |m| {
                m.chunks[0].struct_templates.push(StructTemplate {
                    type_name: "P".into(),
                    field_names: vec![],
                })
            }),
            ("parameter count", |m| m.chunks[0].param_count += 1),
            ("block type", |m| m.chunks[0].block_type = BlockType::Stream),
            ("parameter types", |m| {
                m.chunks[0].param_types.push(TypeTag::Word)
            }),
            ("debug", |m| {
                m.chunks[0].debug_pool = Some(Default::default())
            }),
            ("native names", |m| m.native_names.push("native".into())),
            ("entry", |m| m.entry_point = None),
            ("data presence", |m| m.data_layout = None),
            ("data slots", |m| {
                m.data_layout.as_mut().unwrap().slots[0].name.push('x')
            }),
            ("shared layout", |m| {
                m.data_layout.as_mut().unwrap().shared_layout[0].offset += 1
            }),
            ("private layout", |m| {
                m.data_layout
                    .as_mut()
                    .unwrap()
                    .private_composite_layout
                    .push(PrivateCompositeSlot { slot: 1, offset: 0 })
            }),
            ("private initialization", |m| {
                m.data_layout.as_mut().unwrap().private_init[0] = ConstValue::Int(1)
            }),
            ("word width", |m| m.word_bits_log2 ^= 1),
            ("address width", |m| m.addr_bits_log2 ^= 1),
            ("float width", |m| m.float_bits_log2 ^= 1),
            ("execution bound", |m| m.wcet_cycles ^= 1),
            ("memory bound", |m| m.wcmu_bytes ^= 1),
            ("auxiliary bytes", |m| m.aux_arena_bytes ^= 1),
            ("persistent bytes", |m| m.persistent_composite_bytes ^= 1),
            ("flags", |m| m.flags ^= 1),
            ("shared bytes", |m| m.shared_data_bytes ^= 1),
            ("private bytes", |m| m.private_data_bytes ^= 1),
            ("schema", |m| m.schema_hash ^= 1),
            ("enums", |m| {
                m.enum_layouts.push(EnumLayout {
                    type_name: "E".into(),
                    variants: vec![],
                    min_payload: 0,
                })
            }),
            ("signatures", |m| m.signatures[0].ret = WireShape::Top),
            ("native returns", |m| {
                m.native_return_shapes.push(WireShape::Top)
            }),
        ];
        for (name, change) in changes {
            let mut actual = reference.clone();
            change(&mut actual);
            assert!(
                difference(&actual, &reference).is_some(),
                "undetected {name}"
            );
        }
    }
}
