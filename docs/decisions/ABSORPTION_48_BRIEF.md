# Brief — absorption 48, and the first one whose prediction can name the workspace

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-02, night, BEFORE the merge.**

---

## The present goals

| goal | state |
|---|---|
| **absorption 48** | **this brief**, and not started while the sweep runs |
| the mutation sweep re-run | **in flight** on a quiet machine |
| `f16` | blocked on reference `f16` arithmetic; the other line's next rung, after `Text<N>` |
| publication | held |
| everything previously held | **pushed**, `4e1ed31e..61e185c2`, twelve commits, remote verified |

## What absorption 48 actually is

**Measured with the correct scoping, which took two attempts.** The first measurement used
`git diff HEAD origin/v0.2.3` and reported 254 documentation files and 109 backend files. **That is
the two-way difference and most of it is this branch's own divergence, not their work.** Scoped from
the merge base at `29460378`:

| | |
|---|---|
| commits | 12 |
| `src/` | **2** — `vm.rs` (76 lines), `marshall.rs` (15) |
| `tests/` | **2** — `len_flat_array_hazard.rs` (**121 new lines**), `selfhost_codegen.rs` (7) |
| `docs/` | 2 |
| total | **207 insertions, 12 deletions** |

**`len_flat_array_hazard.rs` is theirs, built from this line's report.** It asserts the conditional-
source program is refused **and** that the refusal is the liftable bound one, so lifting the bound
extractor without an `Expr::If` arm fails by name. This line should absorb it and **not** claim it.

## The prediction, recorded before merging

1. **The tree still builds.** Their `vm.rs` and `marshall.rs` changes are additive to a surface this
   backend consumes; **absorption 46 broke the build through two signature changes and no test-count
   prediction could express it**, which is why this clause exists separately.
2. **`native_codegen` is unchanged at 469 passed, 0 failed, 91 binaries**, under default features and
   again under `narrow-float-32`. They touched nothing in that package.
3. **Workspace coverage goes STALE-COMPILED**, and `tools/workspace-coverage.sh` says so by naming
   `src/vm.rs` and `src/marshall.rs`. **This is the first absorption where that question is asked by
   running something rather than by remembering to think of it.**
4. **The differential still agrees.** Their `vm.rs` change alters the reference side, which is half
   the oracle. If any corpus module disagrees, **that is a finding and not a nuisance.**

**Falsifiers, in order of interest**: a build failure (clause 1); any differential disagreement
(clause 4); a `native_codegen` count that moves at all (clause 2).

## Prior failures this is exposed to

**Measuring an absorption with edits in flight.** Absorption 40 did this and the suite reported a
failure naming the editing session's own renamed test. Several tests here read source text from disk,
so it is not merely untidy. **The sweep must finish first.**

**A prediction phrased only as counts.** Absorption 46's build break, and the workspace staleness that
went unasked for many absorptions because every prediction ranged over one package.

**Reporting the wrong scope.** Already happened once in this brief's own preparation, above, and it is
recorded rather than quietly corrected.

## The wrong turns

**1. Do not merge while the sweep runs.** It mutates `native_codegen/src/lib.rs` in place and restores
it; a merge landing mid-run would fight it, and the sweep's own restore check would then be
meaningless.

**2. Do not re-run the workspace suite reflexively before checking.** Ask the guard first. If it says
`STALE-COMPILED`, the re-run is justified; if it does not, the run is ninety minutes for nothing.

**3. Do not present their `Len` ratchet as this line's work.** This line reported the hazard; they
verified it independently with a control, found the `vm.rs` comment false, and built the ratchet.
**The report was mine, the repair and the test are theirs.**

**4. Do not resolve `REVERSE_PROMPT.md` by discarding their message.** It conflicts on every
absorption. This line's message stays on top; theirs is replaced beneath it.
