# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: V0.2.0 ISA Phase 8 landed on the `V0.2.0-isa` branch. The V0.2.0 publication readiness pass is complete. All eight phases of B20 (1, 2, 3, 3.5, Consolidation B, 4, 5, 6, 7a, 7b, 7c, 8) are done; B20 closes. The branch is ready for merge to `main` and for the V0.2.0 publication step.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Proceed with Phase 8. Address any open concerns if possible. | **Documentation alignment.** FAQ "Strings" section rewritten: the `text` cargo feature, f-string interpolation surface, and the bundled `to_string` / `concat` / `length` / `slice` utility natives are retired references and the section now describes the static-string-plus-host-natives V0.2.0 surface. The static-string escape table no longer references `\{` / `\}`. COOKBOOK "Working with Text" section follows the same shape. The FAQ "Closures" entry and the WHY_REJECTED.md "Recursive closure" / "CallIndirect" entries point at the type-checker-stage rejection diagnostic introduced in Phase 4. The EMBEDDING "Bundled Natives" section updated to reflect `register_utility_natives` shrinking to `println` only and to add `stddsl::Math` / `stddsl::Audio` / `stddsl::Shell` as the bundled library surface. **Version re-affirmation.** `BYTECODE_VERSION` is `1` in `src/bytecode.rs:1429`. **rkyv derives dropped.** `Module`, `Chunk`, and `Op` no longer carry `Archive`, `Serialize`, `Deserialize` because the wire-format codec is the sole serialization path; `WireAuxBody`, `WireChunk`, `ConstValue`, `StructTemplate`, `DataLayout`, `DataSlot`, `SlotVisibility`, `BlockType`, `TypeTag` retain their derives because they participate in the rkyv-encoded auxiliary body. **Stale fixtures retired.** `examples/scripts/piano_roll/piano_roll_*.kel.bin` (10 files) deleted; nothing in the workspace consumed them. |

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
| Live soft-warning trigger test still not added. | Inherited from Phase 6. A live trigger needs a synthetic source program with > 52,428 ops; compile time alone makes this impractical. The threshold logic is exercised through `chunk_size_thresholds_are_consistent` and indirectly through code review. The hard cap path is exercised through the `CompileError` surface; the soft-warning return shape is exercised through `small_chunk_produces_no_warnings`. |
| Narrow-bytecode-on-wide-runtime `CheckedXxx` flag / high half. | Inherited from Consolidation B. The `low` half is correctly sign-extended truncated through `truncate_int_to_declared_width`. Narrow-width overflow detection through `flag` and the `high` half is deferred to a future verifier pass. |

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
- A narrow-width overflow-detection follow-up for `CheckedXxx` flag and high-half reporting under bytecode-declared narrower word width.
- A B15 follow-on: remove `Type::Unknown` entirely now that the V0.2.0 ISA work is closed.
- Operator selection of a different directive.
