# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: V0.1-M1 hardened with CRC-32 algebraic self-inclusion trailer.
**Status**: Complete.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 333 tests pass workspace-wide. 288 keleusma unit, 17 keleusma integration, 22 keleusma-arena unit, 6 keleusma-arena doctests. Two new tests added for the CRC integrity work.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

The bytecode wire format gains a four-byte little-endian CRC-32 trailer. The CRC uses the standard IEEE 802.3 reflected polynomial `0xEDB88320` with init `0xFFFFFFFF`, refin and refout true, and xor-out `0xFFFFFFFF`. The implementation is a hand-rolled bit-by-bit loop in fifteen lines of `bytecode.rs`. No table is allocated at compile time or runtime.

The verification path uses the algebraic self-inclusion residue of this CRC parameterization. Computing the CRC over a byte sequence followed by the little-endian encoding of that sequence's CRC yields a fixed residue constant `0x2144DF1C`. The verifier runs the CRC once over the entire byte slice including the trailer and checks for the residue in a single linear pass. The trailer is conceptually part of the checksummed range without requiring zero-fill or position-aware special casing during verification. This satisfies the "include itself in the checksum range" property requested by the user.

The validation order in `Module::from_bytes` is now truncation, magic, CRC residue, version, and body decode. The CRC check precedes the version check because a corrupted byte in the version field would otherwise be reported as `UnsupportedVersion` rather than the more accurate `BadChecksum`.

## Changes Made

### Source

- **`src/bytecode.rs`**: New `FOOTER_LEN`, `CRC32_POLY`, `CRC32_RESIDUE` constants. New `crc32` function with `pub(crate)` visibility for testability. New `LoadError::BadChecksum` variant with display string. `Module::to_bytes` now appends the CRC as a four-byte little-endian trailer. `Module::from_bytes` now runs the residue check before the version check and adjusts the body slice to exclude the trailer. The minimum admissible bytecode size grows to ten bytes.
- **`src/vm.rs`**: Existing tests updated. `bytecode_rejects_bad_magic` and `bytecode_load_via_vm_propagates_load_error` extended to ten bytes so they reach the magic check rather than failing the truncation check. `bytecode_rejects_unsupported_version` rewritten to compile a real module, patch the version field, recompute the CRC trailer, and verify that the version rejection path triggers independently of the checksum path. New `bytecode_rejects_bad_checksum` test flips a body byte and asserts `BadChecksum`. New `bytecode_residue_property_holds` test confirms the CRC implementation against the reference value `crc32("123456789") = 0xCBF43926` and confirms that appending the little-endian CRC produces the residue `0x2144DF1C`.

### Knowledge Graph

- **`docs/decisions/RESOLVED.md`**: R39 updated. Wire format description now includes the trailer, the CRC parameters, and the residue property. The `BadChecksum` variant is listed alongside the other LoadError variants. The validation order is documented.
- **`docs/architecture/EXECUTION_MODEL.md`**: Bytecode Loading section updated. Wire format description includes the trailer, the CRC parameters, and the residue property.
- **`docs/process/TASKLOG.md`**: V0.1-M1-T5 row added. New history row added.
- **`docs/process/REVERSE_PROMPT.md`**: This file.

## Trade-offs and Properties

The single-pass residue check is the cheapest and cleanest verification approach. The cost is one CRC computation over the entire byte slice. There is no separate read of the trailer field, no zero-fill substitution, and no special handling of the trailer position during the CRC loop.

The CRC-32 reference parameters were chosen for compatibility with the broadly recognized standard. Tools that compute CRC-32 over arbitrary byte sequences will produce values consistent with the verifier. This aids debugging when a bytecode file is suspected of corruption.

The hand-rolled bit-by-bit implementation is intentionally compact. A table-based variant would be roughly four times faster but adds 256 entries of constant data. For bytecode-sized inputs in the kilobyte to megabyte range, the bit-by-bit form completes well within typical load time budgets and avoids the table memory cost. A future iteration may add the table if profiling motivates it.

The residue constant `0x2144DF1C` is documented as a magic number with a citation to the algorithm parameters. Verifying tools or third-party readers of the bytecode can compute the residue independently from the polynomial and verify against the same value.

## Unaddressed Concerns

1. **CRC-32 is a non-cryptographic checksum.** Bit-level corruption is detected with high probability. Adversarial tampering is not. The bytecode integrity guarantee covers accidental corruption only. A cryptographic signature is admissible as a future addition for hosts that need stronger integrity.

2. **The residue constant is a hardcoded magic number.** The constant `0x2144DF1C` follows from the CRC parameters and could be derived at build time through a `const fn` evaluation. For now the constant is documented in source with a citation to the IEEE 802.3 parameter set.

3. **No incremental CRC API.** The `crc32` function processes the full byte slice in one call. A streaming variant would let callers checksum data as it arrives over a network or from disk without buffering the whole input. Not needed for the current `Module::from_bytes` and `Module::to_bytes` flow because they operate on owned byte vectors. A future iteration may add a streaming form if the use case emerges.

4. **Version-rejection test depends on `pub(crate)` visibility of `crc32`.** Exposing the function at crate scope is the simplest path to test isolation. A `#[cfg(test)]` shim would also work and would keep the function strictly private. The current visibility is judged acceptable because the function is a pure utility with no surface specific to bytecode framing.

## Intended Next Step

Three paths.

A. V0.1-M2 implementing P10 path B. Lifetime-parameterize Module. Eliminate String fields in favor of byte-offset references. The CRC trailer integrates naturally because it is position-independent under the residue check.

B. V0.1-M2 advancing to V0.1 candidates such as the type checker (P1), for-in over arbitrary expressions (P2), or error recovery (P3).

C. V0.1-M2 returning to P7 follow-on items, namely operand stack and `DynStr` arena migration in the keleusma runtime.

Recommend A if the precompiled-code use case is the priority. The CRC trailer makes path B more attractive because the integrity check is now cheap and position-independent. Path B's zero-copy execution against `.rodata` becomes the natural completion of the precompiled-distribution story.

Await human prompt before proceeding.

## Session Context

This session began with V0.0-M5 and V0.0-M6 already complete and the arena extracted into a workspace crate. The session resolved P8 and P9, completed three pre-publication audit and polish passes on `keleusma-arena`, published v0.1.0 to crates.io, switched the keleusma main crate to consume the registry version, completed V0.1-M1 implementing precompiled bytecode loading and trust-based verification skip, and now hardened the wire format with a CRC-32 algebraic self-inclusion trailer.
