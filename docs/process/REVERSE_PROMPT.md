# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-28 (session 56) — an inbound finding closed, and Order 1 item 3 moved to three of
five

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I have
not acted on it.** Their own record now says the recommendation *splits* on a question you have not
answered — whether the fixed-point format must interoperate across object files from different
languages. Publication remains held.

## What this increment did

Two things landed. The first closed a finding the `v0.3.0` line handed over and could not close
themselves; the second is the roadmap item.

**ORDER 1 ITEM 3 IS AT THREE OF FIVE.** `field_sets` joins `binding_rows` and `decl_call_rows`.
Two remain, and only the DECLARED half of `field_sets` moved — its field accesses still walk the
reference syntax tree, which the function and the test both say in their own words rather than
letting the headline imply more.

## The part worth your attention: my brief was wrong, cheaply

I wrote a brief saying the work meant surfacing a table held inside `parse.kel`, which would have
required new emission from a stage that is itself in the byte-identity corpus — a much larger and
riskier increment.

**`parse.kel` was already emitting all of it.** The struct's name and every field name were on the
record stream, in declaration order, and the driver mapped the whole run to skip state and threw it
away. The increment touched no stage source.

**The lesson is not "read more".** I did read — I read the producer's internal data structures and
reasoned about what the host could not see. The record stream is the interface, and it already
carried the answer. Reading the producer's internals told me about the producer, not about what
crosses the boundary. The correction is recorded beside the original claim rather than edited away.

## What mutation testing caught that reasoning did not

The driver had one skip state covering struct, trait and impl declarations, and that state exists
because those three once faulted the driver on 29 boundary cases. Collecting structs meant
splitting it.

**Re-admitting trait and impl into the collect leaves the agreement test PASSING**, because its
probes contain neither. A guard whose corpus lacks the construct is a guard for a different
question. A second test now carries that case, with its spelling taken from a shipped example
rather than invented, because five of this line's probes have measured a malformed input and
reported the result as a finding about the stage.

## The earlier increment, briefly

The `v0.3.0` line observed that the self-hosted codegen's 63 op tags and the driver's decoder are
two hand-maintained tables whose only guard asserts that decoding does not panic — **a
transposition passed it**. There were **three** tables, not two; the third is the decoder copy the
differential oracle actually runs, which the shipping decoder claims lockstep with and nothing
checked. **They agree**, now measured rather than inferred.

And a measurement that bears on coverage generally: **sixteen of the sixty-three tags are exercised
by no stage source**, so the self-hosting oracle cannot see a transposition among them. Scoped
deliberately — the per-construct tests do cover composites, so these are invisible to that oracle,
not unchecked.

## Nothing is waiting on you except the ruling you already have

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I have
not acted on it.** Publication remains held.

## One observation, attributed rather than assumed

`cargo clippy --tests --no-default-features -- -D warnings` fails with seven diagnostics, and fails
**identically on a clean tree** — established by stashing, not inferred. Pre-existing, not a
combination continuous integration runs, and not fixed here because it is outside what these
increments were about.

## The twelfth stage, which I would want you to know about

`verify_types.kel` is the only stage source with no byte-identity test, and nothing in the tree
recorded why, so the absence read as an oversight. **It is refused, reproducibly.**

A function reads a `data` block declared **later in the file**, and the self-hosted parser resolves
`block.field` against a table it builds as it meets each block, so the reference resolves to
nothing. Four-line witness, with a control that differs only in declaration order.

**I did not attempt the repair.** It means collecting data declarations before parsing bodies,
which is a two-pass restructuring of a single-pass streaming parser rather than a defect fix. That
is a decision I would rather you saw than have me take quietly inside a session. What landed is the
reproduction, the control, and a derived pin that the corpus is eleven of twelve with the missing
stage named — so the absence is now a documented gap instead of folklore.

Three plausible hypotheses failed before the structural one succeeded, and none of them mentioned
declaration order.

## What I would take up next

Either the two-pass data resolution above — which would take the corpus to twelve and is the
largest single remaining item I can see — or the occurrences half of `occurrence_rows`. Local reads
are body record code 2 carrying a SLOT, and the driver holds parameter and `let` names, so a
slot-to-name map is available.

## A postscript on the session's most repeated mistake

I predicted three times that a slice would be harder or differently shaped than it was, each time by
reasoning from a component's internals rather than from the interface. Twice the record stream
already carried the answer; once the culprit was declaration order rather than any of the three
constructs I suspected. The instrument for the first kind is `parse_record_trace`; for the second it
is bisecting against the real file with a control. Both handoffs now say so.
