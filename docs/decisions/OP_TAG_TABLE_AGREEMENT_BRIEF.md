# Brief — the op-tag tables agree, and the agreement is checked

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: brief for the increment. Written 2026-08-28, session 56.

## The finding, and where it came from

The `v0.3.0` line handed this line a finding it cannot close itself:

> `codegen.kel`'s 63 inline op tags and `decode_op`'s mapping are two independently
> hand-maintained tables of the same numbers. Their guard, `all_wire_op_tags_decode`,
> asserts only that `decode_op` does not panic over `1..=63`. A transposition passes it.

It is **not closable from their side**: `decode_op` is private and `src/selfhost/mod.rs` is
read-only to them. It is unrecorded in this line's task log. Their own framing is careful and
should be preserved: **the claim is about what is CHECKED, not about what is wrong.** No
disagreement has been observed.

## What I verified before writing this, because the brief must not be a guess

1. **The `codegen.kel` table is real and explicit.** `const data wire` at line 154 assigns 63
   named tags, `konst: Word = 1` through `shr: Word = 63`, plus non-tag fields (`radix`, `item`,
   `pack`, work-item kinds, category codes) sharing the same block.
2. **The guard is exactly as described.** `src/selfhost/mod.rs:6220` loops `1..=63` and discards
   the result. It cannot distinguish a correct table from a permuted one.
3. **THERE ARE THREE TABLES, NOT TWO.** The peer counted the emitter and the shipping decoder.
   There is also `tests/selfhost_codegen.rs::decode_op`, and `src/selfhost/mod.rs` names it in a
   comment as the source it was **"ported verbatim"** from and is **"kept in lockstep with"**.
   Nothing checks the lockstep. That is the `five defects, one cause` shape verbatim — the driver
   and its test-file copy drifting apart — and it is the more dangerous of the two, because the
   copy is what the differential oracle runs.

## What the differential oracle already covers, stated honestly

This is the part that keeps the increment from overclaiming.

- A transposition in **`codegen.kel` alone** changes the emitted op-word, the decoder produces a
  different `Op`, the module bytes differ from the reference, and **the byte-identity corpus
  catches it** — for any tag the corpus exercises.
- A transposition in **either decoder alone** is caught the same way.
- A **consistent** renumbering across the emitter and the decoders composes to the identity. The
  op-word is internal to the pipeline and is not a wire format, so a consistent renumbering is
  **semantically harmless** and must not be reported as a defect.

**So the real exposure is the tags the corpus does not exercise**, which is this line's standing
lesson in a new costume: *any construct the corpus does not contain is unverified by
construction*. That is why this increment is a census and not only a guard.

## The work

1. **The emitter's table is a bijection onto a contiguous `1..=63`.** A duplicate or a gap fires.
2. **The two Rust decoders agree tag for tag.** Extracted from source and compared.
3. **A name correspondence**, linking each `wire` field name to the `Op` the decoders produce for
   its number, asserted total over all 63. This is what catches a **one-sided** transposition,
   which neither (1) nor (2) can see: swapping two names' values keeps the table a bijection and
   keeps the two decoders agreeing with each other.
4. **A census of which tags the eleven-stage corpus actually exercises**, with the unexercised
   tags **named**, because that set is precisely where a transposition hides from the oracle.

## Prior failures this increment must not repeat

**A NAIVE ARM EXTRACTOR IS WRONG HERE, AND I ALREADY PROVED IT ON THIS TREE.** A regex over
`^\s+[0-9]+ =>` inside `awk '/^fn decode_op/,/^}/'` reports **63 arms in `src/selfhost/mod.rs` and
111 in `tests/selfhost_codegen.rs`**. The excess is nested match arms in the composite-kind
decoders, whose `0 =>` and `1 =>` are not op tags. The handoff already records that the op-tag and
record-code extractions in this repository **match by brace depth**, and the one that used a
character window was the outlier that failed. **Match by brace depth. A count of 111 is the
extractor being wrong, not the table.**

**A CHECK BUILT FROM THE SAME MODEL AS THE THING IT CHECKS CONFIRMS THE MODEL.** Recorded six
times in one session. The name-correspondence table in item 3 is a **fourth** hand-written table,
and that is the hazard to hold in view. It is justified only because it is a **different kind** of
derivation — names to names, not numbers to numbers — so a numeric transposition breaks it while a
consistent renumbering does not. **If it is written by copying the numbers, it is worthless.**
Write it from the names.

**A GUARD THAT HAS NOT BEEN MADE TO FAIL IS A GUESS.** Every one of these must be mutation-tested,
and the mutation must be **the one the real change would produce** — a swapped pair in one table
only, not an artificial marker. Two mutation attempts last session failed to COMPILE, producing
silence indistinguishable from a guard not firing. **Verify the mutant builds before believing its
result.**

**DERIVE THE SET FROM THE SOURCE AND ASSERT NON-VACUITY.** Two derivations fired on their first
run last session because the walk found zero of what it was looking for. A guard over an empty set
passes.

**DO NOT UPGRADE THE PEER'S CLAIM.** They wrote that no disagreement has been observed. If the
tables agree, say they agree and say what is now checked. Do not write that a defect was found.

**PROPORTIONALITY, AND STATE IT.** `self_hosted_compile` cross-checks against the reference and
refuses on divergence, so a table defect would give a loud error rather than a wrong module.
Exposure is to direct callers of the `self_host_compile*` entry points.

## The wrong turns specifically

- **Do not gate the whole file on `self-host`.** Items 1 to 3 read source text and need no
  feature; gating them hides them from every feature set. Item 4 needs `compile`, which is a
  DEFAULT feature, so gate that test alone.
- **Do not check the feature sets you are working in and skip the others.** Missed twice last
  session; four CI jobs red, then three.
- **Do not report a consistent renumbering as a defect.** See above.
- **Do not claim the census closes the hole.** It measures it. Naming the unexercised tags is the
  deliverable; extending the corpus to cover them is separate work.
