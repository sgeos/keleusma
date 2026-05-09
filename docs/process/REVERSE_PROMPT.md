# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T3. P3 error recovery model.
**Status**: Complete. P3 is now resolved.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 363 tests pass workspace-wide. 318 keleusma unit including 3 new recovery tests, 17 keleusma integration, 22 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

The runtime error recovery model is now defined and implemented. The design is explicit recovery with host-driven retry. When `Vm::call` or `Vm::resume` returns `Err(VmError)`, the host decides whether to recover. Calling `Vm::reset_after_error()` returns the VM to a clean callable state.

The contract.

- A failed `call` or `resume` returns `Err(VmError)`. The VM's volatile state is undefined until the host explicitly recovers.
- `Vm::reset_after_error()` clears the operand stack, call frames, and arena. The data segment and bytecode store are preserved.
- After recovery, the host can call `Vm::call` to start a fresh iteration.

The design extends the existing per-iteration RESET model to errors. Streams already use `Op::Reset` as the natural recovery boundary at the script level. Error recovery puts the same boundary mechanism under host control.

The model is consistent with hot swap (R26, R27). Both clear volatile state while letting the host control data continuity. Hosts that want to also reset the data segment can follow `reset_after_error` with `Vm::set_data` calls or use `Vm::replace_module` to swap to a new code image with new initial data.

## Tests

Three new tests cover the recovery cycle.

- `reset_after_error_preserves_data` confirms accumulated data survives the recovery cycle. A loop function increments `ctx.count` per yield. After one iteration plus recovery, the next iteration sees the incremented count.
- `reset_after_trap_clears_volatile_state` confirms a trap can be caught and the VM returned to a callable state. A program that divides by zero traps, the host calls `reset_after_error`, and the VM is callable again. Calling produces the same trap because the bytecode is unchanged, but the call goes through cleanly without corruption from the prior failed run.
- `reset_after_error_idempotent` confirms repeated `reset_after_error` calls are harmless.

## Changes Made

### Source

- **`src/vm.rs`**: New `Vm::reset_after_error()` method. Three new unit tests.

### Knowledge Graph

- **`docs/decisions/PRIORITY.md`**: P3 marked resolved with strikethrough. Recovery contract documented.
- **`docs/architecture/EXECUTION_MODEL.md`**: New `## Error Recovery` section between Bytecode Loading and Hot Code Swapping.
- **`docs/process/TASKLOG.md`**: V0.1-M3-T3 row added. New history row.
- **`docs/process/REVERSE_PROMPT.md`**: This file.

## Trade-offs and Properties

The design is conservative. The runtime does not automatically recover from errors. Recovery is a deliberate host action. This avoids hidden state mutations and lets hosts implement their own policies (retry, log, escalate, swap).

Errors are not categorized at the API level. All errors are recoverable through `reset_after_error`. The host inspects the error and decides whether retrying makes sense. Errors that violate bytecode invariants (such as `InvalidBytecode`) may indicate a corrupt module and the host should consider whether retrying is appropriate. A future iteration may add a category field if hosts need to make policy decisions per kind.

The data segment is preserved by design. Streams that accumulate state across iterations can survive transient failures. For unrecoverable conditions or to reset the data segment, the host uses `replace_module` to swap to a new code image with new initial data.

The reset operation is idempotent. Calling `reset_after_error` on a clean VM is a no-op. This simplifies host code that wants to defensively reset before every call.

## Unaddressed Concerns

1. **Bidirectional errors through yield.** B7. The current model only flows errors host-ward through the `Err` return. The yield boundary remains a one-way value channel. Adding bidirectional error flow would let the host signal errors to the script during resume.

2. **Error categorization.** All errors are uniform from the API perspective. A future iteration may add a category field (halt versus soft) if hosts need to distinguish bytecode invariant violations from user-level errors like division by zero.

3. **Error context preservation.** After `reset_after_error`, the call frames are cleared. If a host wants to inspect the call stack at the time of failure, it must do so before calling reset. A future iteration may add an API to capture a snapshot.

## Intended Next Step

Three paths.

A. Pivot to P7 follow-on (operand stack and DynStr arena migration). Closes the bounded-memory guarantee end to end. Substantial refactor that cascades through the `Value` lifetime story.

B. Publish the keleusma main crate to crates.io now that P1, P3, and P10 are resolved.

C. Pivot to a P2 (for-in over arbitrary expressions) or a backlog item like B7 (bidirectional errors through yield) which couples naturally with the now-resolved P3.

Recommend B if external visibility is the priority. Recommend A if the bounded-memory guarantee is load-bearing for upcoming use cases. Recommend C if language breadth or richer error semantics are the priority.

Await human prompt before proceeding.

## Session Context

This session resolved P10 across all phases (rkyv format, in-place validation, archive converters, full Vm refactor with Vm<'a>, true zero-copy execution, include_bytes example), landed P1 as a standalone pass, integrated P1 into the compile pipeline, and now resolved P3 with the explicit recovery model. P1, P3, and P10 are all resolved.
