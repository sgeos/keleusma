# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T30. B7 error propagation through yield, and B8 VM allocation model resolved as not-applicable.
**Status**: Complete. B7 implemented as a resume-value pattern with a thin convenience API. B8 closed without code changes after the analysis showed the originally framed shared-arena design is incompatible with existing contracts.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 497 tests pass workspace-wide. 429 keleusma unit (2 new), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

### B7. Error propagation through yield

Recognized that the existing yield/resume cycle already supports bidirectional error handling without runtime extension. The host's `resume(Value)` accepts any value; the script's yield expression evaluates to that value at runtime; the script patterns matches on a script-defined variant union to distinguish success from error. Result-shaped enums (like `enum Reply { Ok(i64), Err }`) or `Option<T>` are both supported with no language change.

Added `Vm::resume_err(error_value)` as a thin wrapper over `Vm::resume`. The wrapper signals intent at the host call site and provides a clear API name for the failure case. Functionally, it routes through the same operand-stack mechanism as `resume`. The choice of API name reflects that the value being passed represents an error in the host-script protocol, not a successful input.

Recovery semantics follow Keleusma's general dynamic-tag dispatch contract: if the script fails to handle the error variant, the next operation that consumes the value traps with a runtime type error. This is not a new failure mode. Scripts that want strict recovery wrap their dialogue logic in an exhaustive match.

WCET implications. No new bytecode, opcode, or runtime mechanism. Match-arm dispatch is bounded by the number of arms at compile time. The verifier's existing analysis applies unchanged. Hosts that need automatic propagation analogous to Rust's `?` operator can implement that pattern in the script with pattern matching and early `return`; no language extension is required.

### B8. VM allocation model

Closed as not-applicable. The originally framed shared-arena design is incompatible with several existing contracts: per-VM `verify_resource_bounds`, per-arena `KString` epoch tracking, per-VM `Op::Reset` semantics, single-threaded arena ownership, and the per-VM cross-yield prohibition on dynamic strings. The legitimate use cases (allocation overhead amortization across sequential scripts) are already covered by the existing pattern of constructing one `Arena` and reusing it across successive `Vm::new` calls between resets. The "complexity to lifetime management" hedge in the original entry understated the problem; the entry is now updated with the full analysis.

## Tests

Two new VM tests:

- `resume_err_propagates_through_enum_reply` exercises both successful (`Reply::Ok(42)`) and failure (`Reply::Err`) resumes through the same enum-based dialogue.
- `resume_err_passes_through_with_value_none` exercises the failure path through a single resume with the `Err` variant.

One new example: `examples/yield_error.rs` demonstrates the pattern end to end with descriptive output.

## Trade-offs and Properties

The decision to expose `resume_err` as a thin wrapper rather than a richer error-propagation mechanism was deliberate. Adding a real semantic difference (such as a sentinel `Value::Trap` that the script must explicitly catch) would introduce a new failure mode without solving a real problem. The script can already enforce strict handling via exhaustive match, and the runtime can already trap on type mismatch. The wrapper provides documentation value at the host's call site without inviting bytecode changes.

The chosen pattern uses script-defined enums rather than a built-in `Result<T, E>`. This keeps the type system simple and avoids the question of how the error type `E` is declared at the function boundary. Hosts and scripts agree on the dialogue's variant union in source. The trade-off is that there is no compiler-enforced shape; if the host resumes with a value whose type does not match the script's declared resume type, the script traps at the next operation. This is the existing contract for all yield/resume exchanges.

For B8, the "not-applicable" resolution rather than implementation reflects the analysis: the sharing question conflicts with five distinct contracts, none of which were considered when the original entry was written. Recording the analysis in the BACKLOG closes the question and prevents a future implementer from approaching it without seeing the constraints.

## Changes Made

### Source

- **`src/vm.rs`**. New `Vm::resume_err(error_value: Value)` method. Two new unit tests.
- **`examples/yield_error.rs`** (new). End-to-end demonstration.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B7 marked resolved with the surface pattern and host pattern documented. B8 marked not-applicable with the full analysis recorded.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T30.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The named B7 and B8 work is closed. Subsequent backlog items relevant to V0.1-M3 include:

- B11. Per-op decode optimization for zero-copy execution.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session resolved two backlog items with different paths to closure. B7 became a recognition that the existing infrastructure already supported the use case, plus a small convenience API and documentation. B8 became an analysis that the originally framed design is incompatible with existing contracts; the entry is closed without code changes. Both close out the named V0.1 work without breaking any existing tests or contracts.
