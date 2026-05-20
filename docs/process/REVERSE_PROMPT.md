# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: V0.2.0 ISA Phase 7c landed on the `V0.2.0-isa` branch. The wire format is now the default: `Module::to_bytes`, `Module::from_bytes`, and `Module::access_bytes` all route through the section-partitioned body. The legacy 32-byte framing header and the `op_from_archived` conversion retire. The rkyv archive of the full `Module` is no longer used at runtime; only the rkyv-encoded `WireAuxBody` remains on the wire.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Proceed with Phase 7c. Address any open concerns if possible. | **Cutover.** `Module::to_bytes` delegates to `wire_format::module_to_wire_bytes`; `Module::from_bytes` delegates to `wire_format::module_from_wire_bytes`; `Module::access_bytes` validates the wire format and returns `&ArchivedWireAuxBody`. New `wire_format::parse_wire_sections` validates the framing and returns slices for the opcode stream, operand pool, and auxiliary body. New `wire_format::read_header_fields` exposes the header-mirrored target widths and declared WCET / WCMU for fast inspection. New `wire_format::decode_op_stream` materializes an owned `Vec<Op>` from a chunk's opcode-stream slice and the shared operand pool. **VM cutover.** `Vm::archived()` returns `&ArchivedWireAuxBody` and reads the aux body offset/length from header bytes 48-55. `decode_all_ops` walks each chunk's opcode-stream slice (bounded by `op_byte_offset` and `op_record_count` on the WireChunk metadata) through `wire_format::decode_op_stream`. `chunk_op_count` reads `op_record_count` from the WireChunk. `verify_native_classifications` walks `self.decoded_ops` instead of the archived chunk ops. The Stream-block IP recovery path consults `self.decoded_ops` for the position of `Op::Stream`. `view_bytes_zero_copy` reads target widths at the wire-format header offsets and consults the auxiliary body for data slot counts. **Decoder order fix.** The width validation moves before the header-versus-aux cross-check so a patched-only-header byte still surfaces as `WordSizeMismatch` / `AddressSizeMismatch` rather than a generic Codec error. **Retired legacy items.** `HEADER_LEN`, `FOOTER_LEN`, `CRC32_RESIDUE`, `HEADER_WCET_OFFSET`, `HEADER_WCMU_OFFSET`, `HEADER_SHARED_DATA_OFFSET`, `HEADER_PRIVATE_DATA_OFFSET`, `HEADER_FLAGS_OFFSET`, `strip_shebang_prefix`, `op_from_archived` all removed. **Fixtures refreshed.** Golden-bytes test regenerated for the V0.2.0 layout (216-byte total for the minimal program). `examples/zero_copy_demo.kel.bin` regenerated from 316 to 324 bytes; `BYTECODE_LEN` constant in `zero_copy_include_bytes.rs` bumped to match. The `bytecode_admits_narrower_word_size` test now uses `compile_with_target(Target::embedded_16())` so both the header and the aux body carry the narrower width. |

## Verification matrix

```bash
cargo test --workspace                                                          # 785 lib + 53 rogue-script + 17 marshall tests, all green
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
| Module still derives `Archive, Serialize, Deserialize`. | The full `Module` rkyv archive is no longer hit at runtime, but the derive is still on the struct because removing it would cascade through every test that constructs a Module by hand and through external API consumers. The derives are currently harmless; a future cleanup phase can drop them once the surface stabilises. |
| Live soft-warning trigger test still not added. | Inherited from Phase 6. A live trigger needs a synthetic source program with > 52,428 ops; compile time alone makes this impractical. The threshold logic is exercised through `chunk_size_thresholds_are_consistent` and indirectly through code review. |
| Piano roll bin fixtures are stale. | Files at `examples/scripts/piano_roll/piano_roll_*.kel.bin` carry pre-V0.2.0 bytes. They are not consumed by `examples/piano_roll.rs` (which uses `include_str!`) so the staleness is harmless, but a cleanup pass could either delete them or regenerate. |

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed as not-applicable (closures retired in Phase 4) |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | In progress (Phases 1, 2, 3, 3.5, Consolidation B, 4, 5, 6, 7a, 7b, 7c complete; Phase 8 pending) |

## Intended Next Step

Awaiting operator prompt. The next development action is one of:

- B20 Phase 8: documentation alignment, `BYTECODE_VERSION` re-affirmation at 1, FAQ / cookbook / embedding-guide updates, and the V0.2.0 publication readiness pass.
- Drop the `Archive, Serialize, Deserialize` derives on `Module` now that the runtime no longer serializes the full module through rkyv.
- A narrow-width overflow-detection follow-up for `CheckedXxx` flag and high-half reporting under bytecode-declared narrower word width.
- Operator selection of a different directive.
