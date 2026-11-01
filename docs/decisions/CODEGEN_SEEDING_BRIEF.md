# BRIEF — absorption 14, and what `codegen.kel` needs

**Written**: 2026-08-26, fifth loop iteration. **For this line's own use.**

## Goal 1 — absorption 14 (#284, Call-record chunk radix)

`+1` function in `parse.kel` (`call_chunk_radix`) and `+1` in `reconstruct.kel`
(`rc_call_chunk_radix`). Net **+2**, so `bound_transfer` is predicted at **1058** from 1056 — the
prediction is recorded here **before** the measurement, as it has been for absorptions 11, 12 and 13.

**Measure it ALONE.** Anything else in flight that could move a gate figure must be held out of the
tree, by stash if necessary. That was enforced rather than intended at absorption 13 and it worked.

The range also carries `wire.kel compiles -- 486 chunks, and NOT byte-identical`, which is the other
line's finding about their own file. `wire.kel` is **exempt** from this line's Order-1 gate, so no
figure here should move on account of it. **If one does, that is the finding.**

## Goal 2 — establish what `codegen.kel` needs. It is NOT more of the same.

`codegen.kel` is the last unseeded stage, and **its shape differs from the two seeded today**:

| | `analyze.kel` / `verify_yield.kel` | `codegen.kel` |
|---|---|---|
| entry | `loop main(resume) { yield run() }` | `loop main(resume) { yield emit_next(resume) }` |
| work per tick | the WHOLE fixpoint, once | **one emission**, resume-driven |
| input | flat parallel op tables | **an AST**: `root`, `kinds`, `args`, `lhs`, `rhs` at 1024 each, plus per-function side arrays at 256 |
| tick budget | irrelevant | **load-bearing** — the stream advances one step per tick |

**So the two facts that made the last two seedings cheap do not carry over.** The op-table encoding
was flat and documented; an AST is a *structure*, and a malformed one is far likelier to produce an
early exit that looks like a result.

> **DO NOT ASSUME THE TICK BUDGET IS A NON-ISSUE HERE.** It was a non-issue twice, for a reason that
> does not apply: those stages did all their work inside a once-per-tick `run()`. **This one does
> not.** Establishing that difference is most of what this goal is.

## Prior failures this is exposed to

1. **Reading a number without establishing its scope** — the `0..16384` cap. Corrected once today;
   the analogous trap here is assuming a tick count from the other two stages.
2. **A truncated read reported as a complete list** — committed today on `analyze.kel`'s slot block.
   **Read the whole `shared data ast` block and say how many slots it has.**
3. **The already-holds-the-answer trap** — `analyze.kel`'s `out_valid` was already 1 unseeded.
   **Measure the unseeded baseline of every candidate observable before asserting on it.**
4. **The truncated fold** — a stage that stops early compares a prefix while reporting as seeded.
5. **Subject-shopping** — report what does not drive the stage.
6. **Sharing a measurement between two changes.**
7. **Running the two suites in parallel** — invalidates the perf canary. Sequential.
8. **Citing a name that does not exist** — the citation guard fired twice today.

## Specific wrong turns to avoid

- **Do not edit `src/selfhost/`** or the other read-only files. A blocked conclusion is a deliverable.
- **Do not widen the shared one-array seed helpers.** Additive builders only, as for the last two.
- **Do not hand-build an AST by guessing node encodings.** `kinds`/`args`/`lhs`/`rhs` are a
  structure with invariants; read how the stage consumes them before writing one.
- **A seeding that half-works and reports as seeded is worse than none.** If `codegen.kel` needs
  more than this increment can give, **say so with the obstacle named** and stop. Two stages were
  seeded today; a third is not owed.
- **Do not declare the Order-1 gate met** even if this succeeds — the gate's own report carries
  qualifications a bare count would drop.
