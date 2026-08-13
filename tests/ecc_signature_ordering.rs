//! The order in which a parity scrub and a signature check compose.
//!
//! # The question
//!
//! A parity plane repairs a flipped bit. A signature refuses a changed byte.
//! Both are ordinary, and putting them on one artifact forces a choice that
//! neither feature announces:
//!
//! ```text
//!   (A) verify, then scrub     the bytes that run were authenticated BEFORE
//!                              the scrub rewrote them
//!   (B) scrub, then verify     the bytes that run are what the check covered
//! ```
//!
//! **They are indistinguishable on every undamaged artifact**, which is why the
//! question does not surface in ordinary testing. They differ only on damaged
//! input, which is the case the plane exists for.
//!
//! # Why this file exists rather than an argument
//!
//! The analysis was written down before it was run, and its own stated weakest
//! link was that **the signature half was read from the implementation and never
//! executed**. Nothing had signed a module, damaged it, scrubbed it and observed
//! the verification outcome. This does that.
//!
//! # What is established here
//!
//! 1. A single-bit fault in a signed artifact **fails verification**. So under
//!    order (A) the module is refused and the plane is dead weight for signed
//!    modules, which is the availability cost of getting the order wrong.
//! 2. Scrubbing that artifact **reproduces the original bytes exactly** and the
//!    signature then verifies. Order (B) recovers the module.
//! 3. A three-bit fault is **mis-repaired**, and the signature **still refuses
//!    it**. The composition is fail-closed, and the signature rather than the
//!    corrector is what makes it so.
//!
//! Result 3 is the one that matters. The corrector reports success on inputs
//! where it produced the wrong answer, so a design that trusted the scrub would
//! run wrong bytes. The signature is the authority; the scrub is a proposal.
#![cfg(all(feature = "compile", feature = "signatures"))]

use ed25519_dalek::SigningKey;
use keleusma::wire_format::{module_to_signed_wire_bytes_with_ecc, verify_module_signature};
use keleusma_wire::{WireView, WordStatus};

const SRC: &str = "signed fn main() -> Word { 21 + 21 }";

fn signed_with_planes() -> (Vec<u8>, ed25519_dalek::VerifyingKey) {
    let signer = SigningKey::from_bytes(&[91u8; 32]);
    let verifying = signer.verifying_key();
    let module = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(SRC).expect("lex")).expect("parse"),
    )
    .expect("compile");
    let bytes = module_to_signed_wire_bytes_with_ecc(&module, &signer).expect("sign");
    (bytes, verifying)
}

/// The auxiliary-body span of a framed artifact, from the framing header.
fn aux_span(bytes: &[u8]) -> (usize, usize) {
    let start = u32::from_le_bytes([bytes[48], bytes[49], bytes[50], bytes[51]]) as usize;
    let len = u32::from_le_bytes([bytes[52], bytes[53], bytes[54], bytes[55]]) as usize;
    (start, len)
}

/// A scrub over the whole artifact: repair every protected region in place.
///
/// This is the operation whose composition with verification is in question, so
/// the test performs it rather than reasoning about it. It returns the repaired
/// artifact and the counts, and it does NOT decide whether the result is
/// trustworthy, which is exactly the point.
fn scrub(bytes: &[u8]) -> (Vec<u8>, usize, usize) {
    let (aux_start, aux_len) = aux_span(bytes);
    let mut out = bytes.to_vec();
    let (mut corrected, mut uncorrectable) = (0usize, 0usize);
    let view = WireView::parse(&bytes[aux_start..aux_start + aux_len]).expect("aux parses");
    for i in 0..view.region_count() {
        let Some(r) = view.region_at(i) else { continue };
        if r.is_ecc_plane() || !r.has_ecc() {
            continue;
        }
        let (Some(base), Ok(data)) = (r.byte_offset(), view.region_bytes(&r)) else {
            continue;
        };
        let Some(plane) = view.ecc_for(&r) else {
            continue;
        };
        for w in 0..data.len().div_ceil(8) {
            match plane.word(data, w) {
                Some(WordStatus::Corrected(v)) => {
                    corrected += 1;
                    let at = aux_start + base + w * 8;
                    let end = core::cmp::min(at + 8, out.len());
                    out[at..end].copy_from_slice(&v.to_le_bytes()[..end - at]);
                }
                Some(WordStatus::Uncorrectable) => uncorrectable += 1,
                _ => {}
            }
        }
    }
    (out, corrected, uncorrectable)
}

/// Byte offset, within the whole artifact, of a protected non-empty region.
fn protected_offset(bytes: &[u8]) -> usize {
    let (aux_start, aux_len) = aux_span(bytes);
    let view = WireView::parse(&bytes[aux_start..aux_start + aux_len]).expect("aux parses");
    for i in 0..view.region_count() {
        let Some(r) = view.region_at(i) else { continue };
        if r.is_ecc_plane() || !r.has_ecc() {
            continue;
        }
        if let (Some(b), Ok(p)) = (r.byte_offset(), view.region_bytes(&r))
            && !p.is_empty()
        {
            return aux_start + b;
        }
    }
    panic!("no protected non-empty region in a signed artifact built with ECC");
}

#[test]
fn a_signed_artifact_built_with_ecc_carries_planes_and_verifies() {
    let (bytes, key) = signed_with_planes();

    // CONTROL ONE. The artifact must verify as produced, or every later
    // assertion measures a signature that never verified.
    verify_module_signature(&bytes, &[key]).expect("CONTROL: the signed artifact must verify");

    // CONTROL TWO. Planes must actually be present, or the scrub below is a
    // no-op and the whole file passes while exercising nothing.
    let (aux_start, aux_len) = aux_span(&bytes);
    let view = WireView::parse(&bytes[aux_start..aux_start + aux_len]).expect("aux parses");
    let planes = (0..view.region_count())
        .filter_map(|i| view.region_at(i))
        .filter(|r| r.is_ecc_plane())
        .count();
    assert!(
        planes > 0,
        "CONTROL: the signed artifact carries no parity plane, so nothing here is tested"
    );

    // CONTROL THREE. A clean scrub must be the identity. Order (B)'s soundness
    // rests on it: if scrubbing an honest artifact changed a byte, every signed
    // module would fail to load.
    let (rescrubbed, c, u) = scrub(&bytes);
    assert_eq!((c, u), (0, 0), "CONTROL: a clean artifact reported faults");
    assert_eq!(
        rescrubbed, bytes,
        "CONTROL: scrubbing an undamaged artifact changed it, which would break every load"
    );
}

/// Result 1 and 2. A single fault fails verification, and scrubbing recovers it.
#[test]
fn a_single_fault_is_refused_by_the_signature_and_recovered_by_a_scrub() {
    let (bytes, key) = signed_with_planes();
    let at = protected_offset(&bytes);

    for bit in 0..8u32 {
        let mut damaged = bytes.clone();
        damaged[at] ^= 1 << bit;

        // ORDER (A): verify first. The module is REFUSED, and the parity plane
        // never gets to do the job it was added for.
        assert!(
            verify_module_signature(&damaged, &[key]).is_err(),
            "bit {bit}: a damaged artifact verified, so the signature does not cover this byte"
        );

        // ORDER (B): scrub first, then verify. The module is RECOVERED.
        let (repaired, corrected, uncorrectable) = scrub(&damaged);
        assert_eq!(
            corrected, 1,
            "bit {bit}: expected exactly one corrected word"
        );
        assert_eq!(
            uncorrectable, 0,
            "bit {bit}: a single fault must be correctable"
        );
        assert_eq!(
            repaired, bytes,
            "bit {bit}: the scrub did not reproduce the original bytes exactly, so no signature \
             computed over the original could ever verify against a repaired artifact"
        );
        verify_module_signature(&repaired, &[key]).unwrap_or_else(|e| {
            panic!("bit {bit}: the repaired artifact failed verification: {e:?}")
        });
    }
}

/// Result 3, and the one that decides the design.
///
/// Three faults in one word are outside what a distance-four code promises. The
/// corrector reports a successful repair and produces the WRONG bytes. The
/// signature is what stops those bytes from running.
#[test]
fn a_mis_repaired_artifact_is_still_refused_which_is_why_the_signature_must_run_last() {
    let (bytes, key) = signed_with_planes();
    let at = protected_offset(&bytes);

    let mut any_miscorrected = false;
    for (a, b, c) in [(0u32, 1u32, 2u32), (0, 1, 7), (1, 3, 5), (0, 2, 4)] {
        let mut damaged = bytes.clone();
        damaged[at] ^= (1 << a) | (1 << b) | (1 << c);

        let (repaired, corrected, uncorrectable) = scrub(&damaged);
        if corrected > 0 && repaired != bytes {
            any_miscorrected = true;
            // THE LOAD-BEARING ASSERTION. The scrub reported success and handed
            // back bytes that are not the module. Only the signature notices.
            assert!(
                verify_module_signature(&repaired, &[key]).is_err(),
                "bits {a},{b},{c}: a MIS-REPAIRED artifact verified. The scrub reported \
                 {corrected} corrections and produced bytes that differ from the original, and \
                 the signature accepted them, which would run unauthenticated code."
            );
        } else if uncorrectable > 0 {
            // Refused outright, which is also fail-closed.
            assert!(verify_module_signature(&repaired, &[key]).is_err());
        }
    }

    // MUST-FIRE. If no pattern mis-corrected, this test proved nothing about the
    // dangerous case and the fault positions need choosing again.
    assert!(
        any_miscorrected,
        "no triple-fault pattern was mis-repaired, so the assertion above never ran and this \
         test does not establish that the composition is fail-closed"
    );
}
