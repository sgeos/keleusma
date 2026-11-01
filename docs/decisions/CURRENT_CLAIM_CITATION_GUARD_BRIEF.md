# Brief: Extend the Citation Guard to the Current-Claim Documents

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Drafted**: 2026-08-27 (session 55)
**Status**: implemented in the same increment

## The goal

`tests/comment_citations.rs` makes a comment that names a nonexistent identifier fail. It
scans `src/` and `tests/`. **It has never scanned the process documents**, and the handoff is
the most-read document in the repository.

## Why now: it demonstrably let a false claim stand

The handoff's open correctness item 4 asserted that an arithmetic result is still unknown to
the type-rejection rules, citing a pin as evidence. **That test does not exist.** Commit
`63574d1f` had already closed that half with a bounded fixpoint.

Three comments under `src/` and `tests/` repeated the claim. The guard would have caught
those — **except the name sat in the `UNRESOLVED` excuse register**, which excuses a citation
from being checked, not from being wrong. And the handoff itself was never scanned at all. So
four places asserted something untrue and nothing failed.

## The scoping decision, which measurement overturned

The obvious scope is all of `docs/process/`. **Measured before choosing, and it is wrong:**

| threshold | cited across `docs/process/` | unresolved |
|---|---|---|
| one underscore | 1,179 | 382 |
| two underscores | 656 | 113 |
| three underscores | 306 | 33 |

Broken down at two underscores:

| document | cited | unresolved |
|---|---|---|
| `HANDOFF.md` | 28 | **1**, a cross-line reference |
| `REVERSE_PROMPT.md` | 1 | 0 |
| `TASKLOG.md` | 317 | **63** |

**`TASKLOG.md` and `DESIGN_JOURNAL.md` are APPEND-ONLY.** They record what was true at the
time and legitimately name things that no longer exist — that is what a historical record is
for. Guarding them would need a sixty-entry excuse list on the first run, which is *answering
a guard by widening the excuse*, the exact failure this increment exists to correct.

**The two documents that are overwritten each session carry only CURRENT claims.** That
property is what makes a dead citation in them a defect rather than a fact of history.

## The cross-line escape, kept to one entry

`alloc_format_kind` exists only on `origin/v0.3.0`. **A test cannot consult another branch**,
so a cross-line reference is indistinguishable from a dead one. One allowlist entry, with the
evidence, and a note not to grow it to silence a failure.

`slot_entry` falls below the two-underscore threshold and needs no entry.

## Prior failures this work must not repeat

- **A guard that manufactures its own findings is worse than no guard.** This file already
  learned it once, from an identifier wrapped across two comment lines. **It happened again
  here**: the first run flagged four corpus SCRIPT FILENAMES — `12_sensor_window` and its
  siblings under `examples/scripts/` — because the filter allowed a leading digit and an
  identifier cannot start with one. Both instances are recorded in the guard's own comment.
- **A guard that has not been made to fail is a guess.** Mutation-tested by adding a dead
  citation to a current-claim document: it fails, and names the document and the identifier.
- **Assert the scan is non-vacuous.** Two derivations in this repository have passed while
  finding nothing.
- **"Their" is INDEXICAL.** A name in the handoff prefixed by "their" belongs to the other
  line's tree, and this repository has already escalated once on an inverted reading of that
  possessive.

## The specific wrong turns to avoid

1. **Do not scan the append-only documents.** Their dead names are history, not defects.
2. **Do not lower the threshold to catch more.** At one underscore the scan is dominated by
   prose words and foreign symbols; a guard drowning in false positives gets ignored.
3. **Do not grow `CROSS_LINE` to silence a failure.** A name belonging to this tree that does
   not resolve is a stale claim.
4. **Do not treat the existing `UNRESOLVED` register as a baseline.** It is a debt register,
   and this increment shrank it from 13 to 12 by resolving one rather than excusing another.
