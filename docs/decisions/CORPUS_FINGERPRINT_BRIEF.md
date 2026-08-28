# BRIEF — turn the widest-input habit into a check

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Make a change to the shared corpus announce itself, rather than relying on the absorption habit | yes |
| 2 | Keep the gate green and the branch published | yes |
| 3 | Absorption 27 if anything lands | opportunistic |
| 4 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |
| 5 | General `Stream` lowering | not in one increment |

## Rationale

The `v0.2.3` line's rule, derived from a pin of theirs that failed here: **before pinning a value, ask
what the widest input to it is and whether that input is pinned too.**

Applied here it named a real exposure. **Thirty-six test files on this line read
`src/selfhost/kel/` and `examples/scripts/`**, and those directories are **owned by `v0.2.3`**. Every
corpus-derived figure — coverage, refusal counts, the interprocedural residual, the yield-escape cost
— rests on inputs another line commits to.

**It has never bitten, because every absorption asks "corpus inputs touched?" before predicting.**
That is the widest-input question, asked by hand, twenty-six times. **A habit is not a check.** It
holds exactly as long as the person doing it remembers why, and the handoff already records that this
line has forgotten recorded rules before — including one this repository had written down.

**The increment is to make the input announce its own change**, so a corpus edit produces a loud,
specific failure naming what moved, instead of a quiet drift in figures that are re-derived only when
someone thinks to.

## Prior failures to avoid repeating

1. **A test named for a canary firing did not make it fire.** Do not name this for a property the
   body does not establish.
2. **A guard can be unfalsifiable by its own precondition.** A fingerprint test that recomputes the
   expectation from the same scan it is checking would pass unconditionally. **The expected value
   must be a constant in the file.**
3. **A directory scan is not a corpus; it is whatever the branch happens to contain.** That is the
   defect being guarded against, so the guard must not reintroduce it.
4. **`DefaultHasher` is not stable across toolchains.** A pin that changes when Rust updates is a
   tripwire that cries wolf, and a tripwire nobody trusts is worse than none.
5. **Nine recorded premises have been found false in consecutive increments.** Write the prediction
   down first.

## Specific wrong turns to avoid

- **Do not hash with a non-deterministic or version-dependent hasher.** Use an algorithm whose output
  is fixed by its definition, so the only thing that can move the pin is the corpus.
- **Do not fingerprint content alone.** A file added and another removed can leave a content digest
  unchanged in principle; include names and the count so an identity change is visible.
- **Do not make the failure message a bare number.** The value of this guard is that it says *which*
  files moved and *what* to re-derive; a mismatch reporting only two integers puts the reader back
  where they started.
- **Do not include files the pins do not read.** Fingerprinting more than the inputs makes the guard
  fire for irrelevant reasons and trains the reader to ignore it.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
