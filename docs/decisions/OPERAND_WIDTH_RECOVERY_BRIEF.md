# BRIEF — recover the operand width that blocks the last two composite sites

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Absorption 20 (`2f9be99a`, op-tag table agreement) | yes |
| 2 | Name the PRODUCER of the unknown operand width at the two refused sites | yes |
| 3 | If the module's own tables can supply that width soundly, use them and raise coverage | yes, conditionally |
| 4 | Keep the gate green, running the gate rather than `cargo test` | yes |
| 5 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |
| 6 | Lower `Stream` | not in one increment |

## Rationale

Corpus coverage is **1070 of 1074 chunks**. The largest remaining group is `NewComposite`, at two
chunks: `12_sensor_window.kel::main` op 23 and `14_frame_log.kel::main` op 24. Both are refused for
**unknown operand width**, already narrowed from the `Flat` arm's three possible causes.

**What is known and must not be re-derived from scratch**: `NewComposite::Flat` carries the total
body size, not the per-field breakdown, so packing `count` popped values needs each operand's width.
`StructTemplate` carries field NAMES only, so it cannot supply widths. The corpus contains **zero**
non-`Flat` composites, so the `Boxed` form is not involved — that was measured and is not a
hypothesis to re-test.

**What is NOT known, and is the whole point of this increment**: *which operand* is unknown, and
*which op produced it*. "Unknown operand width" is the condition, not the cause. Naming the producer
is what turns this from a count into something fixable.

**The live hypothesis, to be tested rather than assumed**: `12_sensor_window.kel` calls `scale()`,
and a `Call` result's width is a plausible thing for the tracker to have lost. The module carries
`signatures: Vec<ChunkSignature>`, whose `ret: WireShape` distinguishes `Scalar { kind }` from `Flat`
from `Top`. If `Call` results are what go unknown, that table can seed them — it is the same table
the typed verifier uses for exactly this purpose, and the previous increment already used it
successfully for the interprocedural residual.

## Prior failures to avoid repeating

1. **Two recorded premises were false in consecutive increments** — "no corpus module has the
   escaping shape" and "the gate does not cover `native_codegen`". **Re-derive; read the file.**
2. **A plausible hypothesis was expensive to assume and cheap to refute** — the `Boxed` guess. Probe
   before building.
3. **A guard passed for weeks on a stale value it never validated.** Any new assertion must be shown
   able to fail.
4. **`cargo test` was mistaken for verification.** `clippy -D warnings` and `cargo doc -D warnings`
   each caught a real defect that tests could not see.
5. **A pinned expectation was guessed rather than measured** (`(0,0)` for the interprocedural
   residual, which was `(0,2)`). Pin what is measured, not what is hoped.

## Specific wrong turns to avoid

- **Do not widen an operand by guessing.** A wrong width mispacks the body silently, and a `Byte` and
  a `Word` are indistinguishable on the stack. That is the exact failure the current refusal exists
  to prevent, so a fix that guesses is worse than the refusal.
- **`WireShape::Top` is UNKNOWN and must stay unknown.** Seeding it as a default width would convert
  a missing table entry into a silent mispack. Only `Scalar { kind }` may supply a width.
- **Do not treat "the refusal moved" as "the chunk lowers".** If seeding removes this refusal and a
  different one fires, coverage may not move at all. Report the refusal text after the change, not
  just the count.
- **Verify against the canonical flat layout**, which packs cumulatively with no padding. A width
  that satisfies the tracker but disagrees with the layout is a miscompile.
- **Do not claim a coverage figure that was not re-derived.** `1070 of 1074` must be re-measured
  after any change, and re-stamped even if unchanged.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
