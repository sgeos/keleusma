# BRIEF — measure the sixteen streaming corpus modules' arena touch

**Filed 2026-09-23, against `407b5c59`, backlog 0, suite 651.**

## The gap, stated more precisely than the handoff states it

The pickup list says the sixteen remaining streaming corpus modules "remain
static-only, and driving them needs the `corpus_differential` seeding machinery."

**Re-reading that file narrows the gap considerably.** `corpus_differential.rs`
already drives corpus modules with seeded arguments — `args_for_seed`, 64 seeds,
482 `(module, seed)` pairs — and those sixteen are among them. **They are already
driven for CORRECTNESS.** What they lack is a dynamic measurement of how much of
the published arena figure they TOUCH.

Three populations, three states:

| population | arena touch |
|---|---|
| three hand-written streams | measured — 16, 24 and 48 bytes of 520–552 byte plans |
| the twelve self-hosted stages | measured — **zero** of a 520–600 byte plan |
| **the other sixteen streaming modules** | **static only** |

## Why it is worth closing

`region_composition.rs` establishes statically that a flat `stream_spill_bytes`
reservation is **98% of the published figure for the twelve stages** and 27% for
`piano_roll_0` — so it is not uniform, and the sixteen are unmeasured in between.

**Report six in `REVERSE_PROMPT.md` quotes those static figures to the other
line**, and `outstanding_reports.rs` fails if they drift. A dynamic figure for the
sixteen would make that disclosure stronger, or would show the static share is not
the whole story — which is the more interesting outcome.

## Wrong turns, named

1. **Do not re-implement seeding.** It exists, it is tuned (64 seeds, chosen after
   4 and 24 were shown insufficient), and a second implementation is a second thing
   to drift. Reuse the existing driver's shape.

2. **Do not fill the region with one pattern.** `arena_high_water.rs` uses **two
   complementary patterns** for a stated reason: a byte written with the fill value
   would hide. One pattern silently under-reports.

3. **Do not accept a measurement without showing the instrument can see a touch.**
   Poke one byte into the region and require the reported extent to move. This
   package has a recorded case of a canary that documented its own lack of reach.

4. **Do not silently skip a module that will not run under a seed.** A driver that
   quietly drops subjects reports a figure for a population narrower than its
   description — the failure this session has found five times. **Count what was
   driven and what was skipped, and print both.**

5. **Do not report a changed figure without correcting report six.** It quotes the
   static numbers, and `outstanding_reports.rs` guards them. A disclosure whose
   figures have drifted is worse than none, because the other line would act on
   numbers this line no longer measures.

6. **This is not a defect report.** Over-provisioning is safe and the reservation's
   rationale stands. The finding is the SIZE of looseness in a bound this line
   sells as definitive, and what to do about it is the operator's call.

7. **`src/` and `tests/` at the repository root are read-only to this line.**

## Pre-commitment

- **If the sixteen touch near zero like the stages, say so plainly** and strengthen
  report six with the dynamic figure. That is the expected outcome and it is not a
  disappointment.
- **If any module touches substantially more than its static share predicts, that
  is the finding** — report it with the module and the numbers, and do not adjust
  the instrument until it agrees with the static figure.
- **If the existing driver cannot reach these modules without changes to
  `corpus_differential.rs` itself, stop and report the shape** rather than
  rewriting a tuned instrument late in a session.
