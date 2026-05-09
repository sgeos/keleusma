# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: V0.1-M1 word and address sizes as base-2 exponents with relaxed acceptance and integer masking.
**Status**: Complete.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 341 tests pass workspace-wide. 296 keleusma unit, 17 keleusma integration, 22 keleusma-arena unit, 6 keleusma-arena doctests. Two new tests confirm narrower bytecode acceptance and integer truncation under the masking pass.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

Word size and address size in the bytecode header are now encoded as base-2 exponents. The actual width in bits is `1 << field`. The current Keleusma runtime is built for sixty-four-bit words and sixty-four-bit addresses, so `RUNTIME_WORD_BITS_LOG2` and `RUNTIME_ADDRESS_BITS_LOG2` are both six.

The acceptance policy relaxed to `bytecode_exponent <= runtime_exponent`. Bytecode compiled for a narrower target runs on a wider runtime. A program compiled for thirty-two-bit words runs on a sixty-four-bit runtime under the integer masking pass. A program compiled for sixty-four-bit words is rejected on a thirty-two-bit runtime.

The encoding restricts widths to powers of two. The covered set is one, two, four, eight, sixteen, thirty-two, sixty-four, one-hundred-twenty-eight, and two-hundred-fifty-six bits at exponents zero through eight. The restriction excludes non-power-of-two architectures such as twenty-four-bit DSPs (Motorola 56000) and thirty-six-bit historical machines (PDP-10). Keleusma's stated target range from 6502 through ARM64 is entirely powers of two, so the restriction is acceptable.

The VM applies sign-extending integer truncation `(value << shift) >> shift` where `shift = 64 - (1 << word_bits_log2)` to arithmetic results when the bytecode declares a word size narrower than the runtime supports. The truncation is applied after `Add`, `Sub`, `Mul`, `Div`, `Mod`, and `Neg` for integer operands. The float and string paths are unaffected. When the declared width matches or exceeds sixty-four bits, the truncation is the identity.

The expression `2147483647 + 1` produces `2147483648` under sixty-four-bit semantics. Under the bytecode-declared thirty-two-bit semantics on a sixty-four-bit runtime, the masking pass produces `i32::MIN` which is `-2147483648`. The new `bytecode_masking_truncates_to_declared_width` test confirms the behavior.

`BYTECODE_VERSION` bumped from two to three. The version-two and version-one wire formats are now obsolete intermediate formats that were never released to crates.io.

## Changes Made

### Source

- **`src/bytecode.rs`**: `BYTECODE_VERSION` bumped to three. New `RUNTIME_WORD_BITS_LOG2` and `RUNTIME_ADDRESS_BITS_LOG2` constants set to six. Old `RUNTIME_WORD_BITS` and `RUNTIME_ADDRESS_BITS` removed. New `Module::word_bits_log2` and `Module::addr_bits_log2` fields with `#[serde(skip, default = "...")]` so they are excluded from the postcard body. Default helper functions return the runtime constants. New `truncate_int` helper applies sign-extending mask. `Module::to_bytes` writes the per-Module exponents from `self`. `Module::from_bytes` validates `<=` and stores the exponents on the deserialized Module. `LoadError::WordSizeMismatch` and `LoadError::AddressSizeMismatch` field renamed from `expected` to `max_supported` to reflect the relaxed policy. Display strings updated to report decoded bit widths.
- **`src/compiler.rs`**: Module construction sets `word_bits_log2` and `addr_bits_log2` to runtime defaults.
- **`src/vm.rs`**: Arithmetic ops `Add`, `Sub`, `Mul`, `Div`, `Mod`, `Neg` now apply `bytecode::truncate_int` to integer results using `self.module.word_bits_log2`. The `binary_arith` helper takes the same path. Integer division and modulo switched to `wrapping_div` and `wrapping_rem` for consistency under truncation. Existing tests updated to use the new field names and exponents. Two new tests added: `bytecode_admits_narrower_word_size` and `bytecode_masking_truncates_to_declared_width`. Golden bytes updated for version three with exponent encoding. Existing `bytecode_rejects_word_size_mismatch` and `bytecode_rejects_address_size_mismatch` updated for `max_supported` field name and use `runtime_log2 + 1` to trigger rejection.
- **`src/verify.rs`**: Test-only Module constructions updated to include the new fields.

### Knowledge Graph

- **`docs/decisions/RESOLVED.md`**: R39 updated. Wire format description includes the exponent encoding and the relaxed acceptance policy. The integer masking pass is documented. Power-of-two restriction is recorded as acceptable for the stated target range.
- **`docs/architecture/EXECUTION_MODEL.md`**: Bytecode Loading section updated. Constants renamed. Acceptance policy and masking documented.
- **`docs/process/TASKLOG.md`**: V0.1-M1-T7 row added. New history row added.
- **`docs/process/REVERSE_PROMPT.md`**: This file.

## Trade-offs and Properties

The exponent encoding is more compact than the literal bit count and aligns with hardware conventions. Eight bits suffice to express widths from one bit to two-hundred-fifty-six bits.

The relaxed acceptance policy enables forward-compatible bytecode distribution. A binary compiled for 8-bit AVR can run on a 64-bit ARM64 runtime under masking. The cost is per-arithmetic-op truncation overhead. For 64-bit-on-64-bit (the current default), the truncation is the identity branch and the overhead is a single comparison.

The sign-extending shift pattern `(value << shift) >> shift` is branchless and produces correct two's complement semantics for all widths from one to sixty-three bits. The shift count is bounded by `64 - 1 = 63`, which is within Rust's defined-behavior range for shifts.

The `wrapping_div` and `wrapping_rem` switch matches the wrapping behavior of `wrapping_add`, `wrapping_sub`, `wrapping_mul`, and `wrapping_neg` already in use. The only previously non-wrapping case was `i64::MIN / -1`, which is now handled.

## Unaddressed Concerns

1. **Float width is not yet parameterized.** The header records integer word size only. `Value::Float` is always `f64`. A future iteration may add a separate float-size field for targets that use `f32` natively. Not blocking because the current single-target runtime always uses `f64`.

2. **Constants in the postcard body are i64-encoded.** A bytecode compiled for thirty-two-bit words still serializes its constants as i64 because `Value::Int` holds `i64`. The values are correct after deserialization but the wire form is wider than necessary. A future iteration may add target-aware constant encoding under B10. Not blocking.

3. **Address-related operations.** The address-bits field is read and validated but no current opcode is parameterized by address size. The field is metadata only at present. Future opcodes that compute or compare addresses will use it. Not blocking.

4. **Cross-width execution is not exhaustively tested.** The new tests cover the 32-bit-on-64-bit path with the canonical `i32::MAX + 1` overflow. Other widths (16-bit, 8-bit) and other operations (multiplication overflow, division by `-1` with `i64::MIN` truncated to narrower) are exercised through the masking helper but not pinned by individual tests. The masking pass is the same regardless of width, so the existing test should be sufficient. A future iteration may add per-width tests if confidence requires.

5. **The `truncate_int` function is `pub(crate)`.** Visibility is the minimum required for tests to access it. Could be promoted to `pub` for host-side use, but no host scenario currently motivates that.

## Intended Next Step

Three paths.

A. V0.1-M2 implementing P10 path B. Lifetime-parameterize Module. Eliminate String fields in favor of byte-offset references. The current header design is already path-B-friendly because the length, word size, and address size are positional and known at fixed offsets.

B. V0.1-M2 advancing to V0.1 candidates such as the type checker (P1), for-in over arbitrary expressions (P2), or error recovery (P3).

C. V0.1-M2 returning to P7 follow-on items, namely operand stack and `DynStr` arena migration in the keleusma runtime.

Recommend A if the precompiled-code use case is the priority. The wire format is now stable, well-tested, and forward-compatible for narrower-target bytecode. Path B's zero-copy execution against `.rodata` becomes the natural completion of the precompiled-distribution story.

Await human prompt before proceeding.

## Session Context

This session began with V0.0-M5 and V0.0-M6 already complete and the arena extracted into a workspace crate. The session resolved P8 and P9, completed three pre-publication audit and polish passes on `keleusma-arena`, published v0.1.0 to crates.io, switched the keleusma main crate to consume the registry version, completed V0.1-M1 implementing precompiled bytecode loading and trust-based verification skip, hardened the wire format with a CRC-32 algebraic self-inclusion trailer, extended the header with length, word size, and address size and pinned a golden-bytes test, and now shifted word and address fields to base-2 exponent encoding with relaxed acceptance and integer masking. The phase has advanced to V0.1.
