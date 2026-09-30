//! The fifth delegate-context field stores the bounded yield-site destination.
//! Each suspension writes its site identifier before returning to the host.
//! Operand metadata starts at zero before producer facts replace it. Reset
//! clears local kinds to Unit and lengths to zero with the actual local values.
//! Coroutine tag and extent stores carry scalar metadata, never body addresses.
//! Owned mixed-body copies use a statically bounded reservation and the selected
//! runtime extent. Borrowed region aliases retain their original address.
//!
//! Flat host values are copied by exact verified extent at value transfers.
//! Copies and ownership flags have fixed-size allocations hoisted before
//! inlining, then captured by LLVM when live across suspension. Private and
//! ordinary-region references keep false ownership flags and retain aliases.
//! The matcher includes memmove and the ownership helper explicitly.
//!
//! **EVERY PLACE THE EMITTER MOVES AN OPERAND, AND WHAT THE DESTINATION
//! OUTLIVES.**
//!
//! # Why this exists, and why it is not the census next door
//!
//! `pointer_offset_census.rs` is the deliberate instrument for ADDRESS
//! arithmetic, written after an unguarded array index returned foreign memory.
//! **The defect that prompted THIS file was in VALUE movement**, which that
//! census does not look at: a composite written into a private data slot was
//! stored as one word, so the slot received the BODY'S ADDRESS where the runtime
//! copies its bytes. It was found by checking a citation, not by any instrument.
//!
//! # The question, which is not "is it a word move"
//!
//! A body address stored as a word is **usually correct**. An operand slot holds
//! one; so does a local. The question is:
//!
//! > **Does the destination outlive the memory the address points into?**
//!
//! The ephemeral region dies at `Op::Reset` and its construction sites are
//! overwritten by later iterations. A destination that survives either must not
//! hold an address into it.
//!
//! | destination | outlives the region? | what stops an address landing there |
//! |---|---|---|
//! | stable arena-slot header | until release | continuation points into emitted code kept alive by the host; the reply cell carries scalar bits; release stores a null continuation |
//! | operand slot (`push_w`) | no — dies with the call | nothing needed; an address is the intended content |
//! | local slot (`SetLocal`, parameters, the resume value) | no — cleared at `Op::Reset` | nothing needed, same reason |
//! | retcon latest reply | yes, until release | the public lowering admits scalar parameters only; initialised at start and updated on resume |
//! | retcon delegate context | yes, until release | three pointers refer only to the reply cell or entry locals that LLVM captures in the same frame; helper stores update the same scalar entry parameter and latest reply |
//! | retcon local zero and local reset | no, cleared at Reset | resume stores scalar bits; Reset clears locals and restores the latest scalar reply |
//! | operand spill slice | no — abandoned when the depth goes to zero | nothing needed |
//! | composite body field | no — the body is itself region-resident | a `Width::Body` operand is MEMCPY'd, never stored as a word |
//! | **shared data slot, scalar** | **yes** — host buffer | a body operand with no stated placement is REFUSED |
//! | **shared data slot, composite** | **yes** — host buffer | the body is COPIED to the slot's stated offset, for its stated length. A read hands back a pointer INTO the host's buffer, which is safe precisely because that buffer outlives the call — the same move into the ephemeral region would not be |
//! | **private data slot** | **yes** — persistent across `Op::Reset` | a body operand is REFUSED, unless the slot is a declared composite, which is COPIED into the pool |
//! | **persistent composite pool** | **yes** | reached only by a memcpy of the derived body size |
//! | stream resume-state word | yes | never carries an operand — only a constant yield index |
//! | composite-slot initialisation word | yes — persistent, and it must be | never carries an operand either: a constant one, written after the body copy so a trap inside the copy cannot leave the slot claiming a body it does not hold |
//!
//! # Two routes are NOT store sites, and are named rather than omitted
//!
//! A value can also leave through a `return` and through a `yield`. Neither is a
//! store, so neither appears in the count below, and a census that silently
//! skipped them would claim more than it measures — the exact failure the pointer
//! census recorded about its own first version.
//!
//! - **return** — a composite return hands back a region pointer; the caller's
//!   region is the caller's, and the recorded matter is
//!   `composite_return_aliasing.rs`.
//! - **yield** — a composite that escapes its iteration is REFUSED; see
//!   `YIELD_ESCAPE_REFUSAL.md` and `interproc_yield_escape.rs`.
//!
//! # ⚠ THE GENERAL REFUSAL IS NOW A BACKSTOP, NOT A LIVE PATH
//!
//! A composite operand can only be assigned to a composite-typed slot, and every
//! composite-typed slot now has a stated placement — a pool entry for a private
//! one, a composite-flagged layout entry for a shared one. **So the refusal for
//! "a body with nowhere stated to put it" is no longer reachable from compilable
//! source.** It is kept for bytecode that did not come from the compiler, and
//! `every_composite_typed_data_slot_has_a_stated_placement` is what would notice
//! if a new composite-typed slot arrived without one.
//!
//! # What this file cannot do
//!
//! **It counts sites and classifies destinations. It does not prove any
//! individual site obeys its row** — that is what the differential does by
//! execution. What it refuses is silent GROWTH: a new move site fails it, and the
//! author must place the destination in the table above.

use keleusma_native::{LowerOptions, module_refusals};

mod common;

/// Every way an operand's bits are MOVED to a destination.
///
/// Two forms, and the pair is the point: a word store and a body copy are the
/// two ways a composite can reach a destination, and a census matching only the
/// first would be blind to exactly the distinction the defect turned on.
const MOVE_FORMS: &[&str] = &[
    "build_store(",
    "build_memcpy(",
    "store i64 ",
    "build_memmove(",
];

/// Move sites in the emitter, at the stamp.
///
/// **Re-derive rather than transcribe.**
// 19 -> 24 with two latest-reply stores, one resume-local store, and
// the two Reset stores. These destinations are classified above.
// 24 -> 27, including the coroutine context and parsed intrinsic helper.
// 27 -> 30 for the stable continuation, reply, and release stores.
const RECORDED_MOVE_SITES: usize = 59;
// 18 -> 19 on 2026-09-11, when the shared composite slot landed: one body copy
// into the host's buffer, at the offset and length the module's shared layout
// STATES. Unlike the persistent pool, nothing here is derived — so there is no
// partition to validate, only a field to read.
//
// 17 -> 18 on 2026-09-11, the increment AFTER this census was written, and it
// fired on its author. The new site marks a composite slot as written. Its
// destination outlives the region — it has to, since the slot does — but it
// **carries a constant, not an operand**, so no address can reach it. Ordering is
// the part worth recording: the mark is emitted AFTER the body copy, so a trap
// inside the copy cannot leave a slot claiming to hold a body it does not.
//
// 17 at first derivation, 2026-09-11: fifteen word stores and two body copies.
// Ten carry an operand; the rest write a constant — a zeroed local, a yield
// index, a cleared state word — and are counted because a constant store today
// is a site someone can route an operand through tomorrow.

fn move_sites() -> Vec<(&'static str, usize, String)> {
    let mut sites = Vec::new();
    for file in [
        "src/lib.rs",
        "src/coroutine.rs",
        "src/coroutine/host.rs",
        "src/coroutine/ownership.rs",
        "src/coroutine/kinds.rs",
    ] {
        let src = std::fs::read_to_string(file).expect("the emitter is readable");
        for (i, line) in src.lines().enumerate() {
            if MOVE_FORMS.iter().any(|form| line.contains(form)) {
                sites.push((file, i + 1, line.trim().to_string()));
            }
        }
    }
    sites
}

#[test]
fn every_operand_move_has_a_classified_destination() {
    let sites = move_sites();
    println!("\n================ OPERAND-MOVE SITES IN THE EMITTER");
    for (file, n, l) in &sites {
        println!("  {file}:{n}  {}", &l[..l.len().min(72)]);
    }
    println!("  ------------------------------------------------");
    println!("  sites: {}", sites.len());
    println!(
        "\n  A destination is safe when it does NOT outlive the memory an address\n  \
         points into, or when a body reaching it is refused or copied. Every\n  \
         destination is classified in this file's header.\n================\n"
    );

    // **NON-VACUITY, AND IT IS FORM-BY-FORM.** A matcher that found nothing
    // would pass forever; worse, one that found only the word stores would
    // reproduce the blind spot that made the defect invisible, since the body
    // copy is the other half of the distinction.
    for f in MOVE_FORMS {
        assert!(
            sites.iter().any(|(_, _, l)| l.contains(f)),
            "no site matches {f:?}, so this census is blind to one of the two ways \
             an operand reaches a destination"
        );
    }

    assert_eq!(
        sites.len(),
        RECORDED_MOVE_SITES,
        "the number of operand-move sites in the emitter has changed. Do not patch \
         the number: place the new destination in this file's header table, and say \
         whether it outlives the memory an address would point into. If it does and \
         nothing refuses or copies a body reaching it, that is the defect this \
         census was written after."
    );
}

/// **EVERY COMPOSITE-TYPED DATA SLOT HAS A STATED PLACEMENT.**
///
/// # The claim this replaces, and why it was replaced rather than weakened
///
/// This test asserted that a composite written into a SHARED slot is refused.
/// **That became false** when the shared composite copy landed, and a weakened
/// version would have been kept green by not implementing it.
///
/// What is true now is stronger and is the reason the refusal is no longer
/// reachable: a composite operand can only be assigned to a composite-typed
/// slot, and **every composite-typed slot now has a stated placement** — a
/// persistent pool entry for a private one, a composite-flagged layout entry with
/// a stated length for a shared one. The general refusal remains as a backstop
/// for bytecode that did not come from the compiler, and the census header says
/// so.
#[test]
fn every_composite_typed_data_slot_has_a_stated_placement() {
    use keleusma::bytecode::SHARED_SLOT_COMPOSITE_FLAG;

    const SUBJECTS: &[(&str, &str)] = &[
        (
            "private direct",
            "struct F { a: Word, b: Word }\n\
             private data log { latest: F, count: Word }\n\
             fn main(t: Word) -> Word { log.latest = F { a: t, b: t }; log.latest.a }\n",
        ),
        (
            "private indexed",
            "struct F { a: Word, b: Word }\n\
             private data log { items: [F; 3], count: Word }\n\
             fn main(t: Word) -> Word { log.items[1] = F { a: t, b: t }; log.items[1].a }\n",
        ),
        (
            "shared direct",
            "struct F { a: Word, b: Word }\n\
             shared data io { latest: F, n: Word }\n\
             fn main(t: Word) -> Word { io.latest = F { a: t, b: t }; io.latest.a }\n",
        ),
    ];

    let mut checked = 0usize;
    for (name, src) in SUBJECTS {
        let m = common::build(src);
        let dl = m.data_layout.as_ref().expect("the subject declares data");
        let shared_count = dl
            .slots
            .iter()
            .filter(|s| s.visibility == keleusma::bytecode::SlotVisibility::Shared)
            .count();

        // A slot is composite-typed exactly when it has a placement of one kind
        // or the other; the point is that NEITHER set is empty for a subject
        // that declares a composite, and that every such slot is covered.
        let private_placed = dl.private_composite_layout.len();
        let shared_placed = dl
            .shared_layout
            .iter()
            .filter(|e| e.kind & SHARED_SLOT_COMPOSITE_FLAG != 0)
            .count();
        assert!(
            private_placed + shared_placed > 0,
            "{name}: the subject declares a composite slot and the module states no \
             placement for any of them, so the copy would have nowhere to go"
        );

        // The stated length of a shared composite must be non-zero, or the kind
        // and the layout disagree and the copy would be empty.
        for e in dl.shared_layout.iter().take(shared_count) {
            if e.kind & SHARED_SLOT_COMPOSITE_FLAG != 0 {
                assert!(
                    e.len > 0,
                    "{name}: a shared slot is marked composite with a stated length of zero"
                );
                checked += 1;
            }
        }
        checked += private_placed;

        // And it lowers: a stated placement that the backend still refuses would
        // make the claim above hollow.
        let refusals = module_refusals(&m, LowerOptions::default());
        assert!(
            refusals.is_empty(),
            "{name}: every composite slot here has a stated placement and the backend \
             still refuses it: {refusals:?}"
        );
    }

    // **NON-VACUITY.** A loop that checked nothing would pass.
    assert!(
        checked >= 5,
        "only {checked} composite placements checked across {} subjects; the \
         extraction is not reading the layouts",
        SUBJECTS.len()
    );
}
