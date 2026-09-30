//! Runtime kinds and body sizes for values merged by control flow.
//! Static facts seed constants. Only local and stack copies transport unknown
//! metadata. LLVM promotes the slots and captures values live across suspension.
use super::types::{ENUM, Facts, kind};
use inkwell::IntPredicate;
use inkwell::builder::Builder;
use inkwell::values::{IntValue, PointerValue};
use keleusma::bytecode::Op;

pub(crate) struct Flags<'ctx> {
    pub locals: Vec<PointerValue<'ctx>>,
    pub slots: Vec<PointerValue<'ctx>>,
    pub local_sizes: Vec<PointerValue<'ctx>>,
    pub slot_sizes: Vec<PointerValue<'ctx>>,
}
impl<'ctx> Flags<'ctx> {
    pub fn new(b: &Builder<'ctx>, count: usize) -> Self {
        let i64t = b.get_insert_block().unwrap().get_context().i64_type();
        let locals = (0..count)
            .map(|_| {
                let slot = b.build_alloca(i64t, "kind_local").unwrap();
                b.build_store(slot, i64t.const_zero()).unwrap();
                slot
            })
            .collect();
        let local_sizes = (0..count)
            .map(|_| {
                let slot = b.build_alloca(i64t, "body_size_local").unwrap();
                b.build_store(slot, i64t.const_zero()).unwrap();
                slot
            })
            .collect();
        Self {
            local_sizes,
            slot_sizes: Vec::new(),
            locals,
            slots: Vec::new(),
        }
    }
    pub fn reset(&self, b: &Builder<'ctx>) {
        let i64t = b.get_insert_block().unwrap().get_context().i64_type();
        for local in &self.locals {
            b.build_store(*local, i64t.const_int(1, false)).unwrap();
        }
        for size in &self.local_sizes {
            b.build_store(*size, i64t.const_zero()).unwrap();
        }
    }
    pub fn load(&self, b: &Builder<'ctx>, index: usize) -> IntValue<'ctx> {
        super::ownership::load(b, self.slots[index])
    }
    pub fn seed(&self, b: &Builder<'ctx>, facts: &Facts, op: &Op) -> Vec<IntValue<'ctx>> {
        let i64t = b.get_insert_block().unwrap().get_context().i64_type();
        for (slot, mask) in self
            .slots
            .iter()
            .zip(&facts.stack)
            .chain(self.locals.iter().zip(&facts.locals))
        {
            let tag = kind(*mask);
            if tag.count_ones() == 1 {
                b.build_store(*slot, i64t.const_int(u64::from(tag), false))
                    .unwrap();
            }
        }
        for (slot, width) in self
            .slot_sizes
            .iter()
            .zip(&facts.extents.stack)
            .chain(self.local_sizes.iter().zip(&facts.extents.locals))
        {
            seed_size(b, *slot, *width);
        }
        let input_sizes = self
            .slot_sizes
            .iter()
            .take(facts.stack.len())
            .map(|slot| super::ownership::load(b, *slot))
            .collect();
        let depth = facts.stack.len();
        let transfer = match op {
            Op::GetLocal(index) => Some((
                self.locals[*index as usize],
                self.slots[depth],
                self.local_sizes[*index as usize],
                self.slot_sizes[depth],
            )),
            Op::SetLocal(index) => Some((
                self.slots[depth - 1],
                self.locals[*index as usize],
                self.slot_sizes[depth - 1],
                self.local_sizes[*index as usize],
            )),
            Op::Dup => Some((
                self.slots[depth - 1],
                self.slots[depth],
                self.slot_sizes[depth - 1],
                self.slot_sizes[depth],
            )),
            _ => None,
        };
        if let Some((source, destination, source_size, destination_size)) = transfer {
            b.build_store(destination_size, super::ownership::load(b, source_size))
                .unwrap();
            b.build_store(destination, super::ownership::load(b, source))
                .unwrap();
        }
        // Record producer tags before a successor merges them. A fallthrough
        // producer can enter a mixed join directly, with no single-kind input
        // fact at the following instruction. Copies above carry dynamic tags.
        for output in &facts.outputs {
            seed_size(b, self.slot_sizes[output.slot], output.width);
            let tag = kind(output.tag);
            if tag.count_ones() == 1 {
                b.build_store(
                    self.slots[output.slot],
                    i64t.const_int(u64::from(tag), false),
                )
                .unwrap();
            }
        }
        input_sizes
    }
}

/// Only the enum edge may dereference the body. A scalar's bits are never an address.
pub(crate) fn enum_test<'ctx>(
    b: &Builder<'ctx>,
    value: IntValue<'ctx>,
    tag: IntValue<'ctx>,
    expected: i64,
) -> IntValue<'ctx> {
    let i64t = value.get_type();
    let ctx = i64t.get_context();
    let before = b.get_insert_block().unwrap();
    let function = before.get_parent().unwrap();
    let reading = ctx.append_basic_block(function, "enum_body");
    let done = ctx.append_basic_block(function, "enum_test_done");
    let is_enum = b
        .build_int_compare(
            IntPredicate::EQ,
            tag,
            i64t.const_int(u64::from(ENUM), false),
            "enum_kind",
        )
        .unwrap();
    b.build_conditional_branch(is_enum, reading, done).unwrap();
    b.position_at_end(reading);
    let pointer = b
        .build_int_to_ptr(
            value,
            ctx.ptr_type(inkwell::AddressSpace::default()),
            "enum_pointer",
        )
        .unwrap();
    let disc = b
        .build_load(i64t, pointer, "enum_discriminant")
        .unwrap()
        .into_int_value();
    disc.as_instruction().unwrap().set_alignment(1).unwrap();
    let equal = b
        .build_int_compare(
            IntPredicate::EQ,
            disc,
            i64t.const_int(expected as u64, true),
            "enum_variant",
        )
        .unwrap();
    b.build_unconditional_branch(done).unwrap();
    b.position_at_end(done);
    let result = b.build_phi(ctx.bool_type(), "enum_matches").unwrap();
    result.add_incoming(&[(&ctx.bool_type().const_zero(), before), (&equal, reading)]);
    b.build_int_z_extend(
        result.as_basic_value().into_int_value(),
        i64t,
        "enum_result",
    )
    .unwrap()
}

fn seed_size(b: &Builder<'_>, slot: PointerValue<'_>, width: crate::Width) {
    let bytes = match width {
        crate::Width::Body(bytes) => bytes,
        crate::Width::Scalar(_) => 0,
        crate::Width::Unknown => return,
    };
    let i64t = b.get_insert_block().unwrap().get_context().i64_type();
    b.build_store(slot, i64t.const_int(u64::from(bytes), false))
        .unwrap();
}
