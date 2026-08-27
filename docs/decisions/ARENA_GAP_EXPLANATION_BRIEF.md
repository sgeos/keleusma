# BRIEF — what actually explains the arena-bound gap

**Written**: 2026-08-27, ninth loop iteration. **For this line's own use.**

## Where this stands

Workstream E's gap: **11 modules demand more arena than their verified figure**, with
`backend = sites × size` and `verified = peak_live × size`.

**Last iteration ruled out the obvious explanation.** Confinement does not account for it: within the
only family that exceeds, the exceeding members are *more* confined than the compliant ones, and the
whole apparent effect is family rather than exceeding.

**So the question is open, and the arithmetic points somewhere specific.**

| module | sites | size | backend | verified | implied `peak_live` |
|---|---|---|---|---|---|
| `rogue_combat` | 4 | 16 | 64 | 16 | **1** |
| `rogue_player_ai` | 5 | 24 | 120 | 24 | **1** |
| `rogue_ai_boss` | 4 | 24 | 96 | 48 | **2** |

**Sites are four or five; peak-live is one or two.**

## The hypothesis, and it is already confirmed on one module by reading

`rogue_combat::main` returns `(Word, Word)` and constructs that tuple at **four** sites — `(0, 0)`,
`(2, dmg)`, `(1, dmg)`, `(0, 0)` — each in a **different arm of nested conditionals.** Exactly one
runs. Peak live is 1; the backend allocates 4 × 16.

> **The backend SUMS over static sites; the verifier takes the PEAK over live values. Where sites sit
> on mutually exclusive paths, the sum exceeds the peak by construction.**

That names a different remedy from confinement entirely: a planner that takes a **max across
exclusive arms** rather than a sum. **Proposing that is not this increment. Establishing whether it
explains the gap is.**

## The measurement

For each exceeding module, compute the **maximum number of construction sites on any single
control-flow path**, and compare it to the implied `peak_live`.

- **If they match**, branch exclusivity explains the gap completely.
- **If path-max still exceeds peak_live**, something else contributes and the residue is named.
- **If path-max equals the raw site count**, the sites are NOT exclusive and the hypothesis is wrong.

**All three are publishable.** The third would refute a hypothesis I have already half-confirmed by
reading, which is exactly why it must be measured rather than asserted.

## Prior failures this is exposed to

1. **Confirming a hypothesis formed by reading one example.** `rogue_combat` motivated this; it
   cannot also be the evidence for it. **Measure the whole exceeding set.**
2. **Overclaiming a remedy.** No planner change is proposed and none may be implied.
3. **A vacuous instrument** — four filters or guards have broken this session. **Show it
   discriminates**: a module whose sites are sequential must report path-max equal to its site count.
4. **Conflating populations** — corpus-wide against exceeding-only. On record as a repeated failure.
5. **Pinning a distribution that corpus growth moves.**
6. **Reporting a figure without the command that produces it.**
7. **Running the two suites in parallel** — invalidates the perf canary. Sequential.

## Specific wrong turns to avoid

- **Do not edit `src/`, `src/confine.rs`, or any read-only file.** This is a measurement.
- **Do not change `plan_chunk_region`.** Even if the max-over-arms rule looks obviously right, it is
  a code-generation change with soundness consequences and it needs its own increment and evidence.
- **Do not assume the path walk is exact.** A conservative approximation is fine and must be
  **labelled** — say whether path-max is an upper or lower bound on true simultaneity, or the number
  will be read as exact.
- **Do not treat a loop as a branch.** A site inside a loop body can be live across iterations; that
  is a different question and conflating them would inflate the explanation.
