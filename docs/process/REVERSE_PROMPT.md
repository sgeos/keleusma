# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-10
**Task**: V0.1-M3-T47 Documentation pass for the SDL3 piano-roll example.
**Status**: Complete. The example is now reachable from each documentation entry point a new user might start from, the keyboard controls are explicit, and two latent correctness bugs in the embedding guide's hot-swap snippet are fixed.

## Verification

**Commands**:

```bash
cargo test --workspace
git diff --stat HEAD~1
```

**Results**:

- 519 tests pass.
- Documentation-only diff. No source files changed.

## Summary

The user observed that the SDL3 piano-roll example was documented only in two places (the top-level `README.md` Examples section and the file's own `//!` doc block) and that the README description had not been refreshed when hot code swap was added in V0.1-M3-T46. The user also pointed out that the documentation knowledge graph and the embedding guide did not surface the example. This pass closes the gaps.

### Top-level README

The Examples section now lists the four principal capabilities the example exercises (bounded-step execution under a real-time audio deadline, thread-safe handoff between the Keleusma main thread and the SDL3 audio callback, multi-voice control flow through the data segment, hot code swap between two precompiled songs at the reset boundary). The keyboard controls are explicit: `s` then Enter to swap, Enter alone to quit.

### docs/README.md

The Quick Reference table gained three rows pointing at standalone script demonstrations (`examples/scripts/`), Rust embedding examples (`examples/`), and the SDL3 audio demo (`examples/piano_roll.rs`). A user navigating the documentation knowledge graph now finds the example without needing to read the top-level README.

### docs/guide/EMBEDDING.md

The Cross-References section gained a piano_roll entry describing the example's coverage. The Hot Code Swapping subsection had two correctness issues fixed.

The first issue was the `Vm::replace_module` signature in the snippet. The signature is `(new_module: Module, initial_data: Vec<Value>) -> Result<(), VmError>`. The prior snippet showed `vm.replace_module(new_module)` with one argument; the actual call requires both. The example surfaced this when implementing the swap.

The second issue was the post-swap protocol. The prior snippet showed `vm.resume(...)` after `replace_module`. The actual contract is that `replace_module` clears coroutine state, sets `started = false`, and the host must invoke `Vm::call` to start the new module's entry point. The corrected snippet uses `vm.call(...)` and explicitly comments the contract.

Both bugs in the doc were latent because no example exercised the code path; the new piano_roll example forced the correct usage and revealed the doc bug. This is the kind of latent inaccuracy that examples are particularly good at catching.

### docs/guide/GETTING_STARTED.md

The Next Steps list gained a piano_roll bullet with the feature-gated run command and the keyboard controls.

### docs/guide/README.md

The Companion Material table gained a piano_roll row.

## Trade-offs and Properties

The decision to surface the example from four documentation entry points (top-level README, docs index, embedding guide, getting-started guide) reflects that different users start from different places. A first-time visitor reads the README. An embedder reads the embedding guide. A user navigating the knowledge graph starts at docs/README.md. Pointing at the example from each location keeps the friction low regardless of starting point.

The decision to keep the keyboard controls visible in the top-level README, rather than only in the file header and runtime banner, makes the example self-documenting at the level a hesitant new user reads first. Visibility costs about three lines and removes a class of "I started it but did not know how to interact with it" failure modes.

The decision to fix the embedding guide's hot-swap snippet, rather than just point at the example for the correct usage, was driven by the snippet being wrong. Pointing at the example without fixing the snippet would leave a documented incorrect API call in the guide. Fixing it once at the source is the right move.

## Files Touched

- **`README.md`** (top-level). Examples section description refreshed to mention hot code swap and the four principal capabilities. Keyboard controls explicit.
- **`docs/README.md`**. Quick Reference table gained three rows pointing at the scripts directory, the embedding examples directory, and the piano_roll example.
- **`docs/guide/EMBEDDING.md`**. Cross-References section gained a piano_roll entry. Hot Code Swapping subsection corrected (signature, post-swap call, native persistence note).
- **`docs/guide/GETTING_STARTED.md`**. Next Steps list gained a piano_roll bullet.
- **`docs/guide/README.md`**. Companion Material table gained a piano_roll row.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T47 plus history entry.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The documentation now surfaces the example from each entry point and the embedding guide's hot-swap section is correct. Remaining priorities are unchanged from the prior tasks.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session closed a documentation drift introduced when the example was extended with hot code swap in V0.1-M3-T46. The example surfaced two latent bugs in the embedding guide's hot-swap snippet (missing `initial_data` argument; `resume` instead of `call`), both of which are now fixed at the source. Cross-references from the top-level README, the documentation knowledge graph index, the getting-started guide, the guide index, and the embedding guide ensure the example is reachable from every documentation entry point a new user might start from.
