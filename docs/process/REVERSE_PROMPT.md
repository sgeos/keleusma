# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T27. WCET safety for recursive closures.
**Status**: Complete. The recursive-closure feature added in V0.1-M3-T26 was inconsistent with the WCET and WCMU analyses by construction. This session corrects the soundness gap by rejecting `Op::MakeRecursiveClosure` in `verify_resource_bounds` and explicitly documents the WCET implications of indirect dispatch over closures.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 480 tests pass workspace-wide. 412 keleusma unit (1 new), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

The user reminded that the broader language goal is WCET and WCMU analysis, which means certain features must remain out of scope. The recursive-closure feature added in the prior session dispatches through `Op::CallIndirect`, which the WCMU analysis cannot follow. The pre-existing `topological_call_order` walk over the call graph traces only `Op::Call` edges and rejects direct-call cycles. Recursive closures escape this cycle detection by construction because their self-reference flows through indirect dispatch.

To preserve the soundness of the resource-bounds verifier, `verify::module_wcmu` now rejects any module that contains `Op::MakeRecursiveClosure` with a clear error message. The safe constructors `Vm::new` and `Vm::load_bytes` therefore reject recursive-closure programs by default. Hosts that need recursive closures and accept the unbounded-recursion risk must construct the VM through `Vm::new_unchecked` or `Vm::load_bytes_unchecked`, which skip the resource-bounds check while preserving structural verification. The `examples/closure_recursive.rs` example was updated to use `Vm::new_unchecked` and now documents the trade-off in the source comment.

A broader observation about indirect dispatch is documented in EXECUTION_MODEL: the WCMU analysis does not follow `Op::CallIndirect` targets, so programs that construct unbounded recursion through indirect dispatch over non-recursive closures (for example `apply(apply, x)` where `apply<F>(f: F, x: i64)` invokes its first argument indirectly) are admissible by the verifier despite being unbounded at runtime. Tightening this would require either a conservative max-cost-over-all-chunks bound for `Op::CallIndirect` or a flow analysis that tracks which chunks each Func value may resolve to. The approximation is recorded as a known limitation rather than fixed in this session because it requires substantial design work and the explicit rejection of `Op::MakeRecursiveClosure` already covers the common path through which the surface language can express unbounded recursion.

## Tests

One new verifier test:

- `verify_resource_bounds_rejects_recursive_closures` constructs a stream chunk that contains `Op::MakeRecursiveClosure` and asserts that `verify_resource_bounds` rejects the module with an error message identifying the cause.

## Trade-offs and Properties

The chosen rejection point is `verify::module_wcmu`. This sits between the `verify` structural pass and the per-Stream-chunk arena-budget check. The rejection happens before any chunk's WCMU is computed, so the error path is clean and has a focused message.

An alternative approach would be a recursion-depth attestation API analogous to `Vm::set_native_bounds`. The host would declare the maximum recursion depth for each recursive closure, and the analysis would multiply that closure's per-invocation WCET and WCMU by the declared depth. This is a future refinement and is recorded in BACKLOG as a follow-on item. For real-time embedding without external attestation, recursive closures remain out of scope and the safe constructor's rejection is the correct contract.

The example continues to demonstrate the recursive-closure feature end to end, but now uses `Vm::new_unchecked` and documents the trade-off in the source comment. This keeps the feature available for development, scripting, and tests while making the WCET implications explicit at the call site.

## Changes Made

### Source

- **`src/verify.rs`**. `module_wcmu` now scans every chunk for `Op::MakeRecursiveClosure` and returns a `VerifyError` if any is present. New unit test `verify_resource_bounds_rejects_recursive_closures`.
- **`examples/closure_recursive.rs`**. Updated to use `Vm::new_unchecked` and to document the WCET trade-off in the doc comment and at the constructor call site.

### Knowledge Graph

- **`docs/architecture/EXECUTION_MODEL.md`**. New "Indirect Dispatch and Recursion" subsection inside "Structural Verification" documents how the analyses handle indirect dispatch, the rejection of `Op::MakeRecursiveClosure` by the safe constructor, the unsafe constructor opt-out path, and the known approximation that the analysis does not follow `Op::CallIndirect` targets.
- **`docs/decisions/BACKLOG.md`**. B3 entry expanded with the WCET and WCMU implications, the verifier behavior, and the future recursion-depth attestation refinement.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T27.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

Recursive closures are now WCET-safe in the sense that the safe constructor refuses to admit them. Hosts that need recursive closures must opt out of the resource-bounds verification through the unsafe constructor.

The known approximations are documented:

- `Op::CallIndirect` cost analysis does not follow indirect-dispatch targets. Unbounded recursion via patterns like `apply(apply, x)` is admissible despite being unbounded.
- A recursion-depth attestation API for recursive closures would re-admit them under a host-declared bound. Not implemented.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

Await human prompt before proceeding. Subsequent work falls outside the named B1, B2.2, B2.3, B2.4, and B3 scope or pertains to WCET refinements (the `Op::CallIndirect` flow analysis or the recursion-depth attestation) that need design work before implementation.

## Session Context

This session was a corrective pass: the recursive-closure feature added in V0.1-M3-T26 was inconsistent with the broader WCET and WCMU goal. The verifier now rejects recursive-closure programs by default, hosts can opt out explicitly through the unsafe constructor, and the WCET implications of indirect dispatch are documented in EXECUTION_MODEL. The closure subsystem is now feature-complete and aligned with the language's analysis goals.
