# One corpus walk, because a habit is not a check

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: closed for the four figure-producing sweeps, 2026-08-29. Twenty-five callers still carry
their own walk and remain exposed** — the scope is stated rather than implied.

## The defect class this closes

Five defects on this line shared one shape: **a measurement enumerated a narrower population than the
thing it described**, then reported the difference as a property of the subjects.

| defect | cause |
|---|---|
| a sweep read 35 modules where its consumers read 74 | the walk was not recursive |
| a fingerprint covered 3 roots where consumers read 4 | roots listed by hand |
| a probe keyed modules by file name | two files named `prelude.kel` merged |
| a rogue directory counted twice | listed explicitly *and* reached by recursion |
| the detection census drove subjects unseeded | a weaker driver than the harness it described |

`corpus_fingerprint.rs` closed the neighbouring hole — corpus **content** — and its header carries the
argument for this one: *"A habit is not a check."*

## The fix is structural

One `corpus_sources()` in `tests/common/mod.rs`, which integration binaries already include. A migrated
sweep **cannot** read a different set. This is the same move that made two mutation censuses agree by
construction rather than by comparison.

## Migrated, each licensed by a comparison rather than by inspection

| sweep | figure it produces |
|---|---|
| `corpus_differential.rs` | modules executing and agreeing |
| `spike_corpus_coverage.rs` | chunks lowerable, opcode instances |
| `isa_lowering_census.rs` | opcodes lowered, NAMED REFUSED |
| `refusal_classes.rs` | the refusal-sentence sweep |

**Every one of the four figures this line reports each increment now rests on the canonical walk.**

For each, a test asserts the shared enumeration returns exactly what that sweep's private walk
returned, and those tests remain as standing checks. **Migrating on the assumption that two walks agree
would have been the very defect being closed, committed while closing it.**

`isa_lowering_census` keeps its `CORPUS_DIRS` constant because it **prints** it; a printed root list
that no longer describes what was read is a quieter version of the same defect, so the retained
constant is compared against the canonical walk rather than trusted.

## What is NOT closed

**Twenty-five test files still carry their own multi-root walk.** They are mostly the `spike_*` and
`probe_*` exploratory genre plus several censuses this increment did not touch. The class is eliminated
**for callers of the shared function**, not repository-wide, and claiming otherwise would be the
overclaim `corpus_fingerprint` exists to prevent.

**No grep-based lint was added** asserting the absence of private walks. This line already shipped a
scanner that counted 33 where the truth was 10 by matching a word inside a comment; a second one is not
the answer.

## Enumeration, not loading

Only the walk is unified. Loading stays where it was — some sources are read with a prelude
prepended — and the migration does not touch it.
