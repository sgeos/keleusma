# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-06-01
**Status**: B29 strippable debug metadata implemented and concluded for V0.2.1 on the `v0.2.1` branch. All twelve record kinds emit, the VM trap read path is wired, the breakpoint runtime mechanism is in place, and the format has an authoritative spec. B29 is marked resolved in `BACKLOG.md` with three precision refinements deferred. A feature branch `feat-flat-memory-model` is cut from `v0.2.1` for the next deliverable.

## Summary of work since the last reverse-prompt update

B29 was carried from framework-only to feature-complete across a sequence of increments on the `v0.2.1` line.

- The chunk-local `debug_pool` section is attached to `Chunk`, carried through the wire format as the optional `WireChunk.debug_pool_bytes`, and emitted by the compiler under `keleusma compile --debug`. The opcode stream stays byte-identical between debug and release; `keleusma strip` removes the section to byte-identical release bytes. There is no debug opcode.
- All twelve `DebugRecordKind` records emit: `CallSite`, `SourceSpan`, `LineNumber`, `VariableName`, `TypeAnnotation`, `AssertionContext`, `BreakpointCandidate`, `GenericInstantiation`, `IfcLabelAnnotation`, `WcetMarker`, `OptimisationMarker`, and `VerifierWitness`.
- `VerifierWitness` is a per-construct structural trace produced inline by the shared `verify_chunk` routine (so it cannot drift from the verdict), plus resource-bound proofs: per-iteration for Stream chunks and per-call (Func) or per-resume (Reentrant) for non-Stream chunks.
- The VM records the faulting op in a `fault_location` field and resolves it to source through `Vm::fault_source_location`, two-tier: an exact span-bearing record at the fault op, else the nearest enclosing statement. Every partial-operation trap (division, modulo, array and data-array indexing, newtype refinement) and every failed debug `assert` resolves exactly via an operator-site `SourceSpan`.
- The breakpoint runtime mechanism (`set_breakpoint`/`clear_breakpoint`/`resume_from_breakpoint`, `GenericVmState::BreakpointHit`) suspends before an armed op with a zero-cost `is_empty` fast path. Candidates emit at statement boundaries, block tail expressions, trap-bearing operators, and function entry.
- The per-resume Reentrant worst-case-execution-time bound is the exact maximum inter-yield segment cost when yields are top-level, and a sound bound that clamps each provably-productive yield-loop to one iteration when a yield is nested in control flow.
- The authoritative format reference is `docs/spec/DEBUG_METADATA.md`, indexed in `docs/spec/README.md` and cross-referenced from `WIRE_FORMAT.md`. The B29 entry in `BACKLOG.md` is marked resolved and retains the design rationale.

## Verification

- `cargo test --workspace` green; `cargo test --all-features --workspace` green.
- `cargo clippy --workspace --tests -- -D warnings` and `--all-features` clean.
- `cargo fmt --all -- --check` clean.
- `cargo doc` clean for the B29 surface; the Keleusma markdown link-checker resolves every relative link including the new spec.
- The pre-push gate passed on every push to `origin/v0.2.1`; the latest is at `6fcf311`.

## Open questions and concerns

- **Relationship between the flat memory model and B28.** `feat-flat-memory-model` is cut and awaiting scope. The existing B28 entry in `BACKLOG.md` already specifies a flat-bytes migration of composite `Value` storage as a pure runtime refactor against the unchanged V0.2.0 instruction set architecture, with phases P0 and P1 complete and P2 through P9 remaining. The "flat memory model" deliverable should be reconciled with B28 before implementation: it is most likely a continuation of that phased plan rather than a separate effort. I will read `BACKLOG.md` B28, `src/value_layout.rs`, `src/flat_value.rs`, and `src/layout_pass.rs` before proposing scope, and confirm the intended boundary with the operator.
- **Three B29 precision refinements deferred**, each a tightening of an already-faithful result, not a coverage or correctness gap: a breakpoint candidate at every operator op; a finer nested-yield Reentrant WCET than the productive-loop-clamped bound; and per-op source spans for non-trapping operations. Documented in the B29 `BACKLOG.md` entry and `DEBUG_METADATA.md`. These are demand-driven and should wait for a concrete consumer.
- `v0.2.1` carries this closeout commit ahead of `origin/v0.2.1`; it has not been pushed (push only when requested).

## Recommended next step

Scope the flat memory model on `feat-flat-memory-model`. First reconcile it with the B28 phased plan (P2 through P9) and confirm with the operator whether this deliverable is the continuation of B28 or a distinct effort, then begin the first phase.

## Reference

- `docs/spec/DEBUG_METADATA.md` is the authoritative debug-metadata format reference.
- `docs/decisions/BACKLOG.md`: B29 (resolved for V0.2.1) and B28 (the flat-bytes composite-value refactor, P0 and P1 complete).
- `src/debug_meta.rs`, `src/compiler.rs`, `src/verify.rs`, `src/vm.rs` carry the B29 implementation.
- `src/value_layout.rs`, `src/flat_value.rs`, `src/layout_pass.rs` carry the B28 flat-layout infrastructure relevant to the flat memory model.
