# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: V0.2.0 signed-modules feature (R42) implemented end-to-end on the `feat-signed-modules` branch. Wire-format header extended through the existing `header_length: u16` field to carry an Ed25519 signature; a new `signed` surface keyword on the entry function emits `FLAG_REQUIRES_SIGNATURE`; the runtime carries a per-Vm trust matrix consulted at `load_signed_bytes` and `replace_module_from_bytes`. The migration matrix for future schemes (ECDSA, ML-DSA, LMS) lives in `secret/SIGNATURE_SCHEME_MIGRATION.md`. Ready for merge to `main`.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Add signed compiled modules to V0.2.0 before publishing. | New `signatures` cargo feature (off by default) brings `ed25519-dalek 2`. Wire format extends the framing header through the existing `header_length: u16` field: bytes 64..72 hold the signature metadata block (scheme_id, signature_length, reserved bytes) and bytes 72.. hold the raw signature payload. Ed25519 (scheme_id = 1) is the only V0.2.0 scheme; unknown scheme_ids reject at framing. The `signed` modifier on the entry function declaration (`signed fn main`, `signed yield main`, `signed loop main`) emits `FLAG_REQUIRES_SIGNATURE = 0x02` in the flags byte; the modifier is admissible only on the entry function (a `signed` modifier on a helper rejects at compile time). The signature message convention is the full framed buffer with signature payload bytes and CRC trailer zeroed; both signer and verifier reconstruct that view before the cryptographic operation. New `Vm::load_signed_bytes(bytes, arena, &keys)` performs verification + load. New `Vm::register_verifying_key` / `clear_verifying_keys` / `verifying_keys_len` manage the per-Vm trust matrix. New `Vm::replace_module_from_bytes(bytes, initial_data)` performs hot-swap with signature verification against the inherited trust matrix. `Vm::new` rejects modules carrying `FLAG_REQUIRES_SIGNATURE` directly (the signature info is lost when the Module is decoded) and directs callers to `load_signed_bytes` or hot-swap. CLI extensions: `keleusma compile --signing-key seed.bin` signs the output; `keleusma run --verifying-key key.pub` (repeatable) populates the runtime trust matrix. Migration plan for future schemes documented in `secret/SIGNATURE_SCHEME_MIGRATION.md`. R42 added to `docs/decisions/RESOLVED.md`; `docs/architecture/WIRE_FORMAT.md` updated with the extension layout. |

## Verification matrix

```bash
cargo test --workspace                                                          # 956 tests across 16 suites, all green
cargo test --workspace --features signatures                                    # 963 across 16, all green
cargo clippy --tests --workspace --all-features -- -D warnings                  # clean
cargo fmt --all -- --check                                                      # idempotent
keleusma compile signed.kel --signing-key seed.bin -o signed.kel.bin            # signed bytecode produced
keleusma run signed.kel.bin --verifying-key correct.pub                         # loads + executes
keleusma run signed.kel.bin --verifying-key wrong.pub                           # rejects: signature did not verify
keleusma run signed.kel.bin                                                     # rejects: empty trust matrix
(cd examples/rtos && cargo run --release --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf --no-default-features \
    --features stm32n6570dk-platform)                                          # boots; unsigned-modules path unchanged
```

## Open concerns

None.

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed as not-applicable (closures retired in Phase 4) |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | Closed (Phases 1, 2, 3, 3.5, Consolidation B, 4, 5, 6, 7a, 7b, 7c, 8 complete) |

## Intended Next Step

V0.2.0-isa branch is ready for merge to `main`. The natural next step is one of:

- Merge the `V0.2.0-isa` branch into `main` and tag the release.
- Manual `cargo publish` of the V0.2.0 crate (the publication step is operator-owned; the agent does not run `cargo publish`).
- A B15 follow-on: remove `Type::Unknown` entirely now that the V0.2.0 ISA work is closed.
- Operator selection of a different directive.
