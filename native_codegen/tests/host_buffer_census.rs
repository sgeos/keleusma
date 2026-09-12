//! **EVERY HARNESS THAT BUILDS HOST BUFFERS, AND WHERE ITS SIZES COME FROM.**
//!
//! # Why this exists, and it is not a hypothetical
//!
//! A native entry takes three pointers. A harness that allocates them must size
//! them from the contract this backend publishes —
//! `required_persistent_capacity_for` plus `persistent_supplement_bytes` for the
//! private region, and `host_arena_supplement_bytes` for the composite region.
//!
//! **Three harnesses in this package sized a region by a literal or by the slot
//! count.** In one day:
//!
//! | harness | how it failed | how it was found |
//! |---|---|---|
//! | the corpus differential | one word per private slot | its canary fired when the composite pool landed |
//! | the two-argument differential | `vec![0u64; 8]` | **SIGSEGV in the gate** — green alone, green under narrow |
//! | the stage differential | one word per private slot | this sweep, before it could fail |
//!
//! **A literal-sized buffer fails by corrupting its neighbour**, which is not a
//! stable observable: it depends on the allocator and on how many tests run
//! beside it. That is why one of the three appeared only in the gate.
//!
//! # What this census refuses
//!
//! A NEW harness that calls a three-pointer entry without deriving its private
//! size from the contract. It cannot verify that an existing harness's arithmetic
//! is right — that is what the canaries do at run time — only that every harness
//! asks the published functions instead of guessing.
//!
//! # What it cannot see
//!
//! - A harness that derives the figure and then uses it wrongly.
//! - A harness that allocates buffers in a helper file this matcher does not
//!   associate with the caller. `tests/common/mod.rs` is such a helper, and it is
//!   listed explicitly for that reason.
//! - Anything about the SHARED segment, whose size is the host's by contract and
//!   which several harnesses legitimately size to a fixed scratch value.

/// Test files that construct the private region for a native call.
///
/// Recorded so a new one announces itself rather than joining silently.
/// ⚠ **THIS LIST WAS FIRST WRITTEN BY HAND AND WAS WRONG.** It named seven
/// files; the matcher found fourteen, three of which asked no contract figure at
/// all. **A hand sweep over a population is the thing this census replaces**, and
/// its first draft demonstrated why in the same increment. The SECOND draft was
/// also wrong — it omitted `rogue_dungen_differential.rs` and included
/// `private_init_image.rs`, which drives through the shared helper and allocates
/// nothing itself. Both corrections came from the matcher, not from another look.
const BUFFER_HARNESSES: &[&str] = &[
    "common/mod.rs",
    "composite_stream_sequence.rs",
    "composite_yield_witness.rs",
    "corpus_differential.rs",
    "declared_float_width.rs",
    "delegated_suspension.rs",
    "general_stream_sequence.rs",
    "module_differential.rs",
    "module_source_differential.rs",
    "native_calls.rs",
    "partial_operation_census.rs",
    "reset_region_retention.rs",
    "rogue_ai_differential.rs",
    "rogue_dungen_differential.rs",
    "stage_differential.rs",
];

/// How a harness may establish its private region size.
///
/// `module_source_differential.rs` is the one member that legitimately uses
/// neither: its programs declare no data at all, it says so, and it still passes
/// a valid guarded pointer.
const CONTRACT_SOURCES: &[&str] = &[
    "required_persistent_capacity_for",
    "install_private_init",
    "persistent_supplement_bytes",
];

fn harness_files() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir("tests").expect("tests directory") {
        let path = entry.expect("entry").path();
        let text = if path.is_dir() {
            let inner = path.join("mod.rs");
            match std::fs::read_to_string(&inner) {
                Ok(t) => t,
                Err(_) => continue,
            }
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            std::fs::read_to_string(&path).unwrap_or_default()
        } else {
            continue;
        };
        // The three-pointer entry signature is what makes a file a host harness.
        // **The census excludes ITSELF.** It carries the signature string in its
        // own matcher, so it would otherwise report as a harness that builds
        // buffers, which it does not.
        let is_self = path
            .file_name()
            .map(|f| f == "host_buffer_census.rs")
            .unwrap_or(false);
        if !is_self && text.contains("*mut u8, *mut u8, *mut u8") {
            let name = if path.is_dir() {
                "common/mod.rs".to_string()
            } else {
                path.file_name().unwrap().to_string_lossy().into_owned()
            };
            out.push((name, text));
        }
    }
    out.sort();
    out
}

#[test]
fn every_host_harness_sizes_the_private_region_from_the_contract() {
    let files = harness_files();
    println!("\n================ HARNESSES THAT BUILD HOST BUFFERS");
    for (name, text) in &files {
        let source = CONTRACT_SOURCES
            .iter()
            .find(|s| text.contains(**s))
            .copied()
            .unwrap_or("NONE");
        println!("  {name:38} <- {source}");
    }
    println!("================\n");

    // **NON-VACUITY.** A matcher finding nothing would pass for ever, and this
    // file's whole subject is a population.
    assert!(
        files.len() >= 5,
        "only {} host harnesses found; the matcher has gone stale",
        files.len()
    );

    let found: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        found, BUFFER_HARNESSES,
        "the set of harnesses that build host buffers changed. A new one must \
         derive its private region size from the published contract — three in \
         this package sized it by a literal or by the slot count, and one of those \
         reached the gate as a SIGSEGV."
    );

    for (name, text) in &files {
        // The one legitimate exception, and it states its own reason: its
        // programs declare no data, so there is no contract figure to ask for.
        if name == "module_source_differential.rs" {
            assert!(
                text.contains("no private slots"),
                "{name} neither derives the contract figure nor says why it does \
                 not need to"
            );
            continue;
        }
        assert!(
            CONTRACT_SOURCES.iter().any(|s| text.contains(*s)),
            "{name} builds host buffers without asking the published contract for \
             the private region's size. A literal fails by corrupting its \
             neighbour, which is not a stable observable."
        );
    }
}
