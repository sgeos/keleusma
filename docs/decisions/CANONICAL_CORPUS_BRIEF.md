# BRIEF — a habit is not a check, applied to the population

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Why this, and why now

Five defects on this line have had one shape: **a measurement enumerated a narrower population than the
thing it described**, and reported the difference as a property of the subjects.

| | |
|---|---|
| a sweep read 35 modules where its consumers read 74 | the walk was not recursive |
| a fingerprint covered 3 roots where consumers read 4 | roots listed by hand |
| a probe keyed modules by file name | two files named `prelude.kel` merged |
| a rogue directory was double-counted | listed explicitly *and* reached by recursion |
| the detection census drove subjects unseeded | a weaker driver than the harness it described |

`corpus_fingerprint.rs` already closed the neighbouring hole — the corpus **content** — and its header
carries the argument for this one verbatim: *"It has never bitten here, because every absorption asks
'corpus inputs touched?' by hand … **A habit is not a check.**"* The same sentence is true of the
population, and the population is where I keep failing.

## The fix is structural, not another guard

The shared probe made two censuses agree **by construction** rather than by comparison, and that
worked. The same move applies here: **one canonical enumeration** in `tests/common/mod.rs`, which
integration binaries can already include, so a migrated sweep *cannot* read a different set.

## Wrong turns to avoid

- **Do not migrate on assumption.** Before switching an existing sweep, **assert the canonical
  enumeration returns exactly what that sweep returns.** A silent change to the biggest census's
  population would be the very defect this closes, committed while closing it.
- **Do not migrate all forty files.** Most reference a single corpus file rather than sweeping.
  Rewriting them is regression risk with no evidence behind it, and this line has already been caught
  manufacturing a finding from a file-count statistic.
- **Do not enumerate differently from `source_for`.** Enumeration and loading are separate; some rtos
  scripts are loaded with a prelude prepended. Unifying the walk must not disturb the loading.
- **Do not claim the class is eliminated.** It is eliminated *for migrated callers*. Files still
  carrying their own walk remain exposed, and the honest statement names how many were migrated.
- **Do not add a grep-based lint** asserting no file has its own walk. This line has already shipped a
  scanner that counted 33 where the truth was 10 by matching a word inside comments.
- **Do not let the canonical count become an unfloored figure.** It is a population; pin it the way the
  fingerprint pins content.

## What good looks like

One enumeration, proven equal to the largest existing sweep before replacing it, used by the sweeps
this line authored, with the number of migrated and unmigrated callers stated rather than implied.
