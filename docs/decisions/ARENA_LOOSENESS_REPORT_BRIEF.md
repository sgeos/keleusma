# BRIEF — report the arena looseness, and guard the report

## Why this rather than the remaining sixteen modules

The obvious next increment was to drive the other sixteen streaming corpus
modules and measure their arena extent dynamically. **I am not doing that, and the
reason is a cost judgement that belongs in the record rather than in my head.**

- The machinery that drives corpus modules — stub tables, per-module seeds,
  argument vectors, entry-signature dispatch — is private to
  `corpus_differential`, whose phase already runs **380 seconds** and had to be
  SPLIT to fit the harness ceiling in the narrow configuration. Extending it
  lengthens the most expensive phase in the gate.
- Duplicating that machinery into a lighter binary would be **a second
  computation of the drive contract**, free to drift from the first — the exact
  failure this package cites when it refuses to re-derive a quantity.
- **The qualitative answer is already established twice, independently.** Three
  hand-written shapes touch 16, 24 and 48 bytes of 520-552 byte plans; the twelve
  self-hosted stages touch **zero** of 520-600. Sixteen more modules would add
  breadth to a conclusion that two disjoint methods already agree on.

**The marginal evidence does not justify lengthening the gate's worst phase.**
That is a judgement, not a proof, and it is written down so it can be overturned.

## What this increment does instead

**Tells the other line.** The finding currently lives only in this line's own
documents: `host_arena_supplement_bytes` — the figure a host is told to add to its
arena — is between 27% and 98% flat reservation across 28 streaming modules, and
for the twelve compiler stages the whole plan goes untouched. **That is
information the `v0.2.3` line and the operator need for an arena-accounting
decision neither this line nor that one can make alone.**

## The discipline this must follow

**Every report this line files carries a guard**, because an un-retracted report
becomes an accusation and an unacknowledged repair is the same failure with the
sign flipped. `outstanding_reports.rs` fails when a report stops reproducing and
says to RETRACT rather than debug. **A sixth report joins that register or it does
not get filed.**

## Prior failures to avoid

- **Crediting this line's own work to the other line.** That happened in this very
  channel two days ago: the addendum said *"your `linkage_symbol_census`"* when the
  census lives entirely in `native_codegen/`. **The spill reservation is THIS
  line's code** — `stream_spill_bytes` is in `native_codegen/src/region.rs`. The
  report is therefore not an accusation at all; it is a disclosure about a figure
  this line publishes, which the other line's hosts would consume.
- **Bare backticked identifiers the root citation scan cannot resolve.** That scan
  does not walk the detached package, and it caught this line twice. Name files by
  path and symbols as prose.
- **Overstating.** Over-provisioning is safe. The reservation is fixed, static,
  documented, and its rationale stands. **The report is a quantification, not a
  defect claim**, and must say so in its first sentence.
- **Asserting what the measurement does not cover.** These are static plans and
  dynamic runs on particular inputs. Neither bounds what an arbitrary program
  would touch.
