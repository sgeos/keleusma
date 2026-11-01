# Brief — the project instructions state test counts that are three times wrong

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: brief for the increment. Written 2026-08-28, session 56.

## THE DEFECT

`CLAUDE.md` — the most-read document in the repository, and the file an agent reads first — states,
in **two** places:

> Approximately 1168 keleusma lib tests plus **368 integration tests across 30 files** ... 42
> keleusma-arena, and 6 keleusma-bench tests

Measured by RUNNING rather than by grepping, because "tests" in that sentence means tests that run:

| claim | stated | measured 2026-08-28 |
|---|---|---|
| lib tests | 1168 | **1263** (`--features self-host`), 1256 default |
| integration files | **30** | **89** |
| integration `#[test]` functions | 368 | **1192** |
| keleusma-arena | 42 | **51** |
| keleusma-bench | 6 | 6 |

**The file count is three times wrong and the integration count is more than three times wrong.**
"Approximately" cannot carry that.

## WHY IT IS OPERATIONALLY RELEVANT, NOT MERELY UNTIDY

This session already hit a case where the number of test binaries mattered: a killed sweep reported
**55 binaries green while 31 never ran**, and the drift was caught only by enumerating the files. An
agent calibrating against "30 files" would have read 55 as comfortably complete. **A stale count in
the orientation document is a wrong prior for every coverage judgement made against it.**

## HOW IT WAS FOUND

By asking, after correcting the example index, whether any OTHER documentation makes claims that
nothing checks. That is the generalisation of the previous increment rather than a new idea, and it
found the largest instance immediately.

## SCOPE, AND THE ONE SENSITIVITY

**`CLAUDE.md` is the project's instruction file.** Editing it changes what a future agent is told,
including me. So: **correct the FACTUAL counts and nothing normative.** No guidance, no policy, no
process wording. If a normative change ever looks warranted, it goes to the operator instead.

State the numbers the way this repository already states numbers that move — **as a dated
measurement with its derivation command** — which is the handoff's own convention and the reason
the handoff's figures have survived where these did not.

## THE GUARD, AND WHY IT MUST NOT BE EXACT

An exact pin would fail on every increment that adds a test, which makes it a nuisance that gets
deleted. **Use a tolerance**: gross drift is the defect, not movement. Today's numbers pass at zero
drift; the uncorrected text fails by a factor of three, which is the demonstration that it fires.

## PRIOR FAILURES THIS INCREMENT MUST NOT REPEAT

**MEASURE THE WAY THE CLAIM MEANS IT.** A grep for `#[test]` counts functions the feature gates may
exclude; the sentence says tests that pass. Both were taken and both are reported, labelled.

**A GUARD THAT CANNOT FIRE IS THE FAILURE THIS REPOSITORY RECORDS MOST OFTEN**, and one bolted onto
a repair is where the temptation peaks. Demonstrate this one against the uncorrected text.

**A MUTATION HAS THREE FAILURE POINTS**: confirm it APPLIED, then that it COMPILED, then believe the
result. Print what changed. BSD `sed` has no `\b` on this machine.

**DO NOT PIN A VALUE WHOSE WIDEST INPUT IS UNPINNED.** The guard reads both the document and the
tree, so both are its inputs.

## THE WRONG TURNS SPECIFICALLY

- **Do not touch anything normative in `CLAUDE.md`.** Counts only.
- **Do not pin exact counts.** They move every increment by design.
- **Do not delete the surrounding prose** describing what the suites cover; only the numbers are
  wrong.
- **Do not gate the guard behind an off-by-default feature.**
