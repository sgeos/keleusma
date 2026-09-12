//! **EVERY PLACE THE EMITTER READS MEMORY IT DID NOT WRITE, AND WHAT GUARANTEES
//! THE CONTENTS.**
//!
//! # The third axis, and the one that was missing
//!
//! `pointer_offset_census.rs` covers how an address is FORMED.
//! `value_movement_census.rs` covers how a value is MOVED to a destination.
//! **Neither asks what is in memory before the emitter reads it**, and two
//! defects in one day sat on exactly that:
//!
//! - a composite data slot read before it was written answered `0`, because the
//!   persistent pool is bytes and bytes have no `Unit`;
//! - a private scalar slot with a declared `= 7` read as `0`, because the
//!   runtime applies `private_init` at load and **there is no native load step**.
//!
//! The second was found by writing this census's question down and asking it of
//! all sixteen read sites, rather than by another accident.
//!
//! | read | what guarantees the contents |
//! |---|---|
//! | operand slot (`peek`, `pop`) | a push earlier in the same call |
//! | local slot (`GetLocal`) | zeroed on first entry, cleared at `Op::Reset`; the runtime's `Unit` is this backend's zero |
//! | operand spill slot | written at the `yield` that suspended, read only by that yield's resume block |
//! | composite field and array element | the constructor that built the body, which is the only producer of a pointer to it |
//! | enum discriminant | the same |
//! | **stream resume-state word** | **the host's zeroed persistent buffer.** Zero means "the loop top", which is the correct meaning for a fresh instance |
//! | **composite initialisation word** | **the host's zeroed buffer.** Zero means "never written", and a read of such a slot FAULTS |
//! | **private slot array** | **the host, installing `region::private_init_image`.** This is the guarantee that did not exist |
//! | **shared data segment** | **the host, by contract.** Out of this backend's reach and deliberately so |
//!
//! # Three of the four host-boundary rows are satisfied by a ZEROED buffer
//!
//! That is worth stating because it is why the fourth went unnoticed: a host that
//! zeroes the region satisfies the resume-state word, the initialisation flags,
//! and every composite slot, and is wrong only about scalar initializers. **A
//! plausible host is right three times out of four**, which is exactly the
//! coverage pattern that lets a defect live.
//!
//! # What this census cannot do
//!
//! It counts read sites and records a guarantee for each. **It does not verify
//! that any guarantee holds** — the differential does that by execution, and the
//! private-slot row had a written guarantee that was false until the image was
//! published. What it refuses is a new read appearing with no answer to the
//! question.

mod common;

/// The forms by which the emitter reads memory.
const READ_FORMS: &[&str] = &["build_load("];

/// Read sites in the emitter, at the stamp.
const RECORDED_READ_SITES: usize = 16;
// 16 at first derivation, 2026-09-11. Four are on the host-provided boundary —
// the resume-state word, the composite initialisation word, the private slot
// array and the shared segment — and the rest read memory this lowering wrote
// earlier in the same call.

fn read_sites() -> Vec<(usize, String)> {
    let src = std::fs::read_to_string("src/lib.rs").expect("the emitter is readable");
    src.lines()
        .enumerate()
        .filter(|(_, l)| READ_FORMS.iter().any(|f| l.contains(f)))
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .collect()
}

#[test]
fn every_memory_read_has_a_stated_guarantee() {
    let sites = read_sites();
    println!("\n================ MEMORY READS IN THE EMITTER");
    for (n, l) in &sites {
        println!("  src/lib.rs:{n}  {}", &l[..l.len().min(72)]);
    }
    println!("  ------------------------------------------------");
    println!("  sites: {}", sites.len());
    println!(
        "\n  Every read's guarantee is recorded in this file's header. FOUR are on\n  \
         the host-provided boundary, and a zeroed buffer satisfies three of\n  \
         them — which is why the fourth went unnoticed.\n================\n"
    );

    assert!(
        sites.len() > 8,
        "only {} read sites found; the matcher has gone stale and this census is \
         measuring nothing",
        sites.len()
    );
    assert_eq!(
        sites.len(),
        RECORDED_READ_SITES,
        "the number of memory reads in the emitter has changed. Do not patch the \
         number: record in this file's header what guarantees the new read's \
         contents. A read whose guarantee is the host's needs a published way for \
         the host to satisfy it — that is what the private slot array lacked."
    );
}

/// **THE HOST-BOUNDARY ROWS, DRIVEN.**
///
/// The header says three of four are satisfied by a zeroed buffer and one needs
/// an installed image. That is a claim about this backend's published surface,
/// so it is checked against that surface rather than left as prose.
#[test]
fn the_host_boundary_rows_have_a_published_way_to_be_satisfied() {
    use keleusma_native::region;

    let m = common::build(
        "struct F { a: Word, b: Word }\n\
         private data log { latest: F, count: Word = 5 }\n\
         loop main(t: Word) -> Word { log.latest = F { a: t, b: t }; \
           let _a = yield log.latest.a + log.count; 0 }\n",
    );

    // The image exists and carries the declared literal, so the one row that a
    // zeroed buffer does NOT satisfy has a published remedy.
    let image = region::private_init_image(&m).expect("placeable");
    assert!(
        image.iter().any(|b| *b != 0),
        "the published image is all zeros for a module declaring `= 5`, so the row \
         that needs it is not actually served"
    );

    // The other three rows live beyond the slot array, and a host zeroes them.
    // Asserting the image stops at the array is what keeps "zero means never
    // written" true for the composite slots.
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
        "the image must cover the slot array only; the resume-state word and the \
         initialisation flags mean 'fresh' when zero, and an image reaching them \
         would make a slot claim to hold a body it does not"
    );
}
