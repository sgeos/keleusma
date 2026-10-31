//! Fuzz target `front_end`. The body is `harness::front_end`, which the corpus replay
//! test in the parent package also calls.

#![no_main]

// Each target uses one harness function, so the others are unused in this binary.
#[allow(dead_code)]
#[path = "../harness.rs"]
mod harness;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = harness::front_end(data);
});
