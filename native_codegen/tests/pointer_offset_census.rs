//! **EVERY POINTER-ARITHMETIC SITE IN THE EMITTER, AND WHERE ITS OFFSET COMES
//! FROM.**
//!
//! # Why this exists
//!
//! On 2026-09-08 `Op::GetIndex` was found computing `base + index * stride` for
//! a RUNTIME index with no bound, on the strength of a comment claiming the
//! compiler emitted a `BoundsCheck` that it does not emit. A three-element array
//! indexed at 5 returned the caller region's filler bytes where the reference
//! faults.
//!
//! That was found by accident. The premise sweep in
//! `upstream_premise_census.rs` is the deliberate version for COMMENTS; this is
//! the deliberate version for the thing a bad comment endangered — **the pointer
//! arithmetic itself.**
//!
//! # The classification, which is the whole content
//!
//! An offset is safe when it is one of two things, and dangerous otherwise:
//!
//! 1. **Compile-time constant** — a local's slot, a field's offset, a call
//!    site's region offset, the stream state word. Nothing a program supplies
//!    reaches it.
//! 2. **Runtime value, bounded before use** — an array index or a data-segment
//!    index, compared UNSIGNED against a bound and branched to the trap on
//!    failure. Unsigned matters: it catches a negative index with the same test.
//!
//! At the stamp, every site is one of those. The two that were not are the
//! `GetIndex` pair, fixed the same day.
//!
//! | site | offset source |
//! |---|---|
//! | locals in the stream frame | constant — `plan_chunk_region` bytes plus slot index |
//! | stream state word (three sites) | constant — `stream_state_off` |
//! | call-site region base | constant — per-site plan |
//! | shared data, indexed and not | runtime, guarded by the opcode's own bound |
//! | private data, indexed and not | runtime, same guard — it precedes the shared/private split |
//! | composite field write and read | constant — the layout's field offset |
//! | composite body base | constant — the site's planned offset |
//! | **flat array element** | **runtime, guarded by `guard_array_index`** |
//! | operand spill store and reload | constant — `spill_off` plus a compile-time slot index |
//! | **nested array element** | **runtime, guarded by `guard_array_index`** — formed by integer ADD, not a gep |
//! | **shared composite body, direct** | constant — the slot's stated offset in the host buffer; the int-to-pointer beside it takes the copy's source address, already formed |
//! | **shared composite body, INDEXED** | runtime, guarded — `first + index * len`, where `index` was compared UNSIGNED against the instruction's declared element count, and the range was proven contiguous at that stated length and uniform in kind before any address was formed |
//! | **composite-slot initialisation word** | constant — the flag array's base plus the slot's position in the module's own pool table, both fixed at lowering |
//! | **persistent composite pool, direct** | constant — this backend's private-slot count times its slot width, plus the module table's pool offset. Neither term is a program value, and the placement is refused outright if it would reach the resume-state word |
//! | **persistent composite pool, INDEXED** | runtime, guarded — `first + index * size`, where `index` was compared UNSIGNED against the instruction's own declared element count before the shared/private split, and `size` is validated uniform across the whole declared range |
//! | **composite initialisation word, indexed** | runtime, guarded by the same check — the flag index moves with the element, so a written element cannot mark a sibling |
//! | five int-to-pointer conversions | each takes an address already formed above; they add no offset of their own |
//!
//! # ⚠ A COUNT CANNOT SEE A SITE CHANGE CLASS
//!
//! **Recorded 2026-09-11, when it happened — and then AGAIN the same day.** The
//! indexed composite data slot turned the pool address from a compile-time
//! constant into `first + index * size`, and the indexed SHARED composite did the
//! same to the host-buffer address. Both times the count held steady and the
//! table had to be revised by hand, which is the limitation working exactly as
//! this section describes and not being fixed by it. It is the SAME LINE — the same gep, fed a computed offset — so the
//! count did not move, this census passed unchanged, and nothing forced the
//! classification above to be revisited.
//!
//! A first attempt widened the matcher to bare `build_int_add`, which took the
//! count from 22 to 38 by sweeping in every ordinary integer operation in the
//! emitter. **That is not a stricter census, it is a broken one**: conflating
//! value arithmetic with address arithmetic would make every future increment
//! move the number for reasons unrelated to addressing.
//!
//! The honest statement is that this file catches NEW sites and not RECLASSIFIED
//! ones, and that the rows above must be re-read whenever an existing site gains
//! a runtime operand. No mechanism here enforces that.
//!
//! # What this test can and cannot do
//!
//! It counts sites. **It cannot prove a given site is guarded** — that is what
//! the differential and `partial_operation_census.rs` do by execution. What it
//! refuses is silent GROWTH: a new pointer-arithmetic site fails this, and the
//! author must classify it above. A site added without a bound is how the
//! `GetIndex` defect existed for as long as it did.

/// Pointer-arithmetic sites in the emitter, at the stamp.
///
/// **Re-derive rather than trust.** It moves whenever a site is added or
/// removed.
const RECORDED_GEP_SITES: usize = 24;
// 22 -> 24 on 2026-09-11, when the shared composite slot landed. **Both are
// compile-time constant.** The body's address is the slot's STATED offset in the
// host buffer — a field of the layout, not a computed quantity — and the second
// site is the int-to-pointer of the copy's source, which is the operand's own
// address and adds no offset.
//
// 21 -> 22 on 2026-09-11, when the composite-slot initialisation words landed.
// **Compile-time constant.** The flag's address is the array base plus the slot's
// INDEX IN THE MODULE'S OWN TABLE, times eight — a position the emitter computes
// while lowering, never a value the program supplies. The read-side test of that
// flag is a load and a compare, neither of which forms an address.
//
// 19 -> 21 on 2026-09-11, when the persistent composite copy landed. **Both are
// compile-time constant.** The pool address is a gep at `private slots x slot
// width + table offset`, every term fixed at lowering; the second site is the
// int-to-pointer of the copy's SOURCE, which is the operand's own address and
// adds no offset. The copy LENGTH is not pointer arithmetic and is not counted
// here — it is a derived size, validated against the module's declared pool
// total in `private_composite_extent`, which is the check that matters for it.
//
// 12 -> 14 on 2026-09-09, when the operand spill slice landed. Both new sites
// take a COMPILE-TIME constant offset: the slice base plus a slot index the
// emitter counts out at lowering time, never a value the program supplies.
// Classified here rather than absorbed into the count, which is the whole
// contract of this file.

/// Every way an address is FORMED in the emitter, not just the one this census
/// was born from.
///
/// # ⚠ THIS LIST WAS ONE ENTRY LONG AND THAT WAS THE BLIND SPOT
///
/// The census matched `build_in_bounds_gep` alone, because the defect that
/// prompted it — the unguarded flat array index — used a gep. **The NESTED array
/// arm does the same arithmetic with `build_int_add` on the raw address**, and
/// was therefore invisible to a file whose header claimed to cover "every
/// pointer-arithmetic site in the emitter".
///
/// It is guarded, so nothing was wrong. **The CLAIM was broader than the
/// measurement**, which is the shape this line records as fabricated coverage.
///
/// Found by asking of this census the question that produced the session's other
/// findings: *what shape was the checker cast in?*
const ADDRESS_FORMS: &[&str] = &[
    // A pointer plus a byte offset.
    "build_in_bounds_gep",
    // The same arithmetic done on the address as an INTEGER, which a gep-only
    // matcher cannot see.
    //
    "build_int_add(parent",
    // An integer becoming a pointer: where a computed address enters pointer
    // space and every later use trusts it.
    "build_int_to_ptr",
];

fn gep_sites() -> Vec<(usize, String)> {
    let src = std::fs::read_to_string("src/lib.rs").expect("the emitter is readable");
    src.lines()
        .enumerate()
        .filter(|(_, l)| ADDRESS_FORMS.iter().any(|f| l.contains(f)))
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .collect()
}

#[test]
fn every_pointer_offset_is_constant_or_bounded() {
    let sites = gep_sites();
    println!("\n================ POINTER-ARITHMETIC SITES IN THE EMITTER");
    for (n, _) in &sites {
        println!("  src/lib.rs:{n}");
    }
    println!("  ------------------------------------------------");
    println!("  sites: {}", sites.len());
    println!(
        "\n  An offset is safe when it is a COMPILE-TIME CONSTANT or a RUNTIME\n  \
         value BOUNDED before use. Every site is classified in this file's\n  \
         header. The two that were neither returned foreign memory.\n================\n"
    );

    // **NON-VACUITY.** A matcher that finds nothing would pass forever.
    assert!(
        !sites.is_empty(),
        "no pointer-arithmetic site matched, so either the emitter stopped using \
         `build_in_bounds_gep` or this matcher has gone stale. A census that \
         cannot fire is indistinguishable from no census."
    );
    assert_eq!(
        sites.len(),
        RECORDED_GEP_SITES,
        "the number of pointer-arithmetic sites in the emitter has changed. Do \
         not patch the number: classify the new site in this file's header as \
         COMPILE-TIME CONSTANT or RUNTIME-AND-BOUNDED, and if it is neither, it \
         is the defect this census was written after."
    );
}

/// **The data-segment guard must precede BOTH the shared and private paths.**
///
/// The private path computes `base + index * PRIVATE_SLOT_BYTES` from the same
/// runtime index. It is safe only because the bound check sits ahead of the
/// branch that separates the two, which is a property of the ORDER of two
/// statements and would be silently lost by a refactor that moved either.
#[test]
fn the_data_index_guard_precedes_the_shared_private_split() {
    let src = std::fs::read_to_string("src/lib.rs").expect("the emitter is readable");
    let guard = src
        .find("\"dataoob\"")
        .expect("the data-segment bound comparison is named `dataoob`");
    let private = src
        .find("\"pdataptr\"")
        .expect("the private data pointer is named `pdataptr`");
    let shared = src
        .find("\"sdataptr\"")
        .expect("the shared data pointer is named `sdataptr`");

    assert!(
        guard < shared && guard < private,
        "the data-segment bound check no longer precedes both pointer \
         computations (guard at {guard}, shared at {shared}, private at \
         {private}). The private path indexes by the same runtime value, so it \
         is bounded only by sitting after this check."
    );
}
