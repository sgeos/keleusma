//! Authenticated encryption layer for compiled Keleusma modules.
//!
//! Implements the V0.2.1 hybrid asymmetric key wrapping described in
//! `tmp/encrypted_signed_modules.md`. The compiler generates an
//! ephemeral X25519 keypair per module, performs Diffie-Hellman
//! against the destination runtime's public key, derives an
//! AES-256-GCM key and nonce through HKDF-SHA-256, and encrypts the
//! body with authenticated encryption. The Ed25519 signature in the
//! parent wire-format layer covers the encrypted body, so an
//! adversary cannot strip the encryption and substitute cleartext
//! bytecode while preserving signature validity.
//!
//! Per-recipient asymmetric keys give compromise containment. A
//! captured runtime reveals only that runtime's private key.
//! Artefacts intended for other runtimes remain confidential because
//! they were encrypted against different public keys. The compiler
//! holds no decryption keys, only the public encryption keys per
//! enrolled recipient.
//!
//! This module is feature-gated on `encryption`. See the crate-level
//! Cargo.toml for the dependency list and the feature-flag wiring.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

/// Scheme identifier in the wire-format encryption metadata block.
/// Value 1 selects X25519 + HKDF-SHA-256 + AES-256-GCM. Other values
/// are reserved for future schemes (post-quantum, ECDSA-P-384, etc.).
pub const ENCRYPTION_SCHEME_X25519_AES256GCM: u8 = 1;

/// Size in bytes of the AES-256 symmetric key derived from the
/// X25519 shared secret. AES-256 is the symmetric primitive.
pub const AES_KEY_LEN: usize = 32;

/// Size in bytes of the AES-GCM nonce. 96 bits is the standard
/// AES-GCM nonce length and what `aes-gcm` expects.
pub const AES_GCM_NONCE_LEN: usize = 12;

/// Size in bytes of the AES-GCM authentication tag. The tag is
/// appended to the ciphertext.
pub const AES_GCM_TAG_LEN: usize = 16;

/// Size in bytes of an X25519 public key.
pub const X25519_PUBLIC_KEY_LEN: usize = 32;

/// Size in bytes of an X25519 private key seed.
pub const X25519_PRIVATE_KEY_LEN: usize = 32;

/// Size in bytes of the SHA-256 fingerprint used as recipient_key_id.
pub const RECIPIENT_KEY_ID_LEN: usize = 32;

/// HKDF context string for deriving the AES-256 symmetric key from
/// the X25519 shared secret. Versioned so future schemes can run
/// HKDF over the same shared secret without colliding.
const HKDF_INFO_KEY: &[u8] = b"keleusma-v1-aes256-gcm-key";

/// HKDF context string for deriving the AES-GCM nonce from the
/// X25519 shared secret. Distinct from the key info to enforce
/// domain separation.
const HKDF_INFO_NONCE: &[u8] = b"keleusma-v1-aes256-gcm-nonce";

/// Errors returned by the encryption layer. Maps the underlying
/// crypto-crate errors to a Keleusma-shaped variant set so callers
/// can pattern-match without depending on the crypto crates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncryptionError {
    /// HKDF expansion produced an unexpected length. Should not
    /// happen in practice; included for completeness.
    KeyDerivationFailed(String),
    /// AES-GCM encryption rejected the plaintext. Internal failure;
    /// the `aes-gcm` crate documents this as nearly impossible in
    /// the operations this module performs.
    EncryptFailed(String),
    /// AES-GCM decryption rejected the ciphertext. The tag did not
    /// authenticate. Either the ciphertext was tampered with, the
    /// wrong decryption key was used, or the wrong nonce was
    /// derived. All three indicate the artefact is not for this
    /// recipient or has been corrupted.
    DecryptFailed(String),
    /// The recipient_key_id field in the encryption metadata does
    /// not match the SHA-256 fingerprint of the local public key.
    /// The artefact was encrypted for a different recipient.
    WrongRecipient,
}

impl core::fmt::Display for EncryptionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::KeyDerivationFailed(s) => write!(f, "HKDF key derivation failed: {}", s),
            Self::EncryptFailed(s) => write!(f, "AES-GCM encryption failed: {}", s),
            Self::DecryptFailed(s) => write!(f, "AES-GCM decryption failed: {}", s),
            Self::WrongRecipient => write!(f, "encrypted artefact is not for this recipient"),
        }
    }
}

/// Encryption metadata block carried in the wire-format header for
/// encrypted modules. Distinct from the `signature_metadata` block
/// to keep the wire format readable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptionMetadata {
    /// Selects the cryptographic suite. Value 1 is
    /// X25519+AES-256-GCM+HKDF-SHA-256. Future schemes claim other
    /// values without breaking ABI.
    pub scheme_id: u8,
    /// Compiler-generated ephemeral X25519 public key. The
    /// recipient uses this together with its own private key to
    /// reconstruct the shared secret. The ephemeral private key
    /// is discarded by the compiler after encryption.
    pub ephemeral_public_key: [u8; X25519_PUBLIC_KEY_LEN],
    /// SHA-256 fingerprint of the destination runtime's public
    /// key. The runtime checks that this matches the SHA-256 of
    /// its own public key before attempting decryption. Catches
    /// artefacts intended for a different recipient before
    /// expensive cryptographic operations.
    pub recipient_key_id: [u8; RECIPIENT_KEY_ID_LEN],
    /// AES-GCM nonce. Derived from HKDF, included in the artefact
    /// so the recipient can reproduce the key agreement without
    /// recomputing the HKDF input.
    pub aes_gcm_nonce: [u8; AES_GCM_NONCE_LEN],
}

impl EncryptionMetadata {
    /// Serialize the encryption metadata block to the 88-byte form
    /// the wire format expects. Layout per `tmp/encrypted_signed_modules.md`:
    ///
    /// - offset 0:     scheme_id (1 byte)
    /// - offset 1:     reserved (1 byte, zero)
    /// - offset 2-3:   metadata length (u16 LE; always 88 for v1 scheme)
    /// - offset 4-35:  ephemeral public key (32 bytes)
    /// - offset 36-67: recipient_key_id (32 bytes)
    /// - offset 68-79: AES-GCM nonce (12 bytes)
    /// - offset 80-87: reserved (8 bytes, zero, for 8-byte alignment)
    pub fn to_bytes(&self) -> [u8; 88] {
        let mut buf = [0u8; 88];
        buf[0] = self.scheme_id;
        buf[1] = 0; // reserved
        buf[2..4].copy_from_slice(&88u16.to_le_bytes());
        buf[4..36].copy_from_slice(&self.ephemeral_public_key);
        buf[36..68].copy_from_slice(&self.recipient_key_id);
        buf[68..80].copy_from_slice(&self.aes_gcm_nonce);
        // bytes 80..88 are zero-initialised already
        buf
    }

    /// Parse the 88-byte encryption metadata block from the wire
    /// format. Returns `None` if the buffer is too short, the
    /// scheme is unknown, or the encoded length is wrong.
    pub fn from_bytes(buf: &[u8]) -> Option<Self> {
        if buf.len() < 88 {
            return None;
        }
        let scheme_id = buf[0];
        if scheme_id != ENCRYPTION_SCHEME_X25519_AES256GCM {
            return None;
        }
        let length = u16::from_le_bytes([buf[2], buf[3]]);
        if length != 88 {
            return None;
        }
        let mut ephemeral_public_key = [0u8; X25519_PUBLIC_KEY_LEN];
        ephemeral_public_key.copy_from_slice(&buf[4..36]);
        let mut recipient_key_id = [0u8; RECIPIENT_KEY_ID_LEN];
        recipient_key_id.copy_from_slice(&buf[36..68]);
        let mut aes_gcm_nonce = [0u8; AES_GCM_NONCE_LEN];
        aes_gcm_nonce.copy_from_slice(&buf[68..80]);
        Some(Self {
            scheme_id,
            ephemeral_public_key,
            recipient_key_id,
            aes_gcm_nonce,
        })
    }
}

/// Compute the SHA-256 fingerprint of an X25519 public key. Used
/// as the `recipient_key_id` so the runtime can detect
/// wrong-recipient artefacts before attempting decryption.
pub fn recipient_key_id(public_key: &[u8; X25519_PUBLIC_KEY_LEN]) -> [u8; RECIPIENT_KEY_ID_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(public_key);
    let digest = hasher.finalize();
    let mut out = [0u8; RECIPIENT_KEY_ID_LEN];
    out.copy_from_slice(&digest);
    out
}

/// Encrypt `plaintext` against the destination runtime's X25519
/// public key. Returns the encryption metadata block and the
/// ciphertext (which includes the 16-byte AES-GCM authentication
/// tag appended).
///
/// The compiler generates an ephemeral X25519 keypair per module,
/// performs DH against the recipient's public key, derives the AES
/// key and GCM nonce through HKDF-SHA-256, and encrypts the body.
/// The ephemeral private key is dropped after this function
/// returns; only the ephemeral public key is preserved in the
/// metadata for the recipient to reconstruct the shared secret.
pub fn encrypt_to_recipient(
    plaintext: &[u8],
    recipient_public_key: &[u8; X25519_PUBLIC_KEY_LEN],
    rng_seed: &[u8; X25519_PRIVATE_KEY_LEN],
) -> Result<(EncryptionMetadata, Vec<u8>), EncryptionError> {
    // The compiler must provide entropy for the ephemeral key. We
    // accept it as a parameter so the caller (typically the CLI)
    // can source it from the OS RNG without this module depending
    // on `rand_core`. Treats `rng_seed` as the ephemeral private
    // key material directly. The caller is responsible for ensuring
    // the seed is from a cryptographically secure source.
    let ephemeral_private = StaticSecret::from(*rng_seed);
    let ephemeral_public = PublicKey::from(&ephemeral_private);
    let recipient_pk = PublicKey::from(*recipient_public_key);

    // X25519 ECDH produces a 32-byte shared secret.
    let shared_secret = ephemeral_private.diffie_hellman(&recipient_pk);

    // Derive the AES-256 key and GCM nonce through HKDF-SHA-256
    // with distinct info strings.
    let hkdf = Hkdf::<Sha256>::new(None, shared_secret.as_bytes());
    let mut key = [0u8; AES_KEY_LEN];
    hkdf.expand(HKDF_INFO_KEY, &mut key)
        .map_err(|e| EncryptionError::KeyDerivationFailed(format!("{:?}", e)))?;
    let mut nonce = [0u8; AES_GCM_NONCE_LEN];
    hkdf.expand(HKDF_INFO_NONCE, &mut nonce)
        .map_err(|e| EncryptionError::KeyDerivationFailed(format!("{:?}", e)))?;

    // Encrypt with AES-256-GCM. The crate appends the 16-byte
    // authentication tag to the ciphertext automatically.
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| EncryptionError::EncryptFailed(format!("{:?}", e)))?;
    let ciphertext = cipher
        .encrypt(aes_gcm::Nonce::from_slice(&nonce), plaintext)
        .map_err(|e| EncryptionError::EncryptFailed(format!("{:?}", e)))?;

    let metadata = EncryptionMetadata {
        scheme_id: ENCRYPTION_SCHEME_X25519_AES256GCM,
        ephemeral_public_key: ephemeral_public.to_bytes(),
        recipient_key_id: recipient_key_id(recipient_public_key),
        aes_gcm_nonce: nonce,
    };

    Ok((metadata, ciphertext))
}

/// Decrypt an encrypted body using the runtime's X25519 private
/// key. Verifies that the artefact was intended for this recipient
/// before attempting decryption.
///
/// Returns `Err(WrongRecipient)` if the `recipient_key_id` in the
/// metadata does not match the SHA-256 of the local public key.
/// Returns `Err(DecryptFailed)` if the AES-GCM authentication tag
/// fails to authenticate (either tampered ciphertext or wrong key).
pub fn decrypt_from_metadata(
    metadata: &EncryptionMetadata,
    ciphertext: &[u8],
    local_private_key: &[u8; X25519_PRIVATE_KEY_LEN],
) -> Result<Vec<u8>, EncryptionError> {
    // Construct the local keypair and check the recipient_key_id
    // before doing any cryptographic work. Catches misaddressed
    // artefacts early.
    let local_secret = StaticSecret::from(*local_private_key);
    let local_public = PublicKey::from(&local_secret);
    let local_fingerprint = recipient_key_id(local_public.as_bytes());
    if local_fingerprint != metadata.recipient_key_id {
        return Err(EncryptionError::WrongRecipient);
    }

    // Reconstruct the shared secret from the ephemeral public key
    // in the metadata and the local private key. By X25519's
    // Diffie-Hellman property this produces the same shared secret
    // the compiler derived.
    let ephemeral_pk = PublicKey::from(metadata.ephemeral_public_key);
    let shared_secret = local_secret.diffie_hellman(&ephemeral_pk);

    // Derive the AES-256 key with the same HKDF context the
    // encryption side used. The nonce is supplied directly from
    // metadata; HKDF derivation of the nonce on the encryption side
    // is mirrored here only as a consistency cross-check.
    let hkdf = Hkdf::<Sha256>::new(None, shared_secret.as_bytes());
    let mut key = [0u8; AES_KEY_LEN];
    hkdf.expand(HKDF_INFO_KEY, &mut key)
        .map_err(|e| EncryptionError::KeyDerivationFailed(format!("{:?}", e)))?;

    // Verify that the metadata's nonce matches what the HKDF would
    // have derived. This is a defence-in-depth check; a mismatch
    // here would also surface as a decryption failure below, but
    // catching it explicitly produces a clearer diagnostic.
    let mut expected_nonce = [0u8; AES_GCM_NONCE_LEN];
    hkdf.expand(HKDF_INFO_NONCE, &mut expected_nonce)
        .map_err(|e| EncryptionError::KeyDerivationFailed(format!("{:?}", e)))?;
    if expected_nonce != metadata.aes_gcm_nonce {
        return Err(EncryptionError::DecryptFailed(String::from(
            "nonce in metadata does not match HKDF derivation",
        )));
    }

    // Decrypt with AES-256-GCM. The crate verifies the
    // authentication tag and returns an error if the ciphertext
    // has been tampered with or the wrong key is in use.
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| EncryptionError::DecryptFailed(format!("{:?}", e)))?;
    let plaintext = cipher
        .decrypt(
            aes_gcm::Nonce::from_slice(&metadata.aes_gcm_nonce),
            ciphertext,
        )
        .map_err(|e| EncryptionError::DecryptFailed(format!("{:?}", e)))?;

    Ok(plaintext)
}

/// Derive the X25519 public key from a private seed. Used by the
/// CLI key-management code to write a `.pub` file alongside a
/// private seed file. Pure function; no entropy consumed.
pub fn public_key_from_private(
    private_key: &[u8; X25519_PRIVATE_KEY_LEN],
) -> [u8; X25519_PUBLIC_KEY_LEN] {
    let secret = StaticSecret::from(*private_key);
    let public = PublicKey::from(&secret);
    public.to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a deterministic test private key from a label. Not
    /// cryptographically secure; intentionally so, because tests
    /// need reproducible values. Production keys use the OS RNG.
    fn test_key(label: u8) -> [u8; 32] {
        let mut k = [label; 32];
        // Clamp per X25519 (high bit clear, low 3 bits clear, bit 254 set).
        k[0] &= 248;
        k[31] &= 127;
        k[31] |= 64;
        k
    }

    #[test]
    fn round_trip_smallest() {
        let recipient_sk = test_key(0x11);
        let recipient_pk = public_key_from_private(&recipient_sk);
        let ephemeral_seed = test_key(0x22);
        let plaintext = b"hello, world";

        let (metadata, ciphertext) =
            encrypt_to_recipient(plaintext, &recipient_pk, &ephemeral_seed).expect("encrypt");

        // Ciphertext should be plaintext length plus 16-byte tag.
        assert_eq!(
            ciphertext.len(),
            plaintext.len() + AES_GCM_TAG_LEN,
            "ciphertext length should equal plaintext length plus tag"
        );

        let decrypted =
            decrypt_from_metadata(&metadata, &ciphertext, &recipient_sk).expect("decrypt");

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn round_trip_large_payload() {
        let recipient_sk = test_key(0x33);
        let recipient_pk = public_key_from_private(&recipient_sk);
        let ephemeral_seed = test_key(0x44);
        // 16 KiB plaintext.
        let plaintext: Vec<u8> = (0..16384).map(|i| (i % 256) as u8).collect();

        let (metadata, ciphertext) =
            encrypt_to_recipient(&plaintext, &recipient_pk, &ephemeral_seed).expect("encrypt");

        assert_eq!(ciphertext.len(), plaintext.len() + AES_GCM_TAG_LEN);

        let decrypted =
            decrypt_from_metadata(&metadata, &ciphertext, &recipient_sk).expect("decrypt");

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn wrong_recipient_rejected() {
        let alice_sk = test_key(0x55);
        let alice_pk = public_key_from_private(&alice_sk);
        let bob_sk = test_key(0x66);
        let ephemeral_seed = test_key(0x77);

        // Encrypt to Alice.
        let (metadata, ciphertext) =
            encrypt_to_recipient(b"for alice only", &alice_pk, &ephemeral_seed).expect("encrypt");

        // Try to decrypt as Bob. Should fail at the
        // recipient_key_id check before any crypto runs.
        let result = decrypt_from_metadata(&metadata, &ciphertext, &bob_sk);
        assert_eq!(result, Err(EncryptionError::WrongRecipient));
    }

    #[test]
    fn tampered_ciphertext_rejected() {
        let recipient_sk = test_key(0x88);
        let recipient_pk = public_key_from_private(&recipient_sk);
        let ephemeral_seed = test_key(0x99);
        let plaintext = b"sensitive payload";

        let (metadata, mut ciphertext) =
            encrypt_to_recipient(plaintext, &recipient_pk, &ephemeral_seed).expect("encrypt");

        // Flip a bit in the ciphertext body.
        ciphertext[3] ^= 0x01;

        let result = decrypt_from_metadata(&metadata, &ciphertext, &recipient_sk);
        assert!(
            matches!(result, Err(EncryptionError::DecryptFailed(_))),
            "expected decryption failure on tampered ciphertext, got {:?}",
            result
        );
    }

    #[test]
    fn tampered_tag_rejected() {
        let recipient_sk = test_key(0xaa);
        let recipient_pk = public_key_from_private(&recipient_sk);
        let ephemeral_seed = test_key(0xbb);
        let plaintext = b"sensitive payload";

        let (metadata, mut ciphertext) =
            encrypt_to_recipient(plaintext, &recipient_pk, &ephemeral_seed).expect("encrypt");

        // Flip a bit in the appended 16-byte tag.
        let tag_offset = ciphertext.len() - 8;
        ciphertext[tag_offset] ^= 0x01;

        let result = decrypt_from_metadata(&metadata, &ciphertext, &recipient_sk);
        assert!(matches!(result, Err(EncryptionError::DecryptFailed(_))));
    }

    #[test]
    fn metadata_round_trip() {
        let metadata = EncryptionMetadata {
            scheme_id: ENCRYPTION_SCHEME_X25519_AES256GCM,
            ephemeral_public_key: [0x42; 32],
            recipient_key_id: [0x77; 32],
            aes_gcm_nonce: [0xa5; 12],
        };
        let bytes = metadata.to_bytes();
        assert_eq!(bytes.len(), 88);
        let parsed = EncryptionMetadata::from_bytes(&bytes).expect("parse");
        assert_eq!(parsed, metadata);
    }

    #[test]
    fn metadata_rejects_unknown_scheme() {
        let mut bytes = [0u8; 88];
        bytes[0] = 99; // unknown scheme
        bytes[2..4].copy_from_slice(&88u16.to_le_bytes());
        assert!(EncryptionMetadata::from_bytes(&bytes).is_none());
    }

    #[test]
    fn metadata_rejects_short_buffer() {
        let bytes = [0u8; 40];
        assert!(EncryptionMetadata::from_bytes(&bytes).is_none());
    }

    #[test]
    fn recipient_key_id_is_sha256() {
        // Known SHA-256 of the all-zeros 32-byte input. Sanity
        // check against a published test vector.
        let zeros = [0u8; 32];
        let fingerprint = recipient_key_id(&zeros);
        // SHA-256 of 32 zero bytes, hex:
        // 66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925
        let expected: [u8; 32] = [
            0x66, 0x68, 0x7a, 0xad, 0xf8, 0x62, 0xbd, 0x77, 0x6c, 0x8f, 0xc1, 0x8b, 0x8e, 0x9f,
            0x8e, 0x20, 0x08, 0x97, 0x14, 0x85, 0x6e, 0xe2, 0x33, 0xb3, 0x90, 0x2a, 0x59, 0x1d,
            0x0d, 0x5f, 0x29, 0x25,
        ];
        assert_eq!(fingerprint, expected);
    }

    #[test]
    fn different_ephemeral_keys_produce_different_ciphertexts() {
        let recipient_sk = test_key(0xcc);
        let recipient_pk = public_key_from_private(&recipient_sk);
        let ephemeral_a = test_key(0xdd);
        let ephemeral_b = test_key(0xee);
        let plaintext = b"identical plaintext";

        let (_, ciphertext_a) =
            encrypt_to_recipient(plaintext, &recipient_pk, &ephemeral_a).expect("encrypt");
        let (_, ciphertext_b) =
            encrypt_to_recipient(plaintext, &recipient_pk, &ephemeral_b).expect("encrypt");

        assert_ne!(
            ciphertext_a, ciphertext_b,
            "different ephemeral keys must produce different ciphertexts"
        );
    }
}
