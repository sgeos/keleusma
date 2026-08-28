# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-28 (session 56) — an inbound finding closed by measurement, and sixteen op tags
the self-hosting oracle cannot see

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I have
not acted on it.** Their own record now says the recommendation *splits* on a question you have not
answered — whether the fixed-point format must interoperate across object files from different
languages. Publication remains held.

## What this increment did

The `v0.3.0` line handed this line a finding they could not close: the self-hosted codegen stage's
63 op tags and the driver's decoder are two hand-maintained tables of the same numbers, and their
only guard asserts that decoding does not panic. **A transposition passed it.** It was unrecorded
here.

**The tables agree.** That is now a measurement rather than an inference from a comment claiming
they are kept in lockstep.

## Three things worth your attention

**ONE. THERE WERE THREE TABLES, NOT TWO.** The third is the copy of the decoder inside
`tests/selfhost_codegen.rs` — the one the differential oracle actually runs — which the shipping
decoder's own comment names as its source and claims to be in lockstep with. Nothing checked that.
It is the same pairing that produced five defects from one cause in August, and a drift there
would corrupt the oracle rather than the product.

**TWO. SIXTEEN OF THE SIXTY-THREE TAGS ARE INVISIBLE TO THE BYTE-IDENTITY ORACLE.** No stage source
emits them — the whole composite family, the unchecked arithmetic, and `checkedneg` — so a
transposition among them produces no byte difference to detect. They are named in the test rather
than counted, because the names are where such a defect would hide.

**I have scoped that claim deliberately and want the scope read.** It is the eleven-stage corpus.
The per-construct tests do compile struct constructions, array indexing, enum payloads and tuple
fields through the self-hosted compiler, so these are **not** "unchecked" — they are "invisible to
the self-hosting oracle", which is a narrower and true statement.

**THREE. ONLY ONE OF THE FOUR NEW GUARDS CATCHES THE DEFECT THE FINDING NAMED**, and mutation
testing is what established that rather than reasoning. A one-sided swap leaves the table a
bijection and leaves the two decoders agreeing with each other. The guard that sees it compares
each tag's NAME to the operation its number decodes to — a fourth hand-written table, which is a
hazard, and which earns its place only because it derives names from names where the others derive
numbers from numbers.

## What went wrong, since that is the more useful half

**The citation guard caught me inside ten minutes.** I renamed the census test for scope precision
and left the module header naming the old one. Fourth occurrence of that class here, and the
shortest interval yet between creating a stale citation and having it reported. The guard added
last session is now paying for itself against its own author.

**My first extractor would have compared two different populations.** A naive line pattern reports
63 decoder arms on one side and 111 on the other, the excess being arms of nested matches that look
identical by line shape. I checked the instrument before trusting the reading, which this line has
now had to do three times.

## One observation, attributed rather than assumed

`cargo clippy --tests --no-default-features -- -D warnings` fails with seven diagnostics. It fails
**identically on a clean `v0.2.3`** — established by stashing and re-running, not inferred — so it
is pre-existing and no part of this work. That combination is not one continuous integration runs.
Recorded as a fact about the tree, not as a claim that something is broken, and not fixed here
because it is outside what this increment was about.

## What I would take up next

The third type-channel extraction, which is Order 1 item 3 and the roadmap-advancing work.
`field_sets` at 80 lines or `occurrence_rows` at 100; leave `expression_nodes_and_derived` at 142
for last despite the capability argument favouring it. The pattern is established by the two slices
that already moved and is written into the handoff so it is not rediscovered.
