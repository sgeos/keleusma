# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T36 Plug the CallIndirect WCET hole.
**Status**: Complete. The safe verifier rejects any module that would invoke a first-class function value through indirect dispatch. The closure examples that depended on this admission are removed. Documentation reframed to make the rejection an explicit contract rather than a documented approximation.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 508 tests pass workspace-wide. Two new verifier tests added.
- Format clean.
- Clippy clean.

## Summary

The language's load-bearing guarantee is definitive WCET and WCMU. Programs whose execution time or memory use cannot be statically bounded must be rejected by the safe verifier. `Vm::new_unchecked` exists for trust-skip of precompiled bytecode validated during the build pipeline; using it to admit unbounded programs at runtime is intentional misuse outside the language's contract.

The pre-existing soundness gap was that `Op::CallIndirect` resolves its target chunk at runtime from a `Value::Func` on the operand stack. The static analysis cannot follow this edge through the call graph, so the cost of the indirect call cannot be bounded. The previous documentation framed this as a "known approximation" that programs could exploit through patterns like `apply(apply, x)`, with the remediation being either a flow analysis or a max-cost-over-targets bound. Neither was implemented, so the gap remained open.

This session plugs the gap by rejecting `Op::CallIndirect` outright in `verify::module_wcmu`. The construction ops `Op::PushFunc` and `Op::MakeClosure` remain admissible because they produce values that can be yielded, stored in the data segment, or otherwise consumed without invocation. Only dispatch through `Op::CallIndirect` is the load-bearing rejection. Programs that require definitive WCET and WCMU bounds restrict themselves to direct calls.

`Op::MakeRecursiveClosure` continues to be rejected separately because its construction implies indirect self-dispatch by design.

### Examples removed

Five closure examples are removed:

- `closure_basic`, `closure_capture`, `closure_as_arg`, `closure_nested` all relied on `Vm::new` admitting indirect dispatch. After the strictification, these would all fail at the safe constructor.
- `closure_recursive` used `Vm::new_unchecked` to bypass the previous `Op::MakeRecursiveClosure` rejection. The user has now reframed this kind of usage as intentional misuse rather than a supported escape hatch, so the example modeled an anti-pattern.

The closure feature itself remains in the language. The compile pipeline parses, type-checks, monomorphizes, and emits closure programs. The runtime executes them correctly. Hosts with non-real-time requirements can construct the VM through the unsafe path at their own risk. The repository does not advertise closures as part of the WCET-safe surface and does not include examples that would model unsafe usage.

### Verifier tests added

- `verify_resource_bounds_rejects_call_indirect` constructs a Stream chunk that contains `Op::CallIndirect` and asserts the verifier rejects with a message identifying the cause.
- `verify_resource_bounds_admits_push_func_without_call_indirect` constructs a Stream chunk that contains `Op::PushFunc` followed by `Op::Yield` (no invocation) and asserts the verifier admits it. This pins the intended distinction: production of a Func value is fine; only invocation is rejected.

### Documentation reframed

- `EXECUTION_MODEL.md` Indirect Dispatch and Recursion subsection rewritten. The previous "known approximation" framing is removed in favor of explicit rejection semantics. The text now explicitly describes `Vm::new_unchecked` usage as intentional misuse when employed to admit unbounded programs.
- `LANGUAGE_DESIGN.md` Scope Inclusions list updated. The closures bullet no longer claims WCET safety; it now states that closures are implemented in the pipeline but not part of the WCET-safe surface, and points at the BACKLOG B3 entry and EXECUTION_MODEL.
- `BACKLOG.md` B3 entry retitled "Implemented; not WCET-safe" with the prior "Resolved with environment capture" framing replaced. The detailed implementation history is condensed; the WCET-rejection contract is now the lead.

## Trade-offs and Properties

The chosen rejection point is `verify::module_wcmu`, the same function that already rejected `Op::MakeRecursiveClosure`. The two rejections are now adjacent in the source and share their rationale comment. This keeps the WCET soundness logic in one place.

The strict rejection eliminates first-class function values from WCET-safe programs. This is a real loss of expressive power for hosts that wanted closures in their scripts. The user's framing accepts this loss because the language's value proposition is the bound itself, not feature breadth. Programs that need closures use a non-WCET-safe pipeline; programs that need WCET use direct calls.

A future refinement that recovers some closure expressiveness would be a real flow analysis that determines which chunks each `Op::CallIndirect` could possibly dispatch to. The simplest sound version produces a tight per-call-site target set by tracing the local slot back to its construction. This would admit non-recursive closure use while keeping the cycle rejection. The cost is a bytecode dataflow analysis estimated at 200 to 300 lines. Recorded as future work but not pursued in this session.

## Files Touched

- **`src/verify.rs`**. Extended `module_wcmu` rejection to cover `Op::CallIndirect`. Two new tests.
- **`examples/closure_basic.rs`**, **`closure_capture.rs`**, **`closure_as_arg.rs`**, **`closure_nested.rs`**, **`closure_recursive.rs`**. Removed.
- **`docs/architecture/EXECUTION_MODEL.md`**. Indirect Dispatch and Recursion subsection rewritten.
- **`docs/architecture/LANGUAGE_DESIGN.md`**. Closure bullet under Scope Inclusions updated.
- **`docs/decisions/BACKLOG.md`**. B3 entry retitled and condensed.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T36.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The CallIndirect WCET hole is plugged. Remaining hardening work documented in prior sessions:

- Indirect-dispatch flow analysis to selectively re-admit non-recursive closures. Optional refinement.
- Recursion-depth attestation API for direct recursive calls when statically bounded by host attestation.
- Compile-time WCMU rejection (currently only Vm::new rejects; compile() does not). Defense-in-depth against build-pipeline misconfiguration.
- Fuzz harness for parser and bytecode loader.
- Miri coverage on the runtime crate.
- Criterion benchmarks for measured cost evidence.
- `Type::Unknown` sentinel removal.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session executed the user's stated top priority: plug the WCET hole that allowed unbounded programs to be admitted by the safe constructor. The strict rejection of `Op::CallIndirect` is the simplest sound mechanism. The closure feature continues to exist in the language pipeline but is no longer advertised as WCET-safe. Examples that modeled WCET-unsafe usage are removed. Documentation is reframed to make the rejection an explicit contract rather than an admitted approximation.
