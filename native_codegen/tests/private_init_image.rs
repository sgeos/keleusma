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

/// **A composite slot starts WRITTEN, with its type's zero, since absorption 124.**
///
/// Until then a composite slot's initializer was `Unit`, the image stopped at
/// the slot array, and a read of an unwritten slot faulted as the reference
/// faulted. V0.2.X's widening, its pull request 666, gives the slot its type's
/// zero and the reference moves that body into the pool at construction. So
/// the image writes the zero body and sets the initialisation word, and a read
/// before the first write agrees with the reference rather than faulting.
#[test]
fn a_composite_slot_starts_written_with_its_types_zero() {
    let source = "struct F { a: Word, b: Word }\n\
         private data log { latest: F, count: Word = 3 }\n\
         fn main(a: Word, b: Word) -> Word { let before = log.latest.a + log.latest.b; \
           log.latest = F { a: a, b: b }; before * 100 + log.latest.a + log.count }\n";
    let m = common::build(source);
    let image = region::private_init_image(&m).expect("placeable");
    let dl = m.data_layout.as_ref().expect("data");
    assert_eq!(
        dl.private_composite_layout.len(),
        1,
        "the subject must declare exactly one composite slot"
    );
    let init = keleusma::vm::required_persistent_capacity_for(&m)
        + region::stream_state_supplement_bytes(&m) as usize;
    assert_eq!(
        image.len(),
        init + 8,
        "the image ends after the one initialisation word"
    );
    assert_eq!(
        image[init..init + 8],
        (1024u64 | (16 << 32)).to_le_bytes(),
        "the composite slot's initialisation word must say written, as a write of \
         a sixteen-byte struct, tag 1024, says it"
    );
    // Read before the first write, the reference sees the zero of `F`, and so
    // must native code. A backend that still faulted, or read garbage, differs.
    for (a, b) in [(4, 9), (-2, 7)] {
        let (vm, native) = common::vm_and_native_two_arg(source, a, b);
        assert_eq!(
            vm,
            a + 3,
            "the reference reads the zero of F before the write"
        );
        assert_eq!(native, vm, "a = {a}, b = {b}");
    }
}

#[test]
fn private_float_initializers_follow_the_declared_module_width() {
    let mut module = common::build(
        "private data st { value: Float = 0.5 } fn main(t: Float) -> Float { if t > 0.0 { st.value = t; } st.value }",
    );
    for (log2, expected) in [
        (5, 0x0000_0000_3f00_0000_u64),
        (6, 0x3fe0_0000_0000_0000_u64),
    ] {
        module.float_bits_log2 = log2;
        let image = region::private_init_image(&module).unwrap();
        assert_eq!(image, expected.to_le_bytes());
    }
    module.float_bits_log2 = 0;
    assert!(region::private_init_image(&module).is_none());
}
