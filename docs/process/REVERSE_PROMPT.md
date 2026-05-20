# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: V0.2.0 ISA Phase 7a landed on the `V0.2.0-isa` branch. The wire-format specification and the wire-format types live in `src/wire_format.rs`; the execution path remains on rkyv until the Phase 7b cutover. External-native chunk-level WCMU integration (the longstanding Phase 5 concern) closed as part of the same commit: the verifier folds `max_invocations_per_iteration * per_call_wcmu_bytes` per chunk for external natives while continuing to sum per-call WCMU over static call sites for verified natives.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Proceed with Phase 7. Address any open concerns if possible. | **Phase 7a (wire format spec + types).** New `docs/architecture/WIRE_FORMAT.md` covers the 64-byte framing header, the 4-byte fixed-size opcode records with parity, the 8-byte operand pool entries with type tag and parity, and the section-partitioned body. New `src/wire_format.rs` ships the types: `OpcodeId`, `OpcodeRecord`, `OperandPoolEntry`, the canonical opcode-id table, the encoder `encode_op(&Op, &mut Vec<OperandPoolEntry>) -> Result<OpcodeRecord, WireFormatError>`, and the decoder `decode_op(OpcodeRecord, &[OperandPoolEntry]) -> Result<Op, WireFormatError>`. Round-trip tests cover every Op variant grouped by operand shape (no operand, `u8`, `u16`, `(u16, u8)`, pool `(u16, u16)`, pool `(u16, u16, u8)`). Parity-detection tests for both record and pool entry shapes. Phase 7b (section-partitioned body in `Module::to_bytes` / `from_bytes`) and Phase 7c (rkyv removal from the execution path) remain pending. **External-native chunk-level WCMU integration (Phase 5 concern).** New `verify::NativeIterationBound { per_call_wcmu_bytes, max_invocations: Option<u32> }`. New `verify::module_wcmu_with_bounds` walks each chunk: verified natives accumulate `per_call_wcmu_bytes` per static call site via the existing per-site walk; external natives are deduplicated per chunk and contribute `max_invocations * per_call_wcmu_bytes` once per chunk regardless of static call-site count. New `verify::verify_resource_bounds_with_bounds` wraps the new pass for arena-capacity checks. VM `verify_resources` and `auto_arena_capacity` route through a new private `native_iteration_bounds` helper. The previous "external natives contribute zero" handoff is removed; the new path computes the sound bound directly. |

## Verification matrix

```bash
cargo test --workspace                                                          # 776 lib (was 759; +14 wire-format, +3 bounds) + 53 rogue-script + 17 marshall tests, all green
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
| Phase 7b not started. | The wire-format types are in place but `Module::to_bytes` and `Module::from_bytes` continue to round-trip through rkyv. Phase 7b switches the producer and consumer to the section-partitioned body. Anticipated scope: encode chunks into a contiguous opcode stream with per-chunk byte offsets in a chunk table; emit the operand pool alongside; keep the auxiliary body (chunk metadata, constants, struct templates, native names, data layout, entry point) in rkyv. Approximately 400 to 600 new lines. |
| Phase 7c not started. | Migrates the auxiliary body off rkyv. Anticipated scope: ~400 lines for the auxiliary body encoder/decoder plus removal of the rkyv dependency from the execution path. The cross-process transport mechanism (compile-time host -> precompiled artefact) may retain a rkyv-based serialization separately. |
| `compile,verify` no-default-features test failure inherited. | `target::tests::host_target_admits_floats_and_strings` panics with a float-literals-require-feature lex error under `--no-default-features --features compile,verify` because the test uses `f64` literals. The failure predates this session's work and persists across the V0.2.0-isa branch tip. Out of scope here; should be addressed by either gating the test or replacing the float literal with a non-float counterpart. |
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
| B20 | V0.2.0 ISA and wire format implementation | In progress (Phases 1, 2, 3, 3.5, Consolidation B, 4, 5, 6, 7a complete; Phases 7b, 7c, 8 pending) |

## Intended Next Step

Awaiting operator prompt. The next development action is one of:

- B20 Phase 7b: section-partitioned bytecode body (opcode stream + operand pool through the new wire format; auxiliary body remains rkyv).
- B20 Phase 7c: auxiliary body in custom format; rkyv removed from the execution path.
- B20 Phase 8: documentation alignment and `BYTECODE_VERSION` re-affirmation at 1.
- Fix the `compile,verify` no-default-features `host_target_admits_floats_and_strings` test failure.
- A narrow-width overflow-detection follow-up for `CheckedXxx` flag and high-half reporting under bytecode-declared narrower word width.
- Operator selection of a different directive.
