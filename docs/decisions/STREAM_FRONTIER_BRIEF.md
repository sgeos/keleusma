# BRIEF — where the stream frontier actually lies

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Absorption 24 | yes |
| 2 | Map which `loop`/`yield` shapes the backend lowers and which it refuses | yes |
| 3 | Say what that means for the yield-escape gate, which is shadowed by the `Stream` refusal | yes |
| 4 | Keep the gate green, running the gate rather than `cargo test` | yes |
| 5 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |
| 6 | Implement general `Stream` lowering | not in one increment |

## Rationale

The previous increment found that **a minimal `loop main(t: Word) -> Word { yield t }` lowers with no
refusal**, while `13_telemetry_stream.kel` is refused for `Stream`. Both are streams. **So "the
backend does not support `Stream`" is false as stated, and "the backend supports `Stream`" is equally
false.** The frontier lies somewhere between, and nothing in the tree says where.

That matters beyond tidiness for two reasons.

**First, the yield-escape refusal is shadowed.** It exists to stop a silently wrong value when a
loop-body composite is yielded, and it cannot fire today because the modules that would trigger it are
refused for `Stream` first. **Knowing which stream shapes lower tells us how close that shadow is to
lifting**, and the tripwire already in the tree fires when it does.

**Second, this line has twice described the frontier wrongly** — `Stream` called unsupported outright,
and `Reset` predicted unreachable. Both were corrected by measuring one program. A matrix is the
cheap way to stop guessing.

## Prior failures to avoid repeating

1. **The brief's own guess was wrong twice in a row.** Write the prediction down, then measure it.
2. **A census surveys a population.** "Never visited" meant "not in the corpus". State which
   population any figure describes.
3. **A gate run killed by a signal, and another by a tool cap, both reported plausible numbers.**
   Read the exit status; background the long suite.
4. **`stack_growth`/`stack_shrink` are the peak model**, not pop and push counts.
5. **Coverage is not correctness.** Execution agreement is the evidence.
6. **Six recorded premises have been found false in consecutive increments.** Re-derive.

## Specific wrong turns to avoid

- **Do not infer the frontier from reading the lowering code.** The point is to find out what it
  actually does, including where the code and its comments disagree. Read enough to choose the axes,
  then measure.
- **Do not report "lowers" as "works".** A shape that lowers must also execute and agree before it is
  described as supported; the suspension differential exists for exactly this.
- **Do not conflate a reference rejection with a backend refusal.** A shape the type checker refuses
  never reaches the backend and says nothing about it. Name the stage.
- **Do not enable `delegated_suspension` to widen the matrix.** It is an explicit opt-in whose meaning
  is that unexecuted lowering is acceptable for the module in hand; turning it on to make a table
  fuller would misreport the default frontier.
- **Do not claim the yield-escape gate is closer to firing than measured.** If composite-yielding
  streams remain refused, say so.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
