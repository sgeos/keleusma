//! A signed header over a shorter declared total is refused, not indexed.
//!
//! `verify_module_signature` sliced the artifact to its declared total and then
//! read the signature metadata, the signature and the trailer at offsets fixed by
//! the declared header length. A total shorter than that header made those reads
//! index past the slice, so an untrusted artifact carrying the signature flag
//! panicked the host from `Vm::load_signed_bytes`, before any key was consulted.
//! Found on 2026-10-25 while the self-hosted signature framing (Order 5) was being
//! checked against the reference. The record is
//! `docs/decisions/SIGNATURE_VERIFY_BOUNDS.md`.
#![cfg(all(feature = "compile", feature = "signatures"))]

use keleusma::Arena;
use keleusma::bytecode::{LoadError, Module};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError};
use keleusma::wire_format::{
    FLAG_REQUIRES_SIGNATURE, module_to_signed_wire_bytes, verify_module_signature,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn compile(source: &str) -> Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap(),
    )
    .unwrap()
}

fn signer() -> ed25519_dalek::SigningKey {
    ed25519_dalek::SigningKey::from_bytes(&[61; 32])
}

/// A well-formed signed artifact whose declared total is replaced by `total`.
/// The buffer itself keeps its full length, so only the declaration is short.
fn with_total(signed: &[u8], total: u32) -> Vec<u8> {
    let mut bytes = signed.to_vec();
    bytes[8..12].copy_from_slice(&total.to_le_bytes());
    bytes
}

fn header_length(bytes: &[u8]) -> usize {
    u16::from_le_bytes([bytes[6], bytes[7]]) as usize
}

#[test]
fn every_total_below_the_signed_header_and_trailer_is_refused_without_a_panic() {
    let signed =
        module_to_signed_wire_bytes(&compile("fn main() -> Word { 7 }"), &signer()).unwrap();
    let keys = [signer().verifying_key()];
    let floor = header_length(&signed) + 4;
    assert_eq!(floor, 140, "the signed Ed25519 header is 136 bytes");
    let mut refused = 0;
    for total in 0..floor as u32 {
        let bytes = with_total(&signed, total);
        let verdict = catch_unwind(AssertUnwindSafe(|| verify_module_signature(&bytes, &keys)));
        match verdict {
            Ok(Err(LoadError::Truncated)) => refused += 1,
            other => panic!("total {total}: {other:?}"),
        }
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let loaded = catch_unwind(AssertUnwindSafe(|| {
            Vm::load_signed_bytes(&bytes, &arena, &keys).map(|_| ())
        }));
        assert!(
            matches!(loaded, Ok(Err(VmError::LoadError(_)))),
            "total {total}: {loaded:?}"
        );
    }
    assert_eq!(refused, floor, "every short total reached the verifier");
}

/// Control. The refusal is attached to the short declaration, so the unmodified
/// artifact still verifies and loads, and a wrong key is still a signature
/// failure rather than a truncation.
#[test]
fn the_well_formed_artifact_still_verifies_and_a_wrong_key_is_still_a_signature_failure() {
    let signed =
        module_to_signed_wire_bytes(&compile("fn main() -> Word { 7 }"), &signer()).unwrap();
    assert_ne!(signed[15] & FLAG_REQUIRES_SIGNATURE, 0);
    let keys = [signer().verifying_key()];
    verify_module_signature(&signed, &keys).unwrap();
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    Vm::load_signed_bytes(&signed, &arena, &keys).unwrap();
    let other = [ed25519_dalek::SigningKey::from_bytes(&[62; 32]).verifying_key()];
    assert!(matches!(
        verify_module_signature(&signed, &other),
        Err(LoadError::InvalidSignature)
    ));
    let exact = with_total(&signed, signed.len() as u32);
    verify_module_signature(&exact, &keys).unwrap();
}

/// The public metadata parser has a documented precondition, that the header
/// lies inside the buffer. A caller that breaks it now gets a refusal.
#[test]
fn the_metadata_parser_refuses_a_buffer_shorter_than_the_header() {
    let signed =
        module_to_signed_wire_bytes(&compile("fn main() -> Word { 7 }"), &signer()).unwrap();
    for len in [0, 64, 71, 72, 135] {
        let verdict = catch_unwind(|| {
            keleusma::wire_format::parse_signature_metadata(&signed[..len], 136).map(|_| ())
        });
        assert!(
            matches!(verdict, Ok(Err(LoadError::Truncated))),
            "length {len}: {verdict:?}"
        );
    }
    assert!(
        keleusma::wire_format::parse_signature_metadata(&signed, 136)
            .unwrap()
            .is_some()
    );
}

#[cfg(feature = "encryption")]
#[test]
fn an_encrypted_header_over_a_short_total_is_refused_before_decryption() {
    let recipient = [63; 32];
    let public = keleusma::encryption::public_key_from_private(&recipient);
    let sealed = keleusma::wire_format::module_to_encrypted_signed_wire_bytes(
        &compile("fn main() -> Word { 7 }"),
        &signer(),
        &public,
        &[64; 32],
    )
    .unwrap();
    let keys = [signer().verifying_key()];
    let floor = header_length(&sealed) + 4;
    assert_eq!(floor, 228, "the encrypted header is 224 bytes");
    for total in 0..floor as u32 {
        let bytes = with_total(&sealed, total);
        let verdict = catch_unwind(AssertUnwindSafe(|| {
            keleusma::wire_format::decrypt_encrypted_signed_to_signed_bytes(
                &bytes, &keys, &recipient,
            )
        }));
        assert!(
            matches!(verdict, Ok(Err(LoadError::Truncated))),
            "total {total}: {verdict:?}"
        );
    }
    keleusma::wire_format::decrypt_encrypted_signed_to_signed_bytes(&sealed, &keys, &recipient)
        .unwrap();
}
