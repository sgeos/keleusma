//! **DOES THE GENERATED HEADER STATE EVERY BUFFER THE ENTRY TAKES?**
//!
//! # The gap
//!
//! The entry receives three pointers. The header stated the layout of **one**:
//! the shared segment, with a named offset per slot. The shipped C host sized the
//! other two by eye — `int64_t private_region[8]`, `composite_region[64]` — and
//! that file is what a host programmer copies.
//!
//! `policy.kel` declares no private data, so the guess was harmless. **It would
//! have stopped being harmless the moment the example grew a `private data`
//! block**, and nothing would have said so.
//!
//! # Why the subject here is not `policy.kel`
//!
//! Its private figure is 0 and it builds no composite, so it cannot distinguish a
//! correct emitter from one that prints zeros. **A test whose subject makes every
//! figure zero proves only that zero was printed.** The subject below declares a
//! private scalar with an initializer, a composite slot, and a construction, so
//! all three figures are non-zero and the initial image is non-empty.
//!
//! # What the shipped example still does not exercise
//!
//! Stated here rather than left for a reader to find: `policy.kel` drives the
//! shared segment end to end against the reference, and drives **neither** the
//! private region nor the composite region with any content of its own. Its
//! header figures are structurally correct and behaviourally untested; this file
//! covers the structure, and `private_init_image.rs` covers the behaviour.

use keleusma_native::region;

mod common;

/// Declares a private scalar with an initializer, a composite slot, and a
/// construction, so every figure the header carries is non-zero.
const SUBJECT: &str = "struct F { a: Word, b: Word }\n\
                       private data st { latest: F, count: Word = 7 }\n\
                       fn main(t: Word) -> Word {\n\
                           st.latest = F { a: t, b: t + 1 };\n\
                           st.count = st.count + 1;\n\
                           st.latest.a + st.count\n\
                       }\n";

/// Generate the object and header for `SUBJECT`, through the same path a user
/// runs, and return the header text.
///
/// **Each test generates its own**, in its own directory. A first version had one
/// test generate the header and the other read it, which passes under a single
/// process and **silently becomes a no-op** under a runner that isolates tests —
/// exactly the shape of a check that cannot fail.
fn generated_header(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!("kel-host-contract-{tag}"));
    let _ = std::fs::create_dir_all(&dir);
    let kel = dir.join("subject.kel");
    std::fs::write(&kel, SUBJECT).expect("write the subject");

    let emit = std::process::Command::new(env!("CARGO"))
        .args([
            "run",
            "--quiet",
            "--example",
            "emit_object",
            "--",
            &kel.to_string_lossy(),
            &dir.to_string_lossy(),
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run emit_object");
    assert!(
        emit.status.success(),
        "emit_object failed: {}",
        String::from_utf8_lossy(&emit.stderr)
    );
    std::fs::read_to_string(dir.join("policy.h")).expect("the generated header")
}

fn define_of(header: &str, name: &str) -> Option<u64> {
    header.lines().find_map(|l| {
        let rest = l.strip_prefix(&format!("#define {name} "))?;
        rest.split_whitespace().next()?.parse().ok()
    })
}

#[test]
fn the_header_states_every_buffer_and_its_figures_are_the_published_ones() {
    let header = generated_header("figures");
    let m = common::build(SUBJECT);

    let want_shared = keleusma::vm::shared_data_bytes_for(&m) as u64;
    let want_private = (keleusma::vm::required_persistent_capacity_for(&m)
        + region::persistent_supplement_bytes(&m) as usize) as u64;
    let want_region = u64::from(region::host_arena_supplement_bytes(&m));

    // **NON-VACUITY FIRST.** Every figure must be non-zero for this subject, or a
    // header printing zeros would satisfy the comparisons below.
    assert!(
        want_private > 0 && want_region > 0,
        "the subject must exercise all three buffers: private={want_private} \
         region={want_region}"
    );

    assert_eq!(
        define_of(&header, "KEL_SHARED_BYTES"),
        Some(want_shared),
        "the header's shared figure is not the published one"
    );
    assert_eq!(
        define_of(&header, "KEL_PRIVATE_BYTES"),
        Some(want_private),
        "the header's private figure is not `required_persistent_capacity_for` \
         plus the backend's supplement"
    );
    assert_eq!(
        define_of(&header, "KEL_REGION_BYTES"),
        Some(want_region),
        "the header's composite-region figure is not the transitive one a call \
         site's disjoint block requires"
    );
}

#[test]
fn the_header_carries_the_initial_private_image_byte_for_byte() {
    let header = generated_header("image");
    let m = common::build(SUBJECT);
    let image = region::private_init_image(&m).expect("placeable");
    assert!(
        image.iter().any(|b| *b != 0),
        "the subject declares `= 7`, so its image must have content or this test \
         compares two empty things"
    );

    assert_eq!(
        define_of(&header, "KEL_PRIVATE_INIT_BYTES"),
        Some(image.len() as u64),
        "the header's image length disagrees with the published image"
    );

    let start = header
        .find("KEL_PRIVATE_INIT[] = {")
        .expect("the header carries the image array");
    let body = &header[start..];
    let end = body.find("};").expect("a terminated array");
    let bytes: Vec<u8> = body[..end]
        .split("0x")
        .skip(1)
        .filter_map(|t| u8::from_str_radix(&t[..2], 16).ok())
        .collect();
    assert_eq!(
        bytes, image,
        "the header's image bytes are not the published image"
    );
}
