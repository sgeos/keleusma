//! **A DECLARED INITIALIZER ON A PRIVATE DATA SLOT, WHICH THIS BACKEND NEVER
//! APPLIED.**
//!
//! # The defect
//!
//! A private scalar slot carries its declared `= literal` initializer, or its
//! type's zero, in the module's `private_init` table. The reference applies it
//! when the module is loaded. **There is no native load step** — a host supplies
//! the buffer — so nothing applied it here, and a program reading a slot it had
//! not written got the host's zeros.
//!
//! ```text
//! private data log { count: Word = 7 }
//! fn main(t: Word, u: Word) -> Word { if t < 0 { log.count = 99; } log.count + u }
//!
//!   write skipped : reference 8, this backend 1
//!   write taken   : reference 100, this backend 100
//! ```
//!
//! **No fault, just a wrong value** — and every existing subject agreed, because
//! every one of them writes its slots before reading them.
//!
//! # Found deliberately, by the axis neither census covered
//!
//! Two censuses cover how an address is FORMED and how a value is MOVED. The
//! third axis is **memory the emitter READS but did not write**, and enumerating
//! those sites put four on the host-provided boundary: the resume-state word, the
//! initialisation flags, the shared segment, and the private slot array — whose
//! guarantee is a table this backend had never looked at.
//!
//! # The fix is a published image, not a load step
//!
//! `region::private_init_image` states what a host must install, exactly as
//! `persistent_supplement_bytes` states how large the region must be. **It is the
//! same weaker guarantee**: a host that ignores it is wrong in a way publishing
//! cannot prevent.
//!
//! # What it does not cover
//!
//! - **Composite slots stay zero and keep faulting.** Their initializer is
//!   `Unit`, which is not a body; the initialisation word still reports "never
//!   written", which is what makes an unwritten read fault.
//! - **The shared segment is the host's by contract**, and nothing here changes
//!   that.
//! - **A string or tuple initializer refuses the whole image** rather than
//!   installing a partially correct one. No corpus module carries one.

use keleusma_native::region;

mod common;

/// Written on one branch only, so the reading path can be selected by argument.
const CONDITIONAL: &str = "private data log { count: Word = 7 }\n\
                           fn main(t: Word, u: Word) -> Word { if t < 0 { log.count = 99; } log.count + u }\n";

/// **THE DEFECT, INVERTED.** The unwritten path must now agree.
#[test]
fn a_declared_initializer_is_observable_natively() {
    // Positive argument: the write does not happen, so the slot must read as the
    // declared 7.
    let (vm, nat) = common::vm_and_native_two_arg(CONDITIONAL, 4, 1);
    assert_eq!(
        vm, 8,
        "the reference must read the declared initializer; if this changes, the \
         subject is no longer about initialisation"
    );
    assert_eq!(nat, vm, "the unwritten path diverges: native={nat} vm={vm}");
}

/// **Writing still wins.** An image applied at the wrong moment — on every call
/// rather than once — would pass the test above and fail this one.
#[test]
fn a_written_slot_still_reads_what_was_written() {
    let (vm, nat) = common::vm_and_native_two_arg(CONDITIONAL, -1, 1);
    assert_eq!(vm, 100, "the reference must see the write");
    assert_eq!(nat, vm, "the written path diverges: native={nat} vm={vm}");
}

/// **The image is a published surface, driven here through that surface.**
///
/// Not through a test helper's private arrangement: if the only caller were the
/// harness, a real host would still get zeros and the differential would be
/// green about a defect it had hidden.
#[test]
fn the_published_image_carries_the_declared_bytes() {
    let m = common::build(CONDITIONAL);
    let image = region::private_init_image(&m).expect("a Word initializer is placeable");
    assert!(
        image.len() >= 8,
        "the image must cover the one private slot, got {} bytes",
        image.len()
    );
    let first = i64::from_le_bytes(image[..8].try_into().unwrap());
    assert_eq!(
        first, 7,
        "the first private slot's initial word must be the declared literal"
    );

    // **Non-vacuity.** A module whose slot has no declared literal must produce a
    // different image, or this test would pass against a function that returns
    // the same bytes for everything.
    let plain = common::build(
        "private data log { count: Word }\n\
         fn main(t: Word) -> Word { log.count = t; log.count + 1 }\n",
    );
    let plain_image = region::private_init_image(&plain).expect("a Word slot is placeable");
    assert_ne!(
        plain_image, image,
        "a slot with no declared literal produces the same image as one with `= 7`, \
         so the table is not being read"
    );
}

/// **A composite slot is NOT marked written by the image.**
///
/// The previous increment made a read of an unwritten composite slot fault. An
/// image that zero-filled the pool and set the initialisation word would undo
/// that silently, and every test of it would still pass, because a zeroed body
/// reads as a body.
#[test]
fn the_image_does_not_mark_a_composite_slot_as_written() {
    let m = common::build(
        "struct F { a: Word, b: Word }\n\
         private data log { latest: F, count: Word = 3 }\n\
         fn main(t: Word) -> Word { log.latest = F { a: t, b: t }; log.latest.a + log.count }\n",
    );
    let image = region::private_init_image(&m).expect("placeable");
    let slots = m
        .data_layout
        .as_ref()
        .map(|dl| dl.private_composite_layout.len())
        .unwrap_or(0);
    assert_eq!(
        slots, 1,
        "the subject must declare exactly one composite slot"
    );

    // The image covers only the slot array. The initialisation words live beyond
    // it, so an image that reached them would be longer than the array.
    let private_slots = m
        .data_layout
        .as_ref()
        .map(|dl| {
            dl.slots
                .iter()
                .filter(|s| s.visibility == keleusma::bytecode::SlotVisibility::Private)
                .count()
        })
        .unwrap_or(0);
    assert_eq!(
        image.len(),
        private_slots * 8,
        "the image must cover the slot array and nothing beyond it; reaching the \
         initialisation words would mark a composite slot as written"
    );
}
