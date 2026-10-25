# Order 5 completion audit

> **Navigation** [Framing plan](./ORDER5_CRYPTO_FRAMING_PLAN.md) | [Order 4 audit](./ORDER4_COMPLETION_AUDIT.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md)

## Scope and disposition

This audit records the state of Order 5, the cryptographic framing written in
Keleusma with host-native primitives, on 2026-10-25. Its scope is the gate of
[the framing plan](./ORDER5_CRYPTO_FRAMING_PLAN.md). **That gate is this line's
reading of the roadmap gate and has not been ruled on by the operator.**

**Disposition.** Approach 1 of Order 5 is complete for that first-pass scope on
2026-10-25, on the evidence below, pending the operator's acceptance of the
scope. Approach 2, the primitives in Keleusma, is not started and is not part of
the disposition. `BYTECODE_VERSION` remains 2, the instruction set remains 66
opcodes, the wire format is unchanged, no capacity was raised and nothing was
published.

## Gate evidence

All tests named below are in `tests/crypto_framing.rs` unless another file is
named. The corpus is the 15 example scripts, `frame.kel`, the Order 4
interpreter and three inline programs, 20 modules in all, the largest signed
artifact being 39,428 bytes.

| Gate condition | Evidence | Disposition |
|---|---|---|
| The framing program is admitted by the Rust verifier | `the_framing_program_is_admitted_by_the_rust_verifier` constructs its virtual machine with `Vm::new` | Met |
| Signed artifacts, with and without error-correcting planes, are byte-identical | `signed_artifacts_are_byte_identical_to_the_reference`, 80 comparisons over two signers | Met |
| Encrypted artifacts and plaintext views are byte-identical | `encrypted_artifacts_and_their_plaintext_views_are_byte_identical_to_the_reference`, 40 artifacts and 40 views over two seeds | Met |
| Verification verdicts agree, every class reached | `signature_verdicts_agree_with_the_reference`, 1504 runs, 188 mutations of each of two modules under four key sets, reaching acceptance and all ten refusal classes. `multi_step_verdicts_agree_with_the_reference` adds 30 runs on a 36,948-byte artifact that span several steps, a 200,000-byte shebang line and a set of 300 keys | Met |
| Decryption verdicts agree, every class reached | `decryption_verdicts_agree_with_the_reference`, 1212 runs, 202 cases under three key sets and two local keys, reaching acceptance and every refusal class from truncation through authentication failure | Met |
| Ordering is held by the program | `the_signature_is_checked_before_any_decryption`, `the_protocol_order_is_held_by_the_program`, `only_a_completed_signing_enables_encryption`, `a_completed_encryption_cannot_be_followed_by_another`, and `framing_fields_out_of_range_are_refused_before_any_write` | Met |
| The pipeline with Keleusma sections reproduces the reference | `the_self_hosted_sections_and_framing_reproduce_the_reference`, under `self-host`, 20 modules with sections from the Order 1 wire stages | Met |
| The shipping command line uses it | `keleusma compile --compiler self-hosted` signs and encrypts through it and refuses a divergence. `self_hosted_signing_and_encryption_produce_artifacts_that_run` in `keleusma-cli/tests/secure_deploy.rs` runs both outputs | Met |

**Non-vacuity.** The verdict harnesses assert a floor on every class they
claim to reach. A mutation of `frame.kel` that set the encryption readiness
after every operation was caught by
`a_completed_encryption_cannot_be_followed_by_another`.

## Independent review

A separate agent reviewed `frame.kel` and its driver against the reference
without being given these conclusions. It found no divergence, panic,
unbounded allocation or non-termination, and it agreed with the reference on
its own probes, including multi-step refusals. It raised four points, and each
was acted on before this audit.

1. The ephemeral seed was reusable within one machine, which a host could have
   driven into AES-GCM nonce reuse. The public interface could not reach it.
   The seed is now consumed by its first use.
2. The documentation claimed the program could not write buffers a primitive
   owns, while the plaintext and ciphertext buffers are writable. The wording
   was corrected to the actual policy.
3. The readiness to encrypt was untested after a successful operation, and
   multi-step refusals, the shebang edge cases and the invalid-input and
   seal-failure refusals were unasserted. Tests now cover each.
4. A primitive failure other than the three known ones maps to missing key
   material. It is unreachable because HKDF does not fail for these lengths,
   and it is recorded rather than changed.

## Defect found and repaired

`verify_module_signature` panicked on a signed header over a short declared
total, reachable from signed loading. Repaired in pull request 524. See
[SIGNATURE_VERIFY_BOUNDS.md](./SIGNATURE_VERIFY_BOUNDS.md).

## Residual limits

- **The primitives' cost is outside the program's bound.** Signing,
  verification, sealing and opening run in the host in time linear in the
  message. They are registered with the default native attestation, which does
  not describe that cost. The program's own loops are statically bounded, and
  a numeric per-step bound is not computed in this pass, because the hosted
  runtime's transitive bound does not model native calls.
- **The plaintext body crosses into the virtual machine**, as it does through
  the reference encoder. No key material does.
- **The program is trusted not to alter a primitive's output**, because the
  plaintext and ciphertext buffers are writable to it on other paths.
- **Two reference behaviours are mirrored rather than corrected**, the
  decryption ciphertext running to the buffer's end rather than the declared
  total, and the unchecked reserved bytes of the encryption metadata.
- **Error-correcting planes** are produced on the reference section path only.
  The Keleusma section path does not produce them.
- **An encrypted total above `u32::MAX`** is refused by the program and
  truncated by the reference. It needs an artifact over 4 GiB.
- **Approach 2 is not started**, and the host primitives remain the trusted
  implementation.

## Verification receipts

The local matrix ran on 2026-10-25 over the uncommitted tree on
`feat/v02-order5-crypto-framing`, based on `74058ab4`, after the last edit.
Every step exited 0.

| Command | Result |
|---|---|
| `cargo fmt --all --check` | Clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | Clean |
| `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` | Clean |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | Clean |
| `cargo nextest run --cargo-profile release --workspace` | 3542 passed, 2 skipped |
| `cargo test --release --workspace --doc` | Passed |
| `cargo nextest run --cargo-profile release -p keleusma --features self-host` | 3264 passed, 2 skipped |
| `cargo nextest run --cargo-profile release -p keleusma --no-default-features` | 338 passed |
| `cargo nextest run --cargo-profile release -p keleusma --features signatures` | 2515 passed |
| `cargo nextest run --cargo-profile release -p keleusma --features signatures,shell` | 2532 passed |

**Coverage limits.** The `encryption` feature is not one of the feature
sets continuous integration runs alone. All 13 tests of
`tests/crypto_framing.rs` and all 7 of `keleusma-cli/tests/secure_deploy.rs`
ran in the workspace population, where the command-line crate enables
`encryption` and `self-host`, and the log names each one. The `signatures`
populations run the framing tests that need no encryption, and the isolated
`self-host` population runs none, because it does not enable `signatures`. The `--no-default-features` build emits five dead-code warnings
in `tests/op_tag_tables.rs`, a file this change does not touch. Remote
continuous integration is recorded below once it has run.
