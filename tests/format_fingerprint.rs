//! The format fingerprint refuses a module whose flat layout is not this
//! build's, and refuses a header flag bit this build does not define.
//!
//! Why these exist: `BYTECODE_VERSION` is held at 2 across releases by
//! operator policy, so the version check admits every release that declares 2.
//! Without these checks a module from a different release is accepted and read
//! under the wrong meaning, which is the failure the version number would
//! otherwise have caught.
//!
//! **What is NOT tested here, because it is not true:** that this detects a
//! release which reinterprets a field both readers already read. It does not.
//! Only a version bump does.

//! **Feature guard.** `lexer`, `parser` and `compiler` are gated behind the
//! `compile` feature, so this file cannot exist in a no-default-features build.
//! The gate runs that configuration and it is not one of the three feature sets
//! a routine check covers.
#![cfg(feature = "compile")]

use keleusma::bytecode::Module;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;

fn compiled_bytes(src: &str) -> Vec<u8> {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    module.to_bytes().expect("encode")
}

/// A byte-level edit to the fingerprint is refused.
///
/// **What this does NOT prove.** It does not exercise the fingerprint check.
/// The artifact carries two CRC layers and the edit is caught by a checksum
/// long before the header is read -- measured, not assumed; the companion test
/// below reports the responsible mechanism. The fingerprint check's own
/// rejection path is covered by unit tests in `wire_schema`, which build a
/// well-formed artifact carrying a foreign fingerprint, because that is what a
/// genuinely foreign artifact looks like.
///
/// It is kept because refusing a corrupted artifact at SOME layer is worth
/// pinning, not because it says anything about the fingerprint.
#[test]
fn a_byte_edit_to_the_fingerprint_is_refused_at_some_layer() {
    let mut bytes = compiled_bytes("fn main() -> Word { 1 }");
    let live = keleusma::value_layout::format_fingerprint();
    let needle = live.to_le_bytes();

    // Locate the fingerprint by value. Asserting it occurs EXACTLY once is the
    // point: a test that patched the first of several matches could be
    // corrupting an unrelated field and still see a rejection, which would
    // make it pass for the wrong reason.
    let hits: Vec<usize> = bytes
        .windows(4)
        .enumerate()
        .filter(|(_, w)| *w == needle)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected exactly one occurrence of the fingerprint in the artifact, found {}",
        hits.len()
    );

    // A neighbouring value, so the change is minimal and unmistakably the
    // fingerprint rather than a structural corruption the container would
    // reject on its own.
    bytes[hits[0]..hits[0] + 4].copy_from_slice(&live.wrapping_add(1).to_le_bytes());

    let err = Module::from_bytes(&bytes).err();
    // The container carries a CRC, so a byte-level edit may be caught there
    // first. Either refusal is acceptable; silent acceptance is not.
    assert!(
        err.is_some(),
        "a module carrying a foreign format fingerprint was accepted"
    );
}

#[test]
fn the_live_fingerprint_is_present_in_a_real_artifact() {
    // Guards against the check passing vacuously because nothing writes the
    // field. If the emitter regressed to a zero, the rejection test above
    // could still pass while the mechanism did nothing.
    let bytes = compiled_bytes("fn main() -> Word { 1 }");
    let needle = keleusma::value_layout::format_fingerprint().to_le_bytes();
    assert!(
        bytes.windows(4).any(|w| w == needle),
        "the emitted artifact does not carry the live format fingerprint"
    );
}

#[test]
fn the_refusal_is_reported_and_named() {
    // Which mechanism refuses matters. The container carries a CRC, so a
    // byte-level edit can be caught there instead of by the fingerprint check,
    // and a test that only asserts "some error" would pass while the
    // fingerprint check never ran at all.
    let mut bytes = compiled_bytes("fn main() -> Word { 1 }");
    let live = keleusma::value_layout::format_fingerprint();
    let at = bytes
        .windows(4)
        .position(|w| w == live.to_le_bytes())
        .expect("fingerprint present");
    bytes[at..at + 4].copy_from_slice(&live.wrapping_add(1).to_le_bytes());
    let err = Module::from_bytes(&bytes).expect_err("must be refused");
    // Recorded rather than asserted to a single variant: the point is to make
    // the responsible mechanism visible in the test output, so a future change
    // that moves the refusal from one layer to another is noticed.
    std::eprintln!("refused by: {err:?}");
}
