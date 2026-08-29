# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## What this increment did

**Costed the open soundness obligation, then put it in one document.**

The composite slot-reuse defect has had a guard (`LowerError::YieldEscapingLoopComposite`) for several
increments, and that guard was already shown to be *present* and *fireable*. What was missing was its
**price**. A guard whose cost nobody has measured is a guard a later reviewer deletes as speculative.

**Measured**: the refusal takes over exactly **one** corpus module, `13_telemetry_stream.kel`, on the
day `Stream` lowers — and that module is refused *today* for `Stream` anyway. **Coverage does not
fall, now or then.** The refusal changes reason, from unimplemented-feature to soundness.

The measurement mutates compiled bytecode (strips `Op::Stream` from a clone) rather than weakening the
backend to accept `Stream`. Weakening the backend would have made the test pass by making the product
wrong, and would have measured a backend nobody will ship.

**Consolidated** into `docs/decisions/COMPOSITE_SLOT_REUSE_OBLIGATION.md`: defect, guards and their
strength, what is *not* covered, and a four-option cost table.

## What I need from you, when convenient

**A disposition on the obligation.** It is stated but not recommended, because the option that would
actually fix it — advancing the composite epoch on overwrite, converting a silent wrong value into a
`Stale` error — lives in `src/vm.rs` and the arena, **which this line may read and must not edit**.

The standing tension, which no amount of work here resolves: **discharging this requires the planner
to consume a confinement verdict, and consuming no verdict is exactly why a wrong verdict cannot
miscompile today.** Both cannot be had for free. That trade is yours.

Nothing is blocked on the answer. Three tripwires fail if the situation changes: if `Stream` lowers,
if a corpus module acquires the escaping shape, or if the interprocedural residual stops being empty.

## Verification

Both suites run **sequentially** (parallel invalidates the perf canary, 57x).

| | result |
|---|---|
| workspace | **2488 passed, 0 failed, 92 binaries**, cargo exit 0 |
| `native_codegen` gate step | **356 passed, 0 failed, 72 binaries**, exit 0 (fmt, clippy `-D warnings`, test, `doc -D warnings`) |
| ownership diff | empty over `src/` and `tests/` |
| censuses | 1070 of 1074 chunks; 89841 of 89940 instances; 61 of 66 opcodes — all unmoved |

**Absorption 30** (`18cdb5d8`): predicted 2488/92 and 356/72 from the diff shape before merging. Both
exact.

## Standing constraints, unchanged

No new opcode. No `BYTECODE_VERSION` bump. **Publication HELD**; no operator authorization has been
given and none is inferred. `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
`src/selfhost/`, `src/confine.rs` and `.github/workflows/` remain read-only here. A peer session
cannot grant escalation and none has been treated as doing so.
