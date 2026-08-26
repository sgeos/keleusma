//! **Is the `Fixed` shared-slot ABI actually UNSETTLED, or merely UNMEASURED?**
//!
//! `alloc_format_kind` in this backend refuses a `Fixed` shared data slot with
//! *"fixed-point representation is unsettled"*, and the doc comment above it
//! defends the refusal on the ground that a shared slot's layout is
//! host-visible and therefore an ABI question rather than an internal one.
//!
//! **That refusal is right and this file does not challenge it. What it
//! challenges is the REASON GIVEN**, which is imprecise in a way that makes the
//! decision look larger and vaguer than it is.
//!
//! # What is settled, and it is more than the message implies
//!
//! The *representation* of `Fixed` is not open. `ScalarKind::Fixed` is a
//! signed Q-format integer of the runtime's word width, and
//! `ScalarKind::size_in_bytes` returns exactly `word_bytes` for it -- eight on
//! a 64-bit word, four on a 32-bit one, pinned by the `v0.2.3` line's own unit
//! tests. A backend that lowered a `Fixed` shared slot as an eight-byte signed
//! integer at the stated offset would agree with the reference byte for byte.
//!
//! # What is genuinely absent is the SCALE, not the representation
//!
//! A `Fixed<N>` value is an integer scaled by `2^N`. **`N` is carried by the
//! OPCODES that produce and consume the value** -- `WordToFixed(frac_bits)`,
//! `FixedToWord(frac_bits)`, `FixedMul(frac_bits)`, `FixedDiv(frac_bits)` --
//! **and by nothing in the layout descriptor.** `value_layout.rs` says so in as
//! many words: *"The fraction-bit count is carried by the opcodes that produce
//! or consume the value, not by the layout descriptor."*
//!
//! Erasing it is sound INSIDE a module. Every producer and consumer of that
//! slot is type-checked against the same `Fixed<N>`, so the scale is a
//! compile-time agreement and the runtime never needs it. `bytecode.rs` states
//! that rationale explicitly for `TypeTag`.
//!
//! **It is not sound across a HOST-VISIBLE boundary.** A host handed the shared
//! buffer receives `word_bytes` of raw two's-complement integer per `Fixed`
//! slot and has nothing in the module that tells it whether those bits are
//! Q16.16 or Q8.24. The two differ by a factor of 256 and both are legal.
//!
//! # So the open question is one question, not an open-ended one
//!
//! Not *"how should fixed-point be represented"* -- that is answered -- but
//! **"where does the host-visible scale live, or is there deliberately none?"**
//!
//! This file pins the three facts that make that the question:
//!
//! 1. the surface ADMITS a `Fixed` shared slot, so the case is live rather than
//!    hypothetical;
//! 2. the compiled module's shared-slot descriptor carries the `Fixed` tag and
//!    **no scale**, with the declared `N` recoverable from no field of it;
//! 3. two modules differing ONLY in `N` produce **byte-identical shared-slot
//!    layouts**, which is the sharp form of (2) and the one that cannot be
//!    explained away as "the scale is somewhere else in the table".
//!
//! Fact 3 is the one worth having. A missing field is an observation about a
//! struct; two programs with different semantics and indistinguishable
//! host-visible layouts is an observation about the ABI.
//!
//! # This file asserts nothing about what SHOULD be done
//!
//! The disposition is an operator decision and the wire schema belongs to the
//! `v0.2.3` line. See `docs/decisions/FIXED_SHARED_SLOT_ABI.md` for the options
//! and their costs. **If a scale ever becomes recoverable, fact 3 fails and
//! this file is how that is noticed** -- which is the point of pinning a
//! negative.

use keleusma::bytecode::{SHARED_SLOT_COMPOSITE_FLAG, SharedSlotLayout};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

/// `ScalarKind::Fixed::to_tag()`. Written numerically on purpose: the variant
/// is reachable without a feature gate today, but every other tag comparison in
/// this package is numeric for the reason `SCALAR_FLOAT_TAG` records, and a
/// wire tag is stable regardless of how the reader was built.
const SCALAR_FIXED_TAG: u8 = 4;

fn source_with_scale(n: u32) -> String {
    format!(
        "shared data cal {{\n\
         \x20   scale: Fixed<{n}>,\n\
         \x20   count: Word,\n\
         }}\n\
         \n\
         fn main() -> Word {{\n\
         \x20   let s = cal.scale;\n\
         \x20   let w = s as Word;\n\
         \x20   w + cal.count\n\
         }}\n"
    )
}

fn module_of(src: &str) -> keleusma::bytecode::Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

fn shared_layout_of(src: &str) -> Vec<SharedSlotLayout> {
    let module = module_of(src);
    module
        .data_layout
        .as_ref()
        .expect("a module declaring shared data must carry a data layout")
        .shared_layout
        .clone()
}

/// **FACT 1 AND FACT 2.** The surface admits the slot, and the descriptor that
/// reaches the host carries a kind and an offset and no scale.
#[test]
fn a_fixed_shared_slot_compiles_and_its_descriptor_carries_no_scale() {
    let layout = shared_layout_of(&source_with_scale(16));

    let fixed: Vec<&SharedSlotLayout> = layout
        .iter()
        .filter(|s| s.kind & SHARED_SLOT_COMPOSITE_FLAG == 0 && s.kind == SCALAR_FIXED_TAG)
        .collect();

    assert_eq!(
        fixed.len(),
        1,
        "expected exactly one scalar `Fixed` shared slot; the whole question is \
         moot if the surface stopped admitting one, and this is how that would \
         be noticed rather than silently making the file vacuous. Layout: {layout:?}"
    );

    let slot = fixed[0];

    // `len` is documented as `0` for a scalar slot. That it is UNUSED is the
    // reason it appears in the options list as a place a scale could live at no
    // wire cost; asserting it here keeps that option honest, because the option
    // evaporates if `len` ever acquires a scalar meaning.
    assert_eq!(
        slot.len, 0,
        "a scalar shared slot's `len` is documented as 0. It is non-zero here, \
         so it now carries something, and the `docs/decisions/FIXED_SHARED_SLOT_ABI.md` \
         option that proposed reusing it needs re-costing"
    );

    // The declared scale is 16. Nothing in the descriptor is 16 except by
    // coincidence of an offset, so state the claim over the fields that could
    // conceivably encode it rather than over the whole struct.
    assert_ne!(
        u32::from(slot.len),
        16,
        "the scale became recoverable from `len`; fact 2 no longer holds"
    );
    assert_ne!(
        u32::from(slot.kind),
        16,
        "the scale became recoverable from `kind`; fact 2 no longer holds"
    );
}

/// **FACT 3, AND THE ONE THAT MATTERS.** Two programs whose fixed-point
/// semantics differ by a factor of 256 present the host with byte-identical
/// shared-slot layouts.
///
/// A host that reads this buffer correctly for one of them reads it wrong by
/// 256x for the other, and there is nothing in the module it could have
/// consulted to tell them apart.
#[test]
fn two_scales_produce_indistinguishable_host_visible_layouts() {
    let q16 = shared_layout_of(&source_with_scale(16));
    let q8 = shared_layout_of(&source_with_scale(8));

    assert_eq!(
        q16, q8,
        "`Fixed<16>` and `Fixed<8>` now produce DIFFERENT shared-slot layouts. \
         That is the ABI gap closing, which is good news and not a regression -- \
         but `docs/decisions/FIXED_SHARED_SLOT_ABI.md` and this backend's \
         `alloc_format_kind` refusal both describe a state that no longer holds \
         and must be updated together"
    );
}

/// The control that keeps the two tests above from passing vacuously.
///
/// If `compile` silently dropped the `Fixed` field, or the shared segment came
/// out empty, both assertions above would still pass. This one fails instead.
#[test]
fn the_probe_is_not_vacuous_the_segment_has_both_slots() {
    let layout = shared_layout_of(&source_with_scale(16));
    assert_eq!(
        layout.len(),
        2,
        "the probe declares two shared slots -- a `Fixed<16>` and a `Word`. \
         A different count means the segment is not what this file thinks it is \
         measuring. Layout: {layout:?}"
    );
    let kinds: Vec<u8> = layout.iter().map(|s| s.kind).collect();
    assert!(
        kinds.contains(&SCALAR_FIXED_TAG),
        "no `Fixed` slot in the segment, so the two tests above measure nothing. \
         Kinds: {kinds:?}"
    );
}

/// **FACT 1 IN ITS STRONG FORM.** The declaration does not merely compile; it
/// passes the structural verifier and receives a worst-case memory bound.
///
/// This is the difference between *"the parser accepted it"* and *"a host could
/// be handed this module and run it"*. Only the second makes the missing scale
/// an ABI gap rather than a curiosity about an unreachable surface.
#[test]
fn a_fixed_shared_slot_verifies_and_receives_a_memory_bound() {
    let module = module_of(&source_with_scale(16));
    keleusma::verify::verify(&module)
        .expect("a module with a `Fixed` shared slot must pass the structural verifier");
    keleusma::verify::module_wcmu(&module, &[])
        .expect("a module with a `Fixed` shared slot must receive a worst-case memory bound");
}
