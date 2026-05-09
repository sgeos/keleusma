# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: V0.1-M1 wire format extended with length, word size, address size, and golden-bytes test.
**Status**: Complete.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 339 tests pass workspace-wide. 294 keleusma unit, 17 keleusma integration, 22 keleusma-arena unit, 6 keleusma-arena doctests. Six new tests cover word size mismatch, address size mismatch, oversized length field, undersized length field, trailing padding admission, and golden-bytes pinning.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

The bytecode wire format header expanded from six bytes to twelve. The new fields are a thirty-two-bit total framing length in little-endian, an eight-bit word size in bits, and an eight-bit address size in bits. The version field bumped from one to two. The minimum framing size is sixteen bytes (twelve header plus four trailer).

The recorded length is authoritative. The deserializer truncates the input slice to the recorded length before any further processing. Trailing bytes after the recorded length are ignored, supporting bytecode embedded inside a larger buffer such as a flash region with padding or a multi-segment archive.

Word size and address size record the assumptions the compiler made about the host runtime when emitting the bytecode. The current Keleusma runtime is built for sixty-four-bit words and sixty-four-bit addresses. `RUNTIME_WORD_BITS` and `RUNTIME_ADDRESS_BITS` are both sixty-four. Bytecode that records a different word size or address size is rejected at load time with `WordSizeMismatch` or `AddressSizeMismatch`. The fields prepare the runtime for B10 (target portability), under which the compiler will accept a target descriptor and emit bytecode for various architectures.

A golden-bytes test pins the exact thirty-seven-byte serialization of `fn main() -> i64 { 1 }`. The test catches unintended wire format changes and endian-dependent code paths. Updating the expected byte sequence requires a deliberate decision and a `BYTECODE_VERSION` bump if the change is not backwards compatible.

The validation order in `Module::from_bytes` is truncation, magic, length, CRC residue, version, word size, address size, and body decode. The CRC check precedes the version, word size, and address size checks because a corrupted byte in any of those fields would otherwise be reported as a mismatch rather than the more accurate `BadChecksum`. The length check precedes the CRC check because the CRC range depends on the recorded length.

## Changes Made

### Source

- **`src/bytecode.rs`**: `BYTECODE_VERSION` bumped from one to two. New `RUNTIME_WORD_BITS` and `RUNTIME_ADDRESS_BITS` constants set to sixty-four. `HEADER_LEN` raised from six to twelve. New `WordSizeMismatch` and `AddressSizeMismatch` LoadError variants with display strings. `Module::to_bytes` writes the new header layout including total length in little-endian and the two size fields. `Module::from_bytes` validates length authoritatively, runs the CRC over the truncated slice, and checks version, word size, and address size in order.
- **`src/vm.rs`**: Existing `bytecode_rejects_bad_magic` and `bytecode_load_via_vm_propagates_load_error` extended to sixteen-byte input slices. Existing `bytecode_rejects_bad_checksum` updated to flip a byte beyond the length field so the truncation check does not trip first. Six new tests added: `bytecode_rejects_oversized_length_field`, `bytecode_rejects_undersized_length_field`, `bytecode_rejects_word_size_mismatch`, `bytecode_rejects_address_size_mismatch`, `bytecode_admits_trailing_padding`, and `bytecode_golden_bytes_for_main_returning_one`.

### Knowledge Graph

- **`docs/decisions/RESOLVED.md`**: R39 updated. Wire format description includes the new header fields. Length authority is documented. Word size and address size fields are documented with reference to B10. Endian portability is now explicitly stated and tied to the golden-bytes test. The validation order is updated.
- **`docs/architecture/EXECUTION_MODEL.md`**: Bytecode Loading section updated. Header layout includes the new fields. Length authority is documented.
- **`docs/process/TASKLOG.md`**: V0.1-M1-T6 row added. New history row added.
- **`docs/process/REVERSE_PROMPT.md`**: This file.

## Trade-offs and Properties

The total framing length field enables embedding bytecode within a larger buffer. The reader can find the boundary of a single bytecode within a multi-segment archive or a flash region with padding. The cost is four bytes of header overhead.

The word size and address size fields record forward compatibility metadata. The current single-target runtime rejects bytecode that records a different size, which surfaces silent target mismatches as a clear error. The cost is two bytes of header overhead. The fields are eight-bit unsigned integers, sufficient for any plausible target from eight-bit microcontrollers up to two-hundred-fifty-five-bit hypothetical targets.

The version bump from one to two is the cleanest signaling that the format changed. Version one is now an obsolete intermediate format that was never released to crates.io and was only present during the prior commit in this session. Version two is the first stable wire format.

The golden-bytes test pins thirty-seven bytes that capture the exact serialization of a minimal program. The test will fail loudly on any unintended change. Intentional changes require a `BYTECODE_VERSION` bump and an update to the expected byte sequence. The cost is roughly twelve lines of test code.

## Unaddressed Concerns

1. **The golden test is a single program.** Coverage is high for the framing and a tiny body, but rare combinations of `Op` variants or `Value` shapes that produce unusual postcard encoding are not pinned. A future iteration could pin a richer program to broaden coverage. Not blocking because the current test catches all framing-level changes.

2. **Word size and address size are eight-bit.** This caps representable widths at two-hundred-fifty-five bits, which is sufficient for all known and plausible targets but is not unlimited. A future iteration could promote to sixteen-bit if exotic targets motivate it. Not blocking.

3. **Length field is thirty-two-bit.** This caps bytecode at four gigabytes, which is sufficient for all embedded and most desktop use cases but is not unlimited. A future iteration could promote to sixty-four-bit if very large bytecode motivates it. Not blocking.

4. **Cross-endian verification is by construction, not by test on a big-endian target.** The session has not run the runtime on a big-endian target. The golden-bytes test will detect any unintended native-endian code paths if it ever ran on such a target. Adding a CI matrix that includes a big-endian target through cross or QEMU is admissible as a future addition.

5. **No incremental or chunked deserialization.** The `from_bytes` path operates on the full slice. Hosts that stream bytecode from a network would need to buffer the full bytecode before calling. Adequate for the stated embedded and file-loading use cases.

## Intended Next Step

Three paths.

A. V0.1-M2 implementing P10 path B. Lifetime-parameterize Module. Eliminate String fields in favor of byte-offset references. The new header fields integrate naturally because the length, word size, and address size are all positional and known.

B. V0.1-M2 advancing to V0.1 candidates such as the type checker (P1), for-in over arbitrary expressions (P2), or error recovery (P3).

C. V0.1-M2 returning to P7 follow-on items, namely operand stack and `DynStr` arena migration in the keleusma runtime.

Recommend A if the precompiled-code use case is the priority. The wire format is now stable and well-tested. Path B's zero-copy execution against `.rodata` becomes the natural completion of the precompiled-distribution story.

Await human prompt before proceeding.

## Session Context

This session began with V0.0-M5 and V0.0-M6 already complete and the arena extracted into a workspace crate. The session resolved P8 and P9, completed three pre-publication audit and polish passes on `keleusma-arena`, published v0.1.0 to crates.io, switched the keleusma main crate to consume the registry version, completed V0.1-M1 implementing precompiled bytecode loading and trust-based verification skip, hardened the wire format with a CRC-32 algebraic self-inclusion trailer, and now extended the header with length, word size, and address size, pinned a golden-bytes test, and bumped the wire format version from one to two.
