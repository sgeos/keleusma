# BRIEF — absorption 15, and sweeping the recorded blockers

**Written**: 2026-08-27, fourteenth loop iteration. **For this line's own use.**

## Goal 1 — absorption 15 (#294)

**`wire.kel` self-compiles byte-identically.** `wire.kel` is currently **EXEMPT** from this line's
Order-1 gate, so this absorption is likely to move gate figures — the exempt count, possibly the
stage-source count. The range also adds 8 lines to `parse.kel`.

**Measure it ALONE**, stashing anything else in flight. That discipline has held for absorptions 13
and 14 and caught a conflation both times.

**Predict before measuring**: `parse.kel` gains no top-level `fn` (the diff is 8 lines with no `+fn`
seen), so `bound_transfer` is predicted **unchanged at 1058**. If it moves, the prediction was wrong
and that is the finding.

## Goal 2 — the stale-blocker sweep, and why it is now the priority

**THREE recorded blockers expired unnoticed in this session alone:**

| blocker | why it expired |
|---|---|
| `seed_reconstruct_shared` "cannot be built without the field accessors" | `ParsedFn::body_records()` is public and returns exactly those |
| the single-head reconstruct route "stays blocked" | same cause |
| `FixedDiv` "needs the runtime-fault lowering, deferred to V0.4.0" | `Op::Div` had already built that path |

**Each was accurate when written.** Each outlived its reason because the thing it waited on got built
**for a different purpose** and nobody came back. **That is a distinct failure mode from a stale
figure and a more expensive one: a wrong number misleads a reader, a wrong blocker stops work.**

**Three in one session is a rate, not a run.** The population is measurable: **55 blocker-shaped
claims** in this line's own sources, of which roughly two dozen carry explicit "blocked on", "blocked
by", "defers to" or "cannot be" phrasing.

**The sweep**: enumerate them, and for each decide — still true, expired, or not checkable from here.

## Prior failures this is exposed to

1. **A vacuous selector.** Nine filters or guards have broken this session, twice by sharing a
   namespace with grep's own output. **Prove it discriminates.**
2. **Confusing a blocker with a decision.** "Refused by specification" is not a blocker — e.g.
   `verify_datalayout.kel` is blocked BY DESIGN and must not be reported as expired.
3. **Reporting a blocker as expired without checking the thing it waits on.** The check is: does the
   named dependency now exist? Not: does the sentence look old?
4. **Sweeping too wide.** 55 is the shape-matched population; the checkable subset is smaller and
   stating which was examined is what makes the result honest.
5. **Conflating populations** — this line's files against the whole tree.
6. **Reporting a figure without the command that produces it.**
7. **Running the two suites in parallel** — invalidates the perf canary. Sequential.

## Specific wrong turns to avoid

- **Do not edit `src/` or any read-only file.** An expired blocker owned by the other line is a
  finding to report, not to fix.
- **Do not act on every expired blocker found.** Finding them is this increment; unblocking one is
  its own increment with its own evidence. `FixedDiv` took a full iteration.
- **Do not mark a blocker "still true" because its sentence reads confidently.** That is exactly how
  all three survived.
- **Do not delete a blocker note that has expired.** Correct it in place and say what changed, so the
  next reader sees the expiry rather than a silent edit.
