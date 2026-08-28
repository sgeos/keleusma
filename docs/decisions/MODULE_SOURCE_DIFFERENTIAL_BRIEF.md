# BRIEF — a source-string whole-module differential, and what it unblocks

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | A differential that runs a MULTI-FUNCTION program written as a source string | yes |
| 2 | Use it to verify the chunk-call width seeding, and re-land that seeding if it agrees | yes, conditionally |
| 3 | Keep the gate green, running the gate rather than `cargo test` | yes |
| 4 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |
| 5 | A fixpoint over local widths (the actual composite blocker) | next increment, not this one |
| 6 | Lower `Stream` | not in one increment |

## Rationale

The previous increment implemented a sound change — seeding a chunk-call result's packed width from
`Module::signatures`, the exact analogue of what the native path already does — and **reverted it,
because nothing in the tree could execute it**. That is the correct disposition and it names its own
remedy: `lower_chunk` refuses `Op::Call` outright, so the source-string differential cannot express a
program with a call; the whole-module differentials exist but are driven from files.

**The missing capability is narrow**: compile a source string, lower the WHOLE module, JIT it, call
the entry chunk, and compare against the virtual machine running the same module. Both sides already
exist in the tree — `lower_module` and the four-pointer entry ABI on one side, `Vm::call` on the
other. Nothing new is being invented; two halves are being joined.

**Once it exists the seeding is a one-line change with a test**, and every future multi-function
question becomes answerable the same way. That is why this is worth an increment of its own rather
than being folded into the change it verifies.

## Prior failures to avoid repeating

1. **A harness compared two different functions and reported agreement about nothing.** The existing
   source-string harness lowers `chunks[0]` while the virtual machine calls the ENTRY POINT; a
   two-function fixed-point test passed by mathematical accident. That precondition is now asserted
   there, and the new harness must not reintroduce the same class of mismatch.
2. **A uniform offset error inside a region is value-invariant** — reads and writes shift together,
   so a round trip returns the right answer and a value comparison sees nothing. The existing module
   differential guards this with canary words past every caller-provided buffer. **Omitting canaries
   would make this harness able to pass while the lowering writes out of bounds.**
3. **An ABI mismatch manifests as SIGSEGV inside JIT code**, with no stack and no indication of which
   side is wrong. The existing harness asserts the emitted parameter count before calling through.
4. **Three recorded premises were false in consecutive increments.** Re-derive; prefer a published
   table to a heuristic.
5. **`cargo test` was mistaken for verification.** Lint and documentation checks each caught a real
   defect tests could not see.

## Specific wrong turns to avoid

- **Do not size the region by summing every chunk's plan.** `region_total_bytes(m, entry, 0)` is the
  backend's own demand and accounts for the disjoint per-call-site blocks; a sum is a second opinion
  and could under-provision.
- **Do not skip the canaries or the parameter-count assertion** because the test program is small.
  Those guard the two failure modes a value comparison cannot see.
- **Do not assert the entry chunk is index 0.** Compare what the virtual machine runs against what is
  called natively, by construction rather than by assumption.
- **Do not claim the seeding is verified because a test passes.** It is verified only if the test
  FAILS without it. Demonstrate that, or the test may be passing for unrelated reasons.
- **Do not widen the accepted set on the strength of one program.** If the seeding is re-landed,
  re-derive the corpus coverage figures and state whether they moved.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
