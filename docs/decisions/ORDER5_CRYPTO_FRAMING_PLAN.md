# Order 5 cryptographic framing plan

> **Navigation** [Order 5 audit](./ORDER5_COMPLETION_AUDIT.md) | [Order 4 plan](./ORDER4_HOSTED_RUNTIME_PLAN.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md)

## Scope and its provenance

Order 5 of the V0.2.X roadmap is cryptography orchestration, Workstream E. Its
gate in the roadmap reads "Signing and encryption framing self-hosted with
host-native primitives; primitive re-implementation is a separate, review-gated
opt-in." Open decision 1 of the roadmap defaults to approach 1 first and to
approach 2 as a later milestone. This plan follows that default. Approach 2,
re-implementing SHA-256, Ed25519, X25519 and AES-GCM in Keleusma, is outside
Order 5.

**The concrete gate below is this line's interpretation of the roadmap gate.
The operator has not ruled on it.** The session that built Order 5 had a goal
to complete Order 5 and no gate ruling, so the gate was derived from the
roadmap text and from the oracle discipline the roadmap states for the whole
line, that each self-hosted component is diff-tested for byte-identical or
verdict-identical agreement with the Rust reference. The operator's acceptance
of this scope is recorded as an open question in the reverse prompt.

## What is framing and what is a primitive

Workstream E lists the framing as what bytes are signed, the signature and
encryption metadata layout, the recipient-key fingerprint flow and the
verify-before-decrypt ordering. This plan draws the line at computation on
secrets.

| Kind | Decided in Keleusma | Host primitive |
|---|---|---|
| Signed artifact | Header layout, signature metadata block, section placement and padding, the message the signature covers, CRC trailer | Ed25519 signing |
| Verification | Shebang skipping, every framing check and its verdict in the reference's order, the message, the signature's position, the order in which keys are tried | Ed25519 strict verification of one key |
| Encrypted artifact | Which bytes are sealed, encrypted header layout, section offset shift, encryption metadata block layout, re-signing and CRC | X25519 agreement, HKDF-SHA-256, AES-256-GCM sealing, the SHA-256 fingerprint of the recipient key |
| Decryption | Its pre-checks, verification before any decryption, the metadata checks, the body length check, the fingerprint comparison before any agreement, the plaintext view and its CRC | X25519 agreement with the contributory check, HKDF-SHA-256 with the nonce check, AES-256-GCM opening, the SHA-256 fingerprint of the local key |

The nonce check stays with the primitive because the expected nonce is an HKDF
output. Exposing it to the program would move derived key material across the
boundary for a comparison that the authentication tag repeats.

## Design

**The program.** `src/crypto_frame/frame.kel` is a `loop main` driven by
commands. 10 begins a signing, 11 a verification, 12 an encryption of the
artifact the last signing produced, 13 a decryption, and 2 runs one step of at
most 4096 units. A unit is one byte moved, one key tried or one fixed-size
header operation. Every loop in the program has a static bound, so the Rust
verifier admits it. The phase order is private state of the program, so a host
cannot skip verification before decryption or encrypt an artifact the program
did not sign.

**The driver.** `src/crypto_frame/mod.rs`, `keleusma::crypto_frame`, behind
`signatures`, `compile` and `verify`. Encryption is additionally behind
`encryption`, and the path whose sections come from the Order 1 wire stages is
behind `self-host`. The driver owns ten byte buffers and every key, registers
ten natives over them, and runs the step loop under a budget derived from the
buffer sizes.

**Trust boundary.** No key enters the program. The signing key, the trusted
verifying keys, the ephemeral seed and the local private key stay in the
driver, and a native names a verifying key only by its index. The natives
refuse an index outside a buffer, a byte outside `[0, 255]` and a write to the
input, the sealed parameters or either fingerprint, and such a refusal is a
driver error, never a verdict. The message, signature, plaintext and ciphertext
buffers stay writable, because the program fills each for a primitive on one
path and reads a primitive's output from it on another, so the program is
trusted not to alter a primitive's output. The ephemeral seed is consumed by
its first use, so one machine cannot seal twice under one key and nonce.
The module body does cross the boundary, in both directions, as it does
through the reference encoder.

**Sections are input.** As in the unsigned framing stage of Order 1, section
production is a separate obligation. The reference signer was refactored to
take its sections from a public `wire_format::signing_sections`, so the
reference and the program frame the same bytes. Under `self-host`,
`crypto_frame::signing_sections_via_kel` produces them through the Order 1 wire
stages with no reference encoder.

**Shipping integration.** `keleusma compile --compiler self-hosted` with
`--signing-key`, and with `--encryption-key`, now frames through the program
and refuses its own output unless it is byte-identical to the reference
signer's over the same module, with the same ephemeral seed for encryption.

## The gate

1. The framing program is a Keleusma program the Rust verifier admits.
2. Over a corpus of compiled modules, every signed artifact, with and without
   error-correcting planes, and every encrypted artifact equals the
   reference's byte for byte, and so does the plaintext view decryption
   returns.
3. Over a mutation corpus aimed at every framing check, verification and
   decryption give the reference's verdict class on every case, under several
   key sets, with every refusal class reached.
4. The ordering properties are held by the program. Verification precedes
   decryption, the fingerprint precedes agreement, and the protocol refuses an
   out-of-order command.
5. The whole pipeline with sections from the Order 1 wire stages reproduces the
   reference.
6. The shipping command line uses it.

## A defect found on the way

Checking verdicts against the reference found that
`wire_format::verify_module_signature` panicked on a signed header over a short
declared total, reachable from `Vm::load_signed_bytes`. It was repaired first,
in pull request 524. See [SIGNATURE_VERIFY_BOUNDS.md](./SIGNATURE_VERIFY_BOUNDS.md).

## Mirrored reference behaviour

The program mirrors two reference behaviours that are not obviously intended,
so that the oracle stays exact. Each is recorded rather than changed.

1. Decryption took the ciphertext up to the end of the supplied buffer, not up
   to the declared total, so bytes appended after the total reached the cipher
   and failed authentication. **Repaired on 2026-10-28** by the operator's
   first-hand ruling to decrypt up to the declared total. Both the reference and
   the program now end the ciphertext there, and
   `trailing_bytes_after_the_declared_total_do_not_reach_the_cipher` pins it.
2. The encryption metadata parser checks the scheme and the length and does not
   check the reserved byte or the eight reserved bytes.
