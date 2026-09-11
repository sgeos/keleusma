//! **A COMPOSITE IN A DATA SLOT WAS STORED AS A POINTER, NOT AS A COPY.**
//!
//! # The defect
//!
//! A data slot access lowers to one word-sized load or store. For a flat
//! composite the operand is the ADDRESS of a body in the ephemeral region, so:
//!
//! - a write stored that address where the runtime copies the body's bytes into
//!   the persistent composite pool;
//! - a read handed back an address into a region that later iterations overwrite,
//!   and `Op::Reset` does not clear.
//!
//! The slot survives `Reset` and the body does not, which is the whole point of a
//! data slot: `14_frame_log.kel` opens by saying a slot holds *"a COPY of the
//! composite's bytes ... not a reference to the ephemeral body"*.
//!
//! # Why every existing test agreed anyway
//!
//! **The corpus subject reads its slot in the same iteration that wrote it.** The
//! aliased bytes still hold the right values there, so a pointer and a copy give
//! the same answer, and `14_frame_log.kel` yielded `[81, 84, 87, 90]` on both
//! sides across four cycles.
//!
//! Separating the two takes a subject that writes the slot on ONE loop iteration
//! and then rebuilds the SAME construction site twice more before reading back.
//! The runtime yields `0`; the backend yielded `2` — the last body built at that
//! site.
//!
//! # What was done, and what was not
//!
//! **Refused, not fixed.** The correct lowering copies the body to the offset
//! `private_composite_layout` names, but that pool's base is not pinned against
//! the runtime in this ABI yet, and guessing it would put a wrong answer where a
//! refusal belongs. The cost is stated rather than hidden: `14_frame_log.kel`
//! moves from lowering to refused, so the corpus carries one more refusal.
//!
//! # What this file does not establish
//!
//! - **An unknown-width operand still passes the width check.** It refuses on
//!   `is_body`, so a composite reaching the store with an unreconstructed width
//!   would not be caught by that leg. The private-slot table leg does not depend
//!   on widths, and covers every private composite the module declares.
//! - **An empty composite slot is absent from that table by design**, so a
//!   zero-byte composite slot is outside both legs.
//! - **Shared composite slots** are covered only by the width leg, on the write
//!   side. No corpus module declares one, so there is no subject here.

use keleusma_native::{LowerOptions, module_refusals};

mod common;

/// Writes the slot on the first loop iteration only, then rebuilds the same site
/// twice more. A copy keeps `a == 0`; an alias reads back the last body, `a == 2`.
const DISCRIMINATOR: &str = "\
struct F { a: Word, b: Word }\n\
private data log { latest: F, count: Word }\n\
loop main(t: Word) -> Word {\n\
    for i in 0..3 {\n\
        let f = F { a: i, b: i * 2 };\n\
        if i == 0 { log.latest = f; }\n\
        log.count = log.count + 1;\n\
    }\n\
    let _ack = yield log.latest.a;\n\
    0\n\
}\n";

/// The same shape with a SCALAR slot. The control: if this were refused too, the
/// refusal below would be about data slots in general and would say nothing about
/// composites.
const SCALAR_SLOT: &str = "\
private data log { count: Word }\n\
loop main(t: Word) -> Word {\n\
    for i in 0..3 {\n\
        log.count = log.count + 1;\n\
    }\n\
    let _ack = yield log.count;\n\
    0\n\
}\n";

/// **The reference's answer, pinned independently of the backend.**
///
/// Once the write is refused the backend can no longer be asked, so the semantic
/// the eventual copy must reproduce is recorded here from the runtime alone. A
/// future implementation that yields anything but this is wrong, and this is the
/// test that says so.
#[test]
fn the_runtime_keeps_a_copy_and_the_copy_is_the_first_body() {
    let vm = common::general_vm_sequence(DISCRIMINATOR, 1, &[1, 1]);
    assert_eq!(
        vm,
        vec![0, 0],
        "the runtime must yield the body stored on the FIRST iteration; if this \
         changes, the copy semantics this file is about have changed with it"
    );
}

/// **The refusal, and the control that makes it specific.**
#[test]
fn a_composite_data_slot_is_refused_and_a_scalar_one_is_not() {
    let refusals = module_refusals(&common::build(DISCRIMINATOR), LowerOptions::default());
    assert!(
        !refusals.is_empty(),
        "a composite data slot must be refused; lowering it stores the body's \
         ADDRESS and the runtime stores its BYTES"
    );
    let why = format!("{refusals:?}");
    assert!(
        why.contains("composite"),
        "the refusal must name the composite, or it will be read as a general \
         limitation of data slots: {why}"
    );

    let scalar = module_refusals(&common::build(SCALAR_SLOT), LowerOptions::default());
    assert!(
        scalar.is_empty(),
        "a scalar private slot must still lower, or the refusal above is not \
         discriminating: {scalar:?}"
    );
}

/// The scalar control still AGREES, not merely lowers. Lowering says an arm ran.
#[test]
fn the_scalar_slot_still_agrees_with_the_runtime() {
    common::assert_general_stream_agrees(SCALAR_SLOT, 1, &[1, 1, 1]);
}

/// **The corpus cost, recorded where it is paid.**
///
/// `14_frame_log.kel` lowered and agreed before this refusal. It agreed because
/// it reads its slot in the iteration that wrote it, which is exactly the
/// coincidence the discriminator removes.
#[test]
fn the_corpus_subject_that_lowered_by_luck_is_now_refused() {
    let src = std::fs::read_to_string("../examples/scripts/14_frame_log.kel")
        .expect("the corpus module that homes a composite in a private slot");
    let m = common::try_build(&src).expect("it still compiles; only the backend declines it");
    let refusals = module_refusals(&m, LowerOptions::default());
    assert!(
        !refusals.is_empty(),
        "this module writes a composite into a private slot, so it must be refused \
         for the same reason the discriminator is. If it lowers again, either the \
         copy has been implemented -- in which case drive it through the \
         differential and delete this test -- or the refusal has regressed"
    );
}
