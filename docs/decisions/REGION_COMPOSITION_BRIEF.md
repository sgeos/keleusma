# BRIEF — what is the published arena figure actually made of?

## The finding this extends

`arena_high_water.rs` measured three stream shapes and found each touches 16, 24
or 48 arena bytes inside a plan of 520, 536 or 552. The dominant term is
`stream_spill_bytes`, a flat `MAX_STACK * 8` = **512 bytes reserved for every
stream chunk**, of which the deepest observed reach was **one eight-byte slot**.

**Three subjects is not a corpus.** The claim as it stands is "these three hand
written streams barely touch the reservation", and a reader is entitled to ask
whether that generalises or whether the three were chosen badly.

## What this measures, and why statically

Decompose `host_arena_supplement_bytes` — the figure this backend publishes for a
host to add to its arena — into the terms `region_total_bytes` actually sums:
composite sites from `plan_chunk_region`, `stream_locals_bytes`,
`stream_spill_bytes`, and the disjoint block each `Op::Call` receives. Report the
share each contributes across the whole corpus.

**Statically, because the question is about the PLAN**, not about a run. How much
of it is used is the dynamic question `arena_high_water.rs` already answers for
three subjects; how much of it is a flat constant is a property of the planner and
needs no execution at all. Keeping them separate also keeps the corpus census
cheap enough to run every gate.

## The check that makes this more than arithmetic

**The decomposition must SUM to the published figure.** If it does not, then
`host_arena_supplement_bytes` contains a term this analysis has not named, and
that is a more interesting result than any share. **Assert the sum, do not assume
it** — the whole finding rests on having understood what the planner adds up, and
an unverified reading of a function is exactly the kind of premise this line keeps
recording as false.

Alignment makes exact equality unlikely, since `region_total_bytes` calls
`align_up` at two levels. **Then assert the bound and the slack separately**: the
named terms must not EXCEED the total, and the unexplained remainder must be small
enough to be alignment rather than a missing term. State the threshold and why.

## Prior failures to avoid

- **A census keyed to what the analyst already noticed.** The first handoff-figure
  guard checked three rows and left three alone, and both unchecked ones were
  stale. **Enumerate the population from the corpus, not from the modules that
  motivated the question.**
- **A module that motivated a hypothesis cannot also be its evidence.**
  `arena_gap_explanation.rs` states this explicitly about `rogue_combat`. The
  three stream shapes motivated this; the corpus is the evidence.
- **Reporting a share without a denominator that means anything.** A module with
  no stream chunk reserves no spill at all, so averaging over all modules would
  dilute the figure into meaninglessness. **Partition by whether the module has a
  stream chunk**, and report the two populations separately.
- **Treating over-provisioning as a defect.** It is not. The reservation is fixed,
  static, documented and safe, and its rationale — that a predicate disagreeing
  with the lowering's own would be worse than unused bytes — stands. The finding
  is the SIZE of the looseness in a bound sold as definitive, not that it exists.

## What must not happen

**Do not change the planner.** There is an open soundness obligation recorded
against it in the handoff, and this increment has no mandate to touch it. This is
a measurement, and the operator's arena-accounting question is the operator's.
