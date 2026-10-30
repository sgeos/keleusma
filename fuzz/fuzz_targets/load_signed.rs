//! Fuzz target `load_signed`. The body is `harness::load_signed`, which the corpus
//! replay test in the parent package also calls.

#![no_main]

// Each target uses one harness function, so the others are unused in this binary.
#[allow(dead_code)]
#[path = "../harness.rs"]
mod harness;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = harness::load_signed(data);
});
