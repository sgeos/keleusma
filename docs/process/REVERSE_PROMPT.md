# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: V0.2.0 ISA Phase 7b landed on the `V0.2.0-isa` branch. The wire-format codec round-trips an entire `Module` through the section-partitioned body. The default `Module::to_bytes` / `Module::from_bytes` / `Module::access_bytes` continue to route through rkyv pending the Phase 7c cutover; the parallel route gives the new format coverage without disturbing the existing test surface or the execution loop.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Proceed with Phase 7b. Address any open concerns if possible. | **Phase 7b: parallel-route Module codec.** New rkyv-archived `WireChunk` and `WireAuxBody` types in `src/wire_format.rs` separate per-chunk metadata (constants, struct templates, parameter types, local count, byte offset, record count) from the chunk ops themselves. New `wire_format::module_to_wire_bytes(&Module) -> Result<Vec<u8>, LoadError>` encodes a full Module: 64-byte framing header, opcode stream packed as 4-byte records in chunk declaration order, operand pool packed as 8-byte entries, rkyv-archived auxiliary body, CRC-32 trailer. The section offsets and lengths are written into the framing header; the opcode stream and operand pool are 8-byte aligned through explicit padding. New `wire_format::module_from_wire_bytes(&[u8]) -> Result<Module, LoadError>` validates the framing (magic, version, header length, total length, CRC residue), reads each section, deserializes the auxiliary body, decodes each chunk's ops from its `op_byte_offset` / `op_record_count` slice in the opcode stream, and reconstructs the Module. The decoder cross-checks header-mirrored fields against the auxiliary body and rejects disagreement as `LoadError::Codec`. **Concern: inherited float test failure.** `target::tests::host_target_admits_floats_and_strings` is now gated on `feature = "floats"`; a parallel `host_target_admits_strings_without_floats` covers the same admissibility surface in the no-floats build. The `--no-default-features --features compile,verify` test run that previously failed at lex now passes. |

## Verification matrix

```bash
cargo test --workspace                                                          # 785 lib (+9 wire-format round-trip) + 53 rogue-script + 17 marshall tests, all green
cargo test --lib --no-default-features --features compile,verify                # 699 no-floats lib tests, all green
cargo clippy --tests --all-targets -- -D warnings                               # clean
cargo build --examples                                                          # clean
cargo fmt --all                                                                 # idempotent

# Bare-metal STM32N6570-DK build, full pipeline.
(cd examples/rtos && cargo build --release --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf --no-default-features \
    --features stm32n6570dk-platform)                                          # clean
```

## Open concerns

| Item | Note |
|------|------|
| Phase 7c not started. | The wire-format codec works end-to-end through the parallel route but the default `Module::to_bytes` / `Module::from_bytes` continue to round-trip the entire Module via rkyv. Phase 7c switches the default serialization, removes the chunk `ops` field from the rkyv archive (it lives in the opcode stream), updates `Module::access_bytes` and `Vm::decode_all_ops`, and refreshes the golden-bytes test. Anticipated scope: ~300 lines plus extensive test updates. |
| Live soft-warning trigger test still not added. | Inherited from Phase 6. A live trigger needs a synthetic source program with > 52,428 ops; compile time alone makes this impractical. The threshold logic is exercised through `chunk_size_thresholds_are_consistent` and indirectly through code review. |

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed as not-applicable (closures retired in Phase 4) |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | In progress (Phases 1, 2, 3, 3.5, Consolidation B, 4, 5, 6, 7a, 7b complete; Phases 7c, 8 pending) |

## Intended Next Step

Awaiting operator prompt. The next development action is one of:

- B20 Phase 7c: cut default `Module::to_bytes` / `Module::from_bytes` over to the wire format; remove the rkyv `Module` archive from the execution path.
- B20 Phase 8: documentation alignment and `BYTECODE_VERSION` re-affirmation at 1.
- A narrow-width overflow-detection follow-up for `CheckedXxx` flag and high-half reporting under bytecode-declared narrower word width.
- Operator selection of a different directive.
