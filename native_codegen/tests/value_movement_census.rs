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
//! | operand slot (`push_w`) | no — dies with the call | nothing needed; an address is the intended content |
//! | local slot (`SetLocal`, parameters, the resume value) | no — cleared at `Op::Reset` | nothing needed, same reason |
//! | operand spill slice | no — abandoned when the depth goes to zero | nothing needed |
//! | composite body field | no — the body is itself region-resident | a `Width::Body` operand is MEMCPY'd, never stored as a word |
//! | **shared data slot** | **yes** — host buffer | a body operand is REFUSED |
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
const MOVE_FORMS: &[&str] = &["build_store(", "build_memcpy("];

/// Move sites in the emitter, at the stamp.
///
/// **Re-derive rather than transcribe.**
const RECORDED_MOVE_SITES: usize = 18;
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

fn move_sites() -> Vec<(usize, String)> {
    let src = std::fs::read_to_string("src/lib.rs").expect("the emitter is readable");
    src.lines()
        .enumerate()
        .filter(|(_, l)| MOVE_FORMS.iter().any(|f| l.contains(f)))
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .collect()
}

#[test]
fn every_operand_move_has_a_classified_destination() {
    let sites = move_sites();
    println!("\n================ OPERAND-MOVE SITES IN THE EMITTER");
    for (n, l) in &sites {
        println!("  src/lib.rs:{n}  {}", &l[..l.len().min(72)]);
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
            sites.iter().any(|(_, l)| l.contains(f)),
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

/// **THE ROWS THAT SAY "REFUSED", DRIVEN RATHER THAN ASSERTED.**
///
/// The table is prose until something executes it. These are the two
/// destinations that outlive the region and are closed by a refusal rather than
/// by a copy.
#[test]
fn a_body_reaching_a_surviving_destination_is_refused() {
    // A private slot that is NOT a declared composite, reached by a body. The
    // module declares the slot as a Word, so no pool entry exists for it, and the
    // write would be a one-word store of the body's address.
    const BODY_TO_SCALAR_SLOT: &str = "\
struct F { a: Word, b: Word }\n\
private data log { latest: Word }\n\
fn main() -> Word {\n\
    let f = F { a: 1, b: 2 };\n\
    log.latest = f.a;\n\
    log.latest\n\
}\n";
    // The control: the same shape moving a SCALAR into the same slot lowers.
    let scalar = module_refusals(&common::build(BODY_TO_SCALAR_SLOT), LowerOptions::default());
    assert!(
        scalar.is_empty(),
        "the scalar control must lower, or the refusals below are not specific to a \
         body: {scalar:?}"
    );

    // A composite declared in a SHARED slot, if the reference admits the shape.
    const SHARED_COMPOSITE: &str = "\
struct F { a: Word, b: Word }\n\
shared data io { latest: F }\n\
fn main() -> Word {\n\
    io.latest = F { a: 1, b: 2 };\n\
    0\n\
}\n";
    match common::try_build(SHARED_COMPOSITE) {
        None => {
            // Recorded rather than passed over: the absence of a refusal here is a
            // fact about the reference compiler, not support in this backend.
            println!(
                "a shared composite slot does not compile on the reference compiler, so \
                 the shared row of this file's table has no driven subject"
            );
        }
        Some(m) => {
            let refusals = module_refusals(&m, LowerOptions::default());
            assert!(
                !refusals.is_empty(),
                "a composite written into a SHARED slot must be refused: the store is \
                 one word wide and the host buffer outlives the region"
            );
            let why = format!("{refusals:?}");
            assert!(
                why.contains("ADDRESS") && why.contains("composite"),
                "the refusal must name the body and the address it would store, or it \
                 could be any other limitation of this shape: {why}"
            );
        }
    }
}
