//! Order 5. The cryptographic framing written in Keleusma, `src/crypto_frame/frame.kel`, agrees
//! with the Rust reference byte for byte on what it produces and verdict for verdict on what it
//! checks. See `docs/decisions/ORDER5_CRYPTO_FRAMING_PLAN.md`.
//!
//! The reference is `module_to_signed_wire_bytes`, its error-correcting form,
//! `verify_module_signature`, `module_to_encrypted_signed_wire_bytes` and
//! `decrypt_encrypted_signed_to_signed_bytes`. A verdict is compared as a class, the
//! [`Refusal`] code on the Keleusma side and the reference's `LoadError` mapped onto the same
//! codes by [`class_of`]. Every class each harness relies on reaching has a floor, so a
//! mutation set that stopped reaching a check fails here rather than passing with fewer cases.
#![cfg(all(feature = "compile", feature = "verify", feature = "signatures"))]

use std::collections::BTreeMap;

#[cfg(feature = "encryption")]
use ed25519_dalek::Signer;
use ed25519_dalek::{SigningKey, VerifyingKey};
use keleusma::bytecode::{LoadError, Module};
use keleusma::crypto_frame::{self as cf, FrameError, Refusal};
use keleusma::wire_format::{module_to_signed_wire_bytes, module_to_signed_wire_bytes_with_ecc};

fn compile(source: &str) -> Option<Module> {
    let tokens = keleusma::lexer::tokenize(source).ok()?;
    let program = keleusma::parser::parse(&tokens).ok()?;
    keleusma::compiler::compile(&program).ok()
}

/// The corpus. Every example script that the reference compiles, the framing program itself and
/// the Order 4 interpreter as larger modules, and three small inline programs, one of which
/// declares the `signed` entry modifier.
fn corpus() -> Vec<(String, Module)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths: Vec<_> = std::fs::read_dir(root.join("examples/scripts"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .collect();
    paths.sort();
    paths.push(root.join("src/crypto_frame/frame.kel"));
    paths.push(root.join("src/hosted/interp.kel"));
    let mut out = Vec::new();
    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap();
        if let Some(module) = compile(&source) {
            out.push((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                module,
            ));
        }
    }
    for (name, source) in [
        ("inline/seven", "fn main() -> Word { 7 }"),
        ("inline/signed", "signed fn main(x: Word) -> Word { x + 1 }"),
        (
            "inline/data",
            "data d { n: Word }\nfn main() -> Word { d.n = d.n + 3; d.n }",
        ),
    ] {
        out.push((name.into(), compile(source).expect(name)));
    }
    out
}

fn signer(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

/// The reference's refusal, on the codes `frame.kel` yields. A message that matches no class is
/// a refusal this harness does not know, and fails it.
fn class_of(e: &LoadError) -> i64 {
    let code = match e {
        LoadError::Truncated => Refusal::Truncated,
        LoadError::BadMagic => Refusal::BadMagic,
        LoadError::InvalidSignature => Refusal::InvalidSignature,
        LoadError::Codec(m) => {
            let table = [
                ("module is not signed", Refusal::NotSigned),
                ("less than the minimum", Refusal::HeaderBelowMinimum),
                ("reserved fields must be zero", Refusal::ReservedNonzero),
                ("scheme_id 0 is reserved", Refusal::SchemeZero),
                ("is not supported in this build", Refusal::UnsupportedScheme),
                ("signature_length must be", Refusal::SignatureLength),
                ("does not match expected", Refusal::HeaderLengthMismatch),
                ("without FLAG_ENCRYPTED", Refusal::NotEncrypted),
                (
                    "encrypted modules must be signed",
                    Refusal::EncryptedUnsigned,
                ),
                ("metadata malformed", Refusal::EncryptionMetadata),
                ("not for this recipient", Refusal::WrongRecipient),
                ("low-order", Refusal::NonContributory),
                ("nonce in metadata", Refusal::NonceMismatch),
                ("AES-GCM decryption failed", Refusal::AuthenticationFailed),
            ];
            table
                .iter()
                .find(|(needle, _)| m.contains(needle))
                .unwrap_or_else(|| panic!("unclassified reference refusal {m}"))
                .1
        }
        other => panic!("unclassified reference refusal {other:?}"),
    };
    code as i64
}

fn stage_class<T>(r: &Result<T, FrameError>) -> i64 {
    match r {
        Ok(_) => 0,
        Err(FrameError::Refused(r)) => *r as i64,
        Err(FrameError::Stage(detail)) => panic!("stage defect: {detail}"),
    }
}

fn reference_class<T>(r: &Result<T, LoadError>) -> i64 {
    match r {
        Ok(_) => 0,
        Err(e) => class_of(e),
    }
}

fn put16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// Re-sign `bytes` under the reference's convention with `key`, over the declared total, so a
/// mutation reaches the checks after verification. The CRC is recomputed.
#[cfg(feature = "encryption")]
fn resign(bytes: &mut [u8], key: &SigningKey) {
    let total = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let mut message = bytes[..total].to_vec();
    message[72..136].fill(0);
    message[total - 4..].fill(0);
    let signature = key.sign(&message).to_bytes();
    bytes[72..136].copy_from_slice(&signature);
    let crc = keleusma_wire::crc32(&bytes[..total - 4]);
    bytes[total - 4..total].copy_from_slice(&crc.to_le_bytes());
}

/// Mutations of a signed artifact, each named, aimed at every framing check the verifier makes.
fn mutations(signed: &[u8]) -> Vec<(String, Vec<u8>)> {
    let len = signed.len();
    let mut out: Vec<(String, Vec<u8>)> = vec![("identity".into(), signed.to_vec())];
    let mut with = |name: String, f: &dyn Fn(&mut Vec<u8>)| {
        let mut b = signed.to_vec();
        f(&mut b);
        out.push((name, b));
    };
    for at in (0..140).chain([len / 2, len - 5, len - 4, len - 1]) {
        with(format!("flip {at}"), &|b| b[at] ^= 0x01);
    }
    for total in [0u32, 3, 4, 67, 68, 71, 100, 135, 139, 140] {
        with(format!("total {total}"), &|b| put32(b, 8, total));
    }
    for total in [len as u32 - 1, len as u32 + 1] {
        with(format!("total {total}"), &|b| put32(b, 8, total));
    }
    for header in [0u16, 63, 64, 65, 70, 71, 72, 128, 137, 224, 0xFFFF] {
        with(format!("header {header}"), &|b| put16(b, 6, header));
    }
    for scheme in [0u8, 2, 255] {
        with(format!("scheme {scheme}"), &|b| b[64] = scheme);
    }
    with("reserved byte".into(), &|b| b[65] = 1);
    with("reserved word".into(), &|b| b[71] = 1);
    for length in [0u16, 63, 65] {
        with(format!("signature length {length}"), &|b| {
            put16(b, 66, length)
        });
    }
    with("encrypted flag without metadata".into(), &|b| b[15] |= 0x04);
    with("signature flag cleared".into(), &|b| b[15] &= !0x02);
    with("magic".into(), &|b| b[0] = b'X');
    for keep in [0, 10, 67, 68, 139, len - 1] {
        with(format!("buffer {keep}"), &|b| b.truncate(keep));
    }
    with("trailing bytes".into(), &|b| {
        b.extend_from_slice(&[0xA5; 9])
    });
    with("shebang".into(), &|b| {
        let mut p = b"#!/usr/bin/env keleusma\n".to_vec();
        p.extend_from_slice(b);
        *b = p;
    });
    with("shebang without newline".into(), &|b| {
        b.splice(0..0, b"#!".iter().copied());
        for x in b.iter_mut() {
            if *x == b'\n' {
                *x = 0;
            }
        }
    });
    out
}

#[test]
fn the_framing_program_is_admitted_by_the_rust_verifier() {
    let module = cf::frame_module().unwrap();
    let need = keleusma::vm::required_persistent_capacity_for(&module);
    let mut arena = keleusma::Arena::with_capacity(keleusma::vm::DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).unwrap();
    let vm = keleusma::vm::Vm::new(module.clone(), &arena);
    assert!(vm.is_ok(), "{:?}", vm.err());
    assert!(module.wcet_cycles > 0, "the program declares a time bound");
    assert!(module.native_names.len() >= 10, "{:?}", module.native_names);
}

#[test]
fn signed_artifacts_are_byte_identical_to_the_reference() {
    let corpus = corpus();
    assert!(corpus.len() >= 18, "corpus has {} modules", corpus.len());
    let mut compared = 0;
    let mut largest = 0;
    for (name, module) in &corpus {
        for seed in [11, 12] {
            let key = signer(seed);
            let expected = module_to_signed_wire_bytes(module, &key).unwrap();
            assert_eq!(
                cf::signed_wire_bytes(module, &key, false).unwrap(),
                expected,
                "{name}"
            );
            let expected = module_to_signed_wire_bytes_with_ecc(module, &key).unwrap();
            assert_eq!(
                cf::signed_wire_bytes(module, &key, true).unwrap(),
                expected,
                "{name} ecc"
            );
            compared += 2;
            largest = largest.max(expected.len());
        }
    }
    assert_eq!(compared, corpus.len() * 4);
    assert!(
        largest > 30_000,
        "a large artifact is in the corpus, {largest} bytes"
    );
}

#[test]
fn signature_verdicts_agree_with_the_reference() {
    let right = signer(21);
    let wrong = signer(22);
    let key_sets: Vec<(&str, Vec<VerifyingKey>)> = vec![
        ("none", vec![]),
        ("right", vec![right.verifying_key()]),
        ("wrong", vec![wrong.verifying_key()]),
        (
            "right last",
            vec![
                wrong.verifying_key(),
                signer(23).verifying_key(),
                right.verifying_key(),
            ],
        ),
    ];
    let mut reached: BTreeMap<i64, usize> = BTreeMap::new();
    let mut runs = 0;
    // Two small modules carry every mutation. The verdicts do not depend on the body beyond
    // its length, which the mutations reach at both ends.
    for (name, module) in corpus()
        .into_iter()
        .filter(|(n, _)| n.starts_with("inline/"))
        .take(2)
    {
        let signed = module_to_signed_wire_bytes(&module, &right).unwrap();
        for (mutation, bytes) in mutations(&signed) {
            for (keys_name, keys) in &key_sets {
                let reference = keleusma::wire_format::verify_module_signature(&bytes, keys);
                let stage = cf::verify_signature(&bytes, keys);
                let (r, s) = (reference_class(&reference), stage_class(&stage));
                assert_eq!(s, r, "{name}, {mutation}, keys {keys_name}");
                *reached.entry(r).or_default() += 1;
                runs += 1;
            }
        }
    }
    eprintln!("signature verdicts over {runs} runs: {reached:?}");
    for class in [0, -1, -2, -3, -4, -5, -6, -7, -8, -9, -10] {
        assert!(
            reached.get(&class).copied().unwrap_or(0) > 0,
            "class {class} unreached"
        );
    }
}

/// Control. A difference in the bytes is visible to the comparison, so equality above is not
/// produced by comparing a buffer with itself.
#[test]
fn a_different_key_gives_a_different_artifact_on_both_sides() {
    let module = compile("fn main() -> Word { 7 }").unwrap();
    let a = cf::signed_wire_bytes(&module, &signer(31), false).unwrap();
    let b = cf::signed_wire_bytes(&module, &signer(32), false).unwrap();
    assert_ne!(a, b);
    assert_eq!(
        a[..72],
        b[..72],
        "the header before the signature is key-independent"
    );
    assert!(cf::verify_signature(&a, &[signer(32).verifying_key()]).is_err());
    assert!(
        keleusma::wire_format::verify_module_signature(&a, &[signer(32).verifying_key()]).is_err()
    );
}

#[test]
fn the_protocol_order_is_held_by_the_program() {
    let out_of_order = Refusal::OutOfOrder as i64;
    // A step before any operation, encryption before a signing, an unknown command.
    assert_eq!(cf::probe_protocol(&[2]).unwrap(), [out_of_order]);
    assert_eq!(cf::probe_protocol(&[12]).unwrap(), [out_of_order]);
    assert_eq!(cf::probe_protocol(&[99]).unwrap(), [out_of_order]);
    // A signing with no key supplied reaches the primitive and is refused there, and the
    // program is then idle, so neither a step nor an encryption proceeds.
    let missing = Refusal::MissingKeyMaterial as i64;
    assert_eq!(
        cf::probe_protocol(&[10, 2, 2, 12]).unwrap(),
        [0, missing, out_of_order, out_of_order]
    );
    // A verification with no keys finishes refused, and encryption cannot follow it.
    let truncated = Refusal::Truncated as i64;
    assert_eq!(
        cf::probe_protocol(&[11, 2, 12]).unwrap(),
        [0, truncated, out_of_order]
    );
}

/// The readiness to encrypt is set only by a completed signing, and every other operation,
/// finished or refused, clears it. Found missing from the harness by the independent review.
#[test]
fn only_a_completed_signing_enables_encryption() {
    let module = compile("fn main() -> Word { 7 }").unwrap();
    let key = signer(81);
    let out_of_order = Refusal::OutOfOrder as i64;
    // Signing done, then a verification begins and is refused on the section bytes it was given.
    let r = cf::probe_after_signing(&module, &key, None, &[11, 2, 12]).unwrap();
    assert_eq!(r[0], 0);
    assert!(r[1] < 0, "{r:?}");
    assert_eq!(r[2], out_of_order, "{r:?}");
    // Signing done, then a decryption begins, and encryption after it is refused.
    let r = cf::probe_after_signing(&module, &key, None, &[13, 12]).unwrap();
    assert_eq!(r, [0, out_of_order]);
    // Signing done and no recipient held, so sealing fails, and the program is then idle.
    let seal = Refusal::SealFailed as i64;
    let r = cf::probe_after_signing(&module, &key, None, &[12, 2, 12, 2]).unwrap();
    assert_eq!(r, [0, seal, out_of_order, out_of_order]);
}

#[test]
fn framing_fields_out_of_range_are_refused_before_any_write() {
    let invalid = Refusal::InvalidInput as i64;
    let mut cases = Vec::new();
    for index in 0..13 {
        for value in [-1, if index < 4 { 256 } else { 0x1_0000_0000 }] {
            let mut fields = [0; 13];
            fields[index] = value;
            cases.push(fields);
        }
    }
    // Section lengths the empty input does not hold.
    let mut fields = [0; 13];
    fields[10] = 4;
    cases.push(fields);
    // A total past the 32-bit field.
    let mut fields = [0; 13];
    fields[12] = 0xFFFF_FFFF;
    cases.push(fields);
    for fields in cases {
        let r = cf::probe_signing_fields(fields, &[10, 2]).unwrap();
        assert_eq!(r, [invalid, Refusal::OutOfOrder as i64], "{fields:?}");
    }
    // Control. All-zero fields over empty sections frame and sign.
    assert_eq!(cf::probe_signing_fields([0; 13], &[10, 2]).unwrap(), [0, 1]);
}

/// Refusals and acceptances that span several steps, on the largest corpus module, with a key
/// set of three hundred keys whose last is the signer, and the shebang edge cases.
#[test]
fn multi_step_verdicts_agree_with_the_reference() {
    let right = signer(91);
    let (_, module) = corpus()
        .into_iter()
        .find(|(n, _)| n == "interp.kel")
        .unwrap();
    let signed = module_to_signed_wire_bytes(&module, &right).unwrap();
    assert!(signed.len() > 4 * 4096, "{} bytes", signed.len());
    let len = signed.len();
    let mut many: Vec<VerifyingKey> = (0..299u16)
        .map(|i| SigningKey::from_bytes(&[(i % 250) as u8 + 1; 32]).verifying_key())
        .filter(|k| *k != right.verifying_key())
        .collect();
    many.push(right.verifying_key());
    let key_sets = [
        vec![right.verifying_key()],
        vec![signer(92).verifying_key()],
        many,
    ];
    let mut cases: Vec<(String, Vec<u8>)> = vec![("identity".into(), signed.clone())];
    for at in [len / 2, len - 10, len - 5] {
        let mut b = signed.clone();
        b[at] ^= 1;
        cases.push((format!("flip {at}"), b));
    }
    let mut b = signed.clone();
    b.extend_from_slice(&[7; 7]);
    cases.push(("trailing".into(), b));
    let mut b = vec![b'#', b'!'];
    b.extend(std::iter::repeat_n(b'x', 200_000));
    b.push(b'\n');
    b.extend_from_slice(&signed);
    cases.push(("long shebang".into(), b));
    cases.push(("bare shebang".into(), b"#!".to_vec()));
    cases.push(("shebang line only".into(), b"#!\n".to_vec()));
    let mut b = signed.clone();
    b[15] |= 0x04;
    put16(&mut b, 6, 224);
    cases.push(("encrypted flag and header".into(), b));
    let mut runs = 0;
    for (name, bytes) in &cases {
        for keys in &key_sets {
            let r = reference_class(&keleusma::wire_format::verify_module_signature(bytes, keys));
            let s = stage_class(&cf::verify_signature(bytes, keys));
            assert_eq!(s, r, "{name}, {} keys", keys.len());
            runs += 1;
        }
    }
    assert_eq!(runs, cases.len() * 3);
}

#[cfg(feature = "encryption")]
mod encryption {
    use super::*;
    use keleusma::encryption::public_key_from_private;
    use keleusma::wire_format::{
        decrypt_encrypted_signed_to_signed_bytes, module_to_encrypted_signed_wire_bytes,
    };

    const RECIPIENT: [u8; 32] = [41; 32];

    #[test]
    fn encrypted_artifacts_and_their_plaintext_views_are_byte_identical_to_the_reference() {
        let public = public_key_from_private(&RECIPIENT);
        let key = signer(42);
        let keys = [key.verifying_key()];
        let mut compared = 0;
        for (name, module) in corpus() {
            for seed in [[43u8; 32], [44u8; 32]] {
                let expected =
                    module_to_encrypted_signed_wire_bytes(&module, &key, &public, &seed).unwrap();
                let actual =
                    cf::encrypted_signed_wire_bytes(&module, &key, &public, &seed).unwrap();
                assert_eq!(actual, expected, "{name}");
                let view =
                    decrypt_encrypted_signed_to_signed_bytes(&expected, &keys, &RECIPIENT).unwrap();
                assert_eq!(
                    cf::decrypt_to_signed_bytes(&expected, &keys, &RECIPIENT).unwrap(),
                    view,
                    "{name} view"
                );
                compared += 1;
            }
        }
        assert!(compared >= 36, "{compared}");
    }

    /// Mutations of an encrypted artifact. Those marked re-signed are signed again with the
    /// trusted key, so they pass verification and reach the checks after it.
    fn sealed_mutations(sealed: &[u8], key: &SigningKey) -> Vec<(String, Vec<u8>)> {
        let mut out = mutations(sealed);
        let mut resigned = |name: &str, f: &dyn Fn(&mut Vec<u8>)| {
            let mut b = sealed.to_vec();
            f(&mut b);
            resign(&mut b, key);
            out.push((format!("re-signed {name}"), b));
        };
        resigned("identity", &|_| {});
        resigned("metadata scheme", &|b| b[136] = 2);
        resigned("metadata length", &|b| put16(b, 138, 87));
        resigned("metadata reserved", &|b| b[137] = 9);
        resigned("recipient fingerprint", &|b| b[172] ^= 1);
        resigned("ephemeral key zero", &|b| b[140..172].fill(0));
        resigned("ephemeral key flip", &|b| b[150] ^= 1);
        resigned("nonce", &|b| b[204] ^= 1);
        resigned("ciphertext", &|b| b[230] ^= 1);
        resigned("tag", &|b| {
            let total = b.len();
            b[total - 5] ^= 1;
        });
        resigned("short body", &|b| {
            b.truncate(224 + 15);
            b.extend_from_slice(&[0; 4]);
            let total = b.len() as u32;
            put32(b, 8, total);
        });
        resigned("empty body", &|b| {
            b.truncate(224);
            b.extend_from_slice(&[0; 4]);
            put32(b, 8, 228);
        });
        resigned("encrypted flag cleared", &|b| b[15] &= !0x04);
        out
    }

    /// An encryption that completes clears the readiness, so a second seal under the same
    /// ephemeral seed cannot follow on the same machine.
    #[test]
    fn a_completed_encryption_cannot_be_followed_by_another() {
        let module = compile("fn main() -> Word { 7 }").unwrap();
        let public = public_key_from_private(&RECIPIENT);
        let r = cf::probe_after_signing(
            &module,
            &signer(93),
            Some((public, [94; 32])),
            &[12, 2, 2, 2, 2, 12],
        )
        .unwrap();
        assert_eq!(r[0], 0, "{r:?}");
        assert_eq!(r[4], 1, "the encryption finished within four steps, {r:?}");
        assert!(r[1..5].iter().all(|&v| v == 0 || v == 1), "{r:?}");
        assert_eq!(r[5], Refusal::OutOfOrder as i64, "{r:?}");
    }

    #[test]
    fn decryption_verdicts_agree_with_the_reference() {
        let public = public_key_from_private(&RECIPIENT);
        let right = signer(51);
        let key_sets: Vec<(&str, Vec<VerifyingKey>)> = vec![
            ("none", vec![]),
            ("right", vec![right.verifying_key()]),
            ("wrong", vec![signer(52).verifying_key()]),
        ];
        let locals: [(&str, [u8; 32]); 2] = [("recipient", RECIPIENT), ("other", [53; 32])];
        let signed_only =
            module_to_signed_wire_bytes(&compile("fn main() -> Word { 7 }").unwrap(), &right)
                .unwrap();
        let mut reached: BTreeMap<i64, usize> = BTreeMap::new();
        let mut runs = 0;
        let module = compile("fn main(x: Word) -> Word { x * 3 }").unwrap();
        let sealed =
            module_to_encrypted_signed_wire_bytes(&module, &right, &public, &[54; 32]).unwrap();
        let mut cases = sealed_mutations(&sealed, &right);
        cases.push(("signed only".into(), signed_only));
        for (mutation, bytes) in &cases {
            for (keys_name, keys) in &key_sets {
                for (local_name, local) in &locals {
                    let reference = decrypt_encrypted_signed_to_signed_bytes(bytes, keys, local);
                    let stage = cf::decrypt_to_signed_bytes(bytes, keys, local);
                    let (r, s) = (reference_class(&reference), stage_class(&stage));
                    assert_eq!(s, r, "{mutation}, keys {keys_name}, local {local_name}");
                    if let (Ok(a), Ok(b)) = (&reference, &stage) {
                        assert_eq!(a, b, "{mutation} view");
                    }
                    *reached.entry(r).or_default() += 1;
                    runs += 1;
                }
            }
        }
        eprintln!("decryption verdicts over {runs} runs: {reached:?}");
        for class in [0, -1, -2, -10, -11, -12, -13, -14, -15, -16, -17] {
            assert!(
                reached.get(&class).copied().unwrap_or(0) > 0,
                "class {class} unreached"
            );
        }
    }

    /// Verify before decrypt. A ciphertext that fails authentication is reported as a signature
    /// failure when it is not re-signed, because the signature covers it and is checked first,
    /// and as an authentication failure only once the signature is valid.
    #[test]
    fn the_signature_is_checked_before_any_decryption() {
        let public = public_key_from_private(&RECIPIENT);
        let key = signer(61);
        let keys = [key.verifying_key()];
        let module = compile("fn main() -> Word { 7 }").unwrap();
        let mut bytes = cf::encrypted_signed_wire_bytes(&module, &key, &public, &[62; 32]).unwrap();
        bytes[230] ^= 1;
        assert_eq!(
            cf::decrypt_to_signed_bytes(&bytes, &keys, &RECIPIENT),
            Err(FrameError::Refused(Refusal::InvalidSignature))
        );
        resign(&mut bytes, &key);
        assert_eq!(
            cf::decrypt_to_signed_bytes(&bytes, &keys, &RECIPIENT),
            Err(FrameError::Refused(Refusal::AuthenticationFailed))
        );
        // The fingerprint is compared before any agreement, so a wrong local key is refused as
        // a wrong recipient even when the ciphertext is also damaged.
        assert_eq!(
            cf::decrypt_to_signed_bytes(&bytes, &keys, &[63; 32]),
            Err(FrameError::Refused(Refusal::WrongRecipient))
        );
    }
}

/// The whole signed and encrypted pipeline with no reference encoder. The Order 1 wire stages
/// produce the sections, `frame.kel` frames them, and only the primitives are Rust.
#[cfg(all(feature = "self-host", feature = "encryption"))]
#[test]
fn the_self_hosted_sections_and_framing_reproduce_the_reference() {
    let key = signer(71);
    let public = keleusma::encryption::public_key_from_private(&[72; 32]);
    let mut compared = 0;
    for (name, module) in corpus() {
        let sections = cf::signing_sections_via_kel(&module).unwrap();
        assert_eq!(
            sections,
            keleusma::wire_format::signing_sections(&module, false).unwrap(),
            "{name} sections"
        );
        assert_eq!(
            cf::sign_sections(&module, &sections, &key).unwrap(),
            module_to_signed_wire_bytes(&module, &key).unwrap(),
            "{name} signed"
        );
        assert_eq!(
            cf::encrypt_sections(&module, &sections, &key, &public, &[73; 32]).unwrap(),
            keleusma::wire_format::module_to_encrypted_signed_wire_bytes(
                &module, &key, &public, &[73; 32]
            )
            .unwrap(),
            "{name} encrypted"
        );
        compared += 1;
    }
    assert!(compared >= 18, "{compared}");
}
