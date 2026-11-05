//! **THE PER-SLOT TABLES THE LOWERING READS, EXPANDED FROM THE FIELD LAYOUT.**
//!
//! Encoding F made `keleusma::bytecode::DataLayout` field-based. One
//! `DataField` describes every element of a declared field, its first logical
//! slot, its element count, its kind, the byte offset of element zero and the
//! stride between elements, and one initialiser for all of them. The
//! per-slot vectors the lowering was written against, `shared_layout`,
//! `private_composite_layout` and `private_init`, are gone from the runtime.
//!
//! The lowering's logic is slot-indexed throughout, because `GetData` and
//! `SetData` name logical slots. This module expands the fields into those
//! per-slot tables, so that logic is unchanged. **Each table is built once per
//! module, never per operation**, which matters because a private array of a
//! few thousand elements would otherwise be expanded at every data access.
//!
//! Every function is total. A module without a data layout yields empty
//! tables, which is what the removed fields held for such a module.

use keleusma::bytecode::{ConstValue, DataLayout, SharedSlotLayout, SlotVisibility};

/// The persistent-pool placement of one private slot that holds a flat
/// composite body, the entry the runtime's `PrivateCompositeSlot` carried
/// before Encoding F.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrivateCompositeSlot {
    /// Unified logical slot index, shared slots first.
    pub slot: u32,
    /// Byte offset of this slot's flat composite body in the persistent pool.
    pub offset: u32,
}

/// Number of logical slots, the bound of a `GetData`/`SetData` operand.
pub fn slot_count(layout: Option<&DataLayout>) -> usize {
    layout.map_or(0, DataLayout::slot_count)
}

/// Number of logical slots in shared fields, which occupy the low indices, so
/// this is also the first private slot.
pub fn shared_slot_count(layout: Option<&DataLayout>) -> usize {
    layout.map_or(0, DataLayout::shared_slot_count)
}

/// Every shared slot's host-buffer layout, in slot order.
pub fn shared_layout(layout: Option<&DataLayout>) -> Vec<SharedSlotLayout> {
    layout.map_or_else(Vec::new, DataLayout::shared_slots)
}

/// The name of every shared slot, in slot order. Every element of an array
/// field carries the field's name, as each element slot did before Encoding F,
/// so a consumer that collapses runs of equal names sees the same runs.
pub fn shared_slot_names(layout: Option<&DataLayout>) -> Vec<&str> {
    let Some(layout) = layout else {
        return Vec::new();
    };
    layout
        .fields
        .iter()
        .filter(|field| field.visibility == SlotVisibility::Shared)
        .flat_map(|field| core::iter::repeat_n(field.name.as_str(), field.count as usize))
        .collect()
}

/// The host-buffer offset of element zero of the first shared field whose
/// name satisfies `matches`. A field is matched once by its name, `block.field`,
/// where before Encoding F every element slot of an array carried that name.
pub fn shared_field_offset(layout: &DataLayout, matches: impl Fn(&str) -> bool) -> Option<u32> {
    layout
        .fields
        .iter()
        .filter(|field| field.visibility == SlotVisibility::Shared)
        .find(|field| matches(&field.name))
        .and_then(|field| field.offset)
}

/// The pool placement of every private element whose field has a pool home,
/// sorted ascending by slot, one entry per element. A private composite field
/// without a pool home, an empty composite, contributes nothing, as it did
/// before Encoding F.
pub fn private_composite_layout(layout: Option<&DataLayout>) -> Vec<PrivateCompositeSlot> {
    let Some(layout) = layout else {
        return Vec::new();
    };
    let mut slots = Vec::new();
    for field in &layout.fields {
        let (SlotVisibility::Private, Some(offset)) = (field.visibility, field.offset) else {
            continue;
        };
        for element in 0..field.count {
            slots.push(PrivateCompositeSlot {
                slot: field.first_slot.saturating_add(element),
                offset: offset.saturating_add(element.saturating_mul(field.stride)),
            });
        }
    }
    slots
}

/// The pool offset of private `slot`'s body and the offset that ends it, the
/// next larger pool offset or `pool_bytes` when none follows. This is the rule
/// the per-slot table was read by before Encoding F, a body extending to the
/// next placement, kept so a caller validating the pool partition sees the same
/// extents. `None` when the slot has no pool home.
pub fn private_pool_span(layout: &DataLayout, slot: u32, pool_bytes: u32) -> Option<(u32, u32)> {
    let offset = layout.private_composite_offset(slot as usize)?;
    let end = layout
        .fields
        .iter()
        .filter(|field| field.visibility == SlotVisibility::Private)
        .filter_map(|field| {
            let first = field.offset?;
            // The first element of this field placed after `offset`, if any.
            if first > offset {
                return Some(first);
            }
            if field.stride == 0 {
                return None;
            }
            let next = (offset - first) / field.stride + 1;
            (next < field.count).then(|| first.saturating_add(next.saturating_mul(field.stride)))
        })
        .filter(|next| *next > offset)
        .min()
        .unwrap_or(pool_bytes);
    Some((offset, end))
}

/// The load-time value of every private slot, in private-slot order, so entry
/// `i` initialises the `i`-th private slot.
pub fn private_init(layout: Option<&DataLayout>) -> Vec<ConstValue> {
    layout.map_or_else(Vec::new, DataLayout::private_init_values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

    fn layout(src: &str) -> DataLayout {
        compile(&parse(&tokenize(src).expect("lex")).expect("parse"))
            .expect("compile")
            .data_layout
            .expect("a data layout")
    }

    /// An array field expands to one entry per element, and a private
    /// composite array advances its pool offset by the stride.
    #[test]
    fn fields_expand_to_one_entry_per_element() {
        let l = layout(
            "shared data io { a: Word, b: [Word; 3] }\n\
             private data st { p: [(Word, Word); 2], n: Word }\n\
             fn main() -> Word { st.p[1] = (io.a, io.b[2]); st.n = 1; st.p[1].0 }",
        );
        assert_eq!(slot_count(Some(&l)), 7);
        assert_eq!(shared_slot_count(Some(&l)), 4);
        assert_eq!(shared_layout(Some(&l)).len(), 4);
        assert_eq!(
            shared_slot_names(Some(&l)),
            ["io.a", "io.b", "io.b", "io.b"]
        );
        let pool = private_composite_layout(Some(&l));
        assert_eq!(pool.len(), 2);
        assert_eq!((pool[0].slot, pool[1].slot), (4, 5));
        assert_eq!(pool[1].offset - pool[0].offset, 16);
        assert_eq!(private_init(Some(&l)).len(), 3);
    }

    /// No layout yields the empty tables the removed fields held.
    #[test]
    fn no_layout_yields_empty_tables() {
        assert_eq!(slot_count(None), 0);
        assert!(shared_layout(None).is_empty());
        assert!(shared_slot_names(None).is_empty());
        assert!(private_composite_layout(None).is_empty());
        assert!(private_init(None).is_empty());
    }
}
