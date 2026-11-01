# Brief — the instrument exists, and its answer is seventeen days old

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-02, night.**

---

## The present goals

| goal | state |
|---|---|
| **the mutation census's currency** | **this brief** |
| six commits held locally | deliberate; the `v0.2.3` gate has the machine |
| `f16` | blocked on **reference f16 arithmetic**, not load acceptance |
| publication | held |
| absorption 48 | nothing unabsorbed |

## What I nearly did, and why reading first stopped it

The question worth asking after the opcode dispositions is the one the ISA census states itself: **a
lowering verdict is not a correctness claim.** So which lowered opcodes are covered by a differential
that would actually catch a wrong result?

**I was about to propose building that instrument. It already exists.**
`native_codegen/tools/mutation_sweep.py` answers exactly it — per opcode, would a defect in its
lowering be detected by the corpus differential — with a **pre-registered** mutation set, per-module
process isolation, and four distinct outcomes where a signal and a disagreement are recorded
separately because they mean different things.

**Proposing it as new work would have been the scope error this file's collection is about**, and the
thing that prevented it was opening the file before citing it, which is the practice recorded in
[`SCOPE_DELETION.md`](./SCOPE_DELETION.md) an hour earlier.

## The actual finding: a present-tense safety claim describing a past tree

[`NATIVE_MUTATION_CENSUS.md`](./NATIVE_MUTATION_CENSUS.md) opens with:

> **Status**: measured, the harness repaired twice, and **no hole open**.

Measured, and true when written. **Dated 2026-08-14, extended through 2026-08-16.**

| | |
|---|---|
| census last touched | **2026-08-16** |
| sweep tool last touched | **2026-08-16** |
| `native_codegen/src/lib.rs` commits since | **39** |

**The emitter that sentence characterises has changed thirty-nine times since it was written.** The
sweep mutates that emitter; its answer is a property of the emitter as it stood.

**This is worse than the workspace staleness closed earlier tonight.** That figure was a test count,
and a stale count reads as a count. **"No hole open" reads as a present-tense property of the tree**,
and it is the kind of sentence a release decision leans on.

## What the census found, which is why staleness here matters

Its own table records that `CmpLt` lowered as `SLE` — a boundary defect — was **NOT DETECTED; the
whole differential passed**. That hole was later closed, and `Trap` was closed by changing the
**observable** rather than the inputs.

So this corpus has **already demonstrated** it can execute an opcode thoroughly and still be blind to
a wrong result from it. The sensitivity question is not theoretical here; it has a confirmed positive.

## The wrong turns

**1. Do not rebuild the sweep.** It is better than what would be written fresh, particularly the
pre-registration and the signal-versus-disagreement distinction. Re-running it is the work.

**2. Do not read "39 commits" as "39 defects", or as any defect at all.** It is the size of the gap
between a measurement and its subject. **Staleness is not evidence of a hole.** Saying otherwise
would be inventing a finding.

**3. Do not re-run the sweep on a busy machine.** It runs a process per module per mutation and a
calibrated timeout distinguishes a hang from slowness. **A contended machine makes a slow run look
like a detection**, which would fabricate coverage that is not there. The `v0.2.3` line's gate has the
machine.

**4. Do not report the re-run as a gate.** It is one instrument over the corpus.

**5. If the re-run comes back identical, say so and stamp it.** A figure that re-derives unchanged
still needs its date moved, because that stamp is the only evidence anyone looked.
