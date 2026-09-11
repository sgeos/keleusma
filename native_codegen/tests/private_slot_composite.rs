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
//! # What was done
//!
//! **Refused first, then implemented.** The refusal landed in the same increment
//! that found the defect, because the correct lowering needed the pool's base
//! pinned against the runtime and a guess belongs nowhere near a `memcpy`
//! length. The copy landed next: the write copies the body into the persistent
//! composite pool and the read hands back the pool address, so the value
//! survives `Op::Reset` in place exactly as the runtime's copy does.
//!
//! **The size is DERIVED and the derivation is validated.** `src/compiler.rs`
//! packs the pool with one running total and no padding, so a body's size is the
//! gap to the next entry's offset and the last entry's is the gap to the declared
//! pool size. The table is required to partition the pool before any size is
//! used — a length feeding a copy is the one place here where being slightly
//! wrong is an overrun rather than a wrong answer.
//!
//! # What this file does not establish
//!
//! - **An INDEXED composite slot is refused, not lowered.** Every element slot
//!   carries its own pool entry, and `base + index * size` needs the stride
//!   proven uniform across the range. No corpus module declares an array of
//!   composites, so there is no subject to prove it against.
//! - **An empty composite slot is absent from the pool table by design**, so a
//!   zero-byte composite slot takes neither path.
//! - **Shared composite slots** are refused by the operand-width leg on the write
//!   side only. No corpus module declares one.
//! - **The validation legs have no corpus subject.** A table that fails to
//!   partition the pool cannot be produced by the current compiler, so those
//!   refusals are reasoned rather than driven.

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

/// **THE DISCRIMINATOR, NOW ASSERTING AGREEMENT.**
///
/// It kept its subject and swapped its claim: the same program that separated a
/// copy from an alias now agrees with the runtime yield for yield. Every other
/// subject agrees under BOTH mechanisms, so deleting this one would leave the
/// property untested the moment it was implemented.
#[test]
fn the_composite_slot_is_copied_and_agrees_with_the_runtime() {
    common::assert_general_stream_agrees(DISCRIMINATOR, 1, &[1, 1]);
}

/// **The copy survives `Op::Reset`.**
///
/// Written on the first cycle only and read on every later one. The runtime's
/// persistent region is not reclaimed at a rewind, and neither is this backend's
/// pool — but for a DIFFERENT reason on each side, which is why it is driven
/// rather than argued: the runtime keeps a `Value` in the persistent region, and
/// this backend keeps bytes at a statically placed pool offset.
#[test]
fn a_composite_written_once_is_still_there_cycles_later() {
    const WRITE_ONCE: &str = "\
struct F { a: Word, b: Word }\n\
private data log { latest: F, count: Word }\n\
loop main(t: Word) -> Word {\n\
    if t == 0 { log.latest = F { a: 42, b: 7 }; }\n\
    log.count = log.count + 1;\n\
    let _ack = yield log.latest.a + log.count;\n\
    0\n\
}\n";
    let vm = common::general_vm_sequence(WRITE_ONCE, 0, &[5, 5, 5]);
    assert_eq!(
        vm[0], 43,
        "the first cycle writes and reads back 42, plus a count of one"
    );
    assert!(
        vm[1] > vm[0],
        "later cycles must still see the composite, or this subject proves nothing \
         about survival: {vm:?}"
    );
    common::assert_general_stream_agrees(WRITE_ONCE, 0, &[5, 5, 5]);
}

/// A scalar slot still lowers and agrees. The control: if composites had been
/// made to work by loosening something about data slots generally, this would
/// not be the test that noticed — but its failure would say the change reached
/// further than intended.
#[test]
fn the_scalar_slot_still_agrees_with_the_runtime() {
    common::assert_general_stream_agrees(SCALAR_SLOT, 1, &[1, 1, 1]);
}

/// **The corpus subject runs again**, and the point is not the refusal count.
///
/// It agreed before the defect was found, by coincidence: it reads its slot in
/// the iteration that wrote it. It agrees now by construction.
#[test]
fn the_corpus_subject_lowers_and_agrees() {
    let src = std::fs::read_to_string("../examples/scripts/14_frame_log.kel")
        .expect("the corpus module that homes a composite in a private slot");
    let m = common::try_build(&src).expect("it compiles");
    let refusals = module_refusals(&m, LowerOptions::default());
    assert!(
        refusals.is_empty(),
        "the corpus module must lower now that the pool copy exists: {refusals:?}"
    );
    // Its own header says it yields 81, then 84, 87, 90 on successive cycles.
    let vm = common::general_vm_sequence(&src, 1, &[1, 1, 1, 1]);
    assert_eq!(
        vm,
        vec![81, 84, 87, 90],
        "the runtime no longer matches the script's own documented sequence, so \
         the subject has changed under this test"
    );
    let nat = common::general_native_sequence(&src, 1, &[1, 1, 1, 1]);
    assert_eq!(
        nat, vm,
        "14_frame_log.kel diverges: native={nat:?} vm={vm:?}"
    );
}

/// **An indexed composite slot is refused, and the control is the direct one.**
///
/// Without the pair this says only that something about the subject is
/// unsupported. With it, the refusal is specifically about the INDEXED form.
#[test]
fn an_indexed_composite_slot_is_refused_while_the_direct_one_lowers() {
    const ARRAY_OF_COMPOSITE: &str = "\
struct F { a: Word, b: Word }\n\
private data log { items: [F; 3], count: Word }\n\
loop main(t: Word) -> Word {\n\
    log.items[0] = F { a: 1, b: 2 };\n\
    let _ack = yield log.items[0].a;\n\
    0\n\
}\n";
    match common::try_build(ARRAY_OF_COMPOSITE) {
        None => {
            // The shape is not admitted upstream, which is a fact about the
            // reference compiler rather than about this backend. Recorded here so
            // the absence of a refusal is not read as support.
            println!(
                "an array-of-composite private slot does not compile on the reference \
                 compiler, so the backend's refusal for it has no subject"
            );
        }
        Some(m) => {
            let refusals = module_refusals(&m, LowerOptions::default());
            assert!(
                !refusals.is_empty(),
                "an indexed composite slot must be refused: every element carries its \
                 own pool entry and the stride is not proven uniform"
            );
            let why = format!("{refusals:?}");
            assert!(
                why.contains("INDEXED"),
                "the refusal must name the indexing, or it could be any other \
                 limitation of this subject: {why}"
            );
        }
    }
    let direct = module_refusals(&common::build(DISCRIMINATOR), LowerOptions::default());
    assert!(
        direct.is_empty(),
        "the direct form must still lower, or the refusal above is not about \
         indexing: {direct:?}"
    );
}

/// **SURVIVAL AND REWRITE, TOLD APART.**
///
/// `a_composite_written_once_is_still_there_cycles_later` is satisfied by a pool
/// that is merely never overwritten, which is a weaker property than surviving a
/// reset. This subject writes on some cycles and not others, so the sequence has
/// to show the value PERSISTING across a cycle that does not write AND CHANGING
/// on one that does. A lowering that leaked the write, or one that reset the
/// pool, fails on a different element of the same sequence.
#[test]
fn the_pool_persists_across_a_silent_cycle_and_changes_on_a_writing_one() {
    const ALTERNATING: &str = "\
struct F { a: Word, b: Word }\n\
private data log { latest: F, count: Word }\n\
loop main(t: Word) -> Word {\n\
    if t > 0 { log.latest = F { a: t * 10, b: 0 }; }\n\
    let _ack = yield log.latest.a;\n\
    0\n\
}\n";
    let vm = common::general_vm_sequence(ALTERNATING, 3, &[0, 7, 0]);
    assert_eq!(
        vm,
        vec![30, 30, 70],
        "the runtime must show the value persisting through the silent cycle and \
         changing on the writing one, or this subject does not discriminate: {vm:?}"
    );
    common::assert_general_stream_agrees(ALTERNATING, 3, &[0, 7, 0]);
}
