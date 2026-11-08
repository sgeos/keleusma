# BRIEF — state the roadmap's Order-1 gate as a MEASURED figure

## The goal

`V0_3_X_ROADMAP.md` gives Order 1 a gate in words:

> *"The self-hosted compiler's own bytecode runs correctly as native code, differential-tested
> against the VM."*

**Nobody has ever stated whether it is met.** The corpus differential already drives every self-hosted
stage and knows the answer; it just never separates them from the other fifty-odd modules. Reading
today's exempt list by hand: of the twelve files in `src/selfhost/kel/`, only `wire.kel` is exempt.
So **eleven of twelve execute and agree** — and that is the roadmap's own milestone, unreported.

This is the last item on the roadmap this line can settle without an operator, and it is worth
settling because a milestone nobody has evaluated is indistinguishable from one nobody has met.

## The trap, and it is the biggest on this line

**"Eleven of twelve agree, Order 1 is met" is exactly the inflated headline this line already
shipped once.** The handoff records it: `is_vacuous` asked whether the shared segment was non-zero,
a seeded module is non-zero *before executing a single operation*, and three `verify_*` stages left
`KNOWN_VACUOUS` on that basis — taking the headline from 40 to 44 for no reason at all.

The stages are **seeded**. A stage that agrees might be agreeing on a run that did nothing. So the
figure is worth nothing without, per stage:

- does it **lower**, and does it **execute**;
- what **carries** the agreement — a varying result, a native call, a composite body, a segment
  write, or nothing at all;
- whether it is **vacuous**, and why.

**One stage, `verify_datalayout.kel`, never runs and is blocked BY DESIGN** — its verdict
accumulates across three differently-encoded phases in the retained buffer. Do not invent a
batch-zero seed for it; the handoff says so explicitly.

**`wire.kel` is exempt but is NOT a disagreement.** Measured earlier: both sides fault at tick 19,
the virtual machine naming `IndexOutOfBounds` and the native side raising `SIGTRAP`. The exemption
hides agreement, not a defect — but SIGTRAP proves *a* fault and not *which*, so it is agreement in
the fact and position of the fault, not in its identity. **Say that, do not round it up to
agreement.**

## Specific wrong turns to avoid

- **Do not write a second corpus walker.** The differential already computes every input to this.
  This line resolved a 1032-vs-1027 discrepancy by restricting an existing walker rather than adding
  a forbidden third, and the same rule applies now. Report from inside the existing machinery.
- **Do not assert the gate is MET.** State what is measured and let a reader judge. "Eleven of
  twelve agree" plus a per-stage carrier breakdown is a fact; "Order 1 is met" is a conclusion that
  hides the vacuity question.
- **Do not pin the per-stage distribution.** It moves with the corpus and with the harness. Pin only
  that the stages were found and driven — a report over an empty stage set must fail.
- **Count the stages from the DIRECTORY, not from a list.** A hand-written list of twelve names is
  the thing that goes stale silently, which is the failure the ISA census exists to prevent.
- **Do not let a stage that is EXEMPT read as one that FAILED.** Exempt, vacuous, and disagreeing
  are three different states and the report must keep them apart.

## What a good outcome looks like

The roadmap's Order-1 gate has a number attached, taken from a run, with the vacuity and
fault-identity qualifications stated where the number is read — and a reader can tell which stages
carry real agreement and which do not.

**If the honest answer is "mostly, with two qualifications", that is the answer.** A milestone
reported as met when one stage never runs and another agrees only about the position of a fault
would be the same error this line has spent the whole session finding in other people's claims and
its own.
