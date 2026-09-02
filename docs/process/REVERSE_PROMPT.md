# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## Seven increments and absorption 43. `native_codegen` is 454 passed, 0 failed, 87 binaries.

That figure is over the **`native_codegen` package only**, cargo's exit status and the summed
per-binary counts agreeing, measured with no edits in flight.

## The standing prediction is discharged, and it held

**69 modules still compile** through the `v0.2.3` line's `Text + Text` refusal. It had been open since
before absorption 42 because the refusal sat on a feature branch. The scan behind it still could not
see a variable-to-variable concatenation, so the caveat retires with the prediction rather than
outliving it.

## Three things worth your attention

**The host and bare-metal toolchain requirements are DISJOINT.** The host needs two symbols, the
bare-metal target eleven others, and the intersection is empty. **A host measurement is not even a
conservative guide to an embedded link.** I would have assumed otherwise, and so would the other line.

**A single `f64` addition pulls six runtime symbols on `thumbv8m`.** So `f32` and `f16` buy native
instructions on the target the value proposition is written for — an argument for the float ladder
about what is *linkable*, not what is smaller, and one absent from its original reasoning.

**Half the bound transfers and half does not.** The memory figures describe the emitted object and are
measured against it. The cost-unit figure is a bytecode count under a virtual-machine cost model, and
nothing relates it to native execution. They were printed under one heading reading "PROVEN BOUNDS";
they are not any more. **The figure is kept, because it is true about the bytecode** — only its
subject was unstated.

## What I refused, and one decision I reversed

**No native worst-case-time model was started.** What one would require is named and explicitly not
begun: it is a workstream, not an increment.

**I recorded a decision and reversed it the same day, and both are in the record.** I concluded
"change nothing" about a suspicious default arm. The `v0.2.3` line pointed out that their equivalent
sites refuse at the same junction, so the convention I was contradicting locally is one this codebase
already follows. **The superseded reasoning is kept beside the reversal**, because a decision that
quietly becomes its opposite teaches nothing.

## The methodological finding, which I think outranks the code

`docs/decisions/SCOPE_DELETION.md`. Both lines produced the same error six times in one day: **a true
measurement quoted with its scope deleted.** Not wrong numbers, not careless checks — correct results
quoted as ranging over more than they did. The scoped and unscoped sentences differ by a clause, so
nothing looks missing.

**The worst instance is mine, and it had three falsifiers written to catch it.** All three passed,
because they tested containment in one direction and the claim was about containment. **A falsifier
that shares its claim's framing cannot falsify the framing.**

## Yours

1. **Publication**, still held.
2. **`f16`**, ruled and now buildable once the arithmetic width is absorbed.

## What is blocked on the other line

The arithmetic width is **implemented and pending their gate**. When I absorb it, the one red `f32`
test here should go green — and they asked me to check that rung specifically rather than read a green
as floats-in-general, because nothing implements binary16 or E5M2 yet.

`Text<N>` and `Opaque` remain theirs.

---
---
# Also unread by the human: the `v0.2.3` line's message

**Both lines write this one file, so absorption 34 conflicted here.** Neither message is discarded.
**This is a merge resolution, not a relay** — nothing below was reviewed, re-derived, or endorsed by
the V0.3.X line, and its figures describe that line's tree.

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-09-01 (session 61) — five merges, nothing red or unmerged, and `Text<N>` is the only
major item left

## NOTHING IS RED AND NOTHING OF MINE IS UNMERGED

`origin/v0.2.3` is at `27fcbd11`. Every branch this line created is merged. Worktrees are clean.
That is the first time today it has been true, and it is deliberate: the previous session stranded a
branch twice and I would rather hand you an empty queue than a short one.

The full release gate is green at 13 steps. Per-step, because a total across steps double-counts:
default 2739, no-default 297, signatures 2347, signatures+shell 2364, self-host 2518, wire 57 and 20,
detached compiler 86.

## WHAT LANDED

**The format fingerprint**, per your redirect. Random per release, in a constant beside
`BYTECODE_VERSION`, currently `0x4327_63E1`. `scripts/fingerprint.sh` reads this tree's value, reads
any commit's or tag's, and rolls a new one. **Release step 1b does the rolling**, with the reason
attached: skipping it produces no warning and no test failure, only two releases silently accepting
each other's bytecode.

**Your redirect was right and it answered the objection my version was built on.** I argued a
hand-written constant fails by being forgotten. True, but a derived value covers only what it hashes,
so a release changing an opcode's meaning would leave it unmoved while genuinely differing.

**Float arithmetic honours the declared width** at ten sites. This was a defect in every build, not
just under `narrow-float-32`, because the runtime admits narrower-than-declared bytecode on purpose.
The `v0.3.0` line's `f32` rung went green on absorption, which is independent confirmation from a
separate implementation — mine says the construction is self-consistent, theirs says it is right.

**A target may not claim floats at a width that is not a format.** `has_floats` with a zero width
compiled, loaded and returned 3.75, computing in `f64` while declaring zero bits.

**`Opaque` is sized by the address width**, finally. That branch had been red since yesterday.

## THE OPEN DECISION IS STILL YOURS, AND IT IS NOW ON THE CRITICAL PATH

**Whose release gate is canonical at the back-merge.** The `v0.3.0` line's `scripts/release-gate.sh`
differs from mine by 29 lines — a `native_codegen` step conditional on an LLVM install.

Everything either line has said today about "the gate is green" was said about a **different
instrument**. At the back-merge one definition wins and the losing line's recorded greens were
produced by a tool that no longer exists.

I ruled union rather than choice, since their step covers a package my gate is structurally blind to,
and conditioned it: the skip must be loud, `gate-summary.sh` must show it, and a skipped native step
is NO-GO for a publication shipping the native backend. That last is now in `RELEASE_PROCESS.md`. **If
you disagree with union, say so before the back-merge rather than during one.**

## WHAT I GOT WRONG, BECAUSE THE CORRECTIONS ARE THE USEFUL PART

- **A gate wait that measured its own existence.** `pgrep -f "release-gate.sh"` matches the shell
  running it. I reported a gate as running for nearly two hours after it went green.
- **A denylist for a safety predicate**, in a default-deny codebase, one commit after writing a
  release rule about instruments asserting more than they measured.
- **Two skips in an enumeration test that incremented nothing.** One dropped the exact widths the
  denylist had wrongly admitted.
- **A test named for a subject its body never reached**, which is worse than no test: it consumes the
  attention that would have written a real one.
- **Eleven passing tests covering four of ten sites**, found by mutation rather than by reading.

## THE QUEUE

1. **`Text<N>`.** Designed, authorized, unstarted, and the largest thing left. Its brief and
   completion condition are drafted. The `ScalarKind::Text` collapse to one address must land WITH it
   — that is a wire change, free before publication and unavailable after.
2. **The width bundle**, recorded as debt rather than paid. `addr_bytes` appears at 43 signatures
   across seven modules, several public. Cheaper before a publication than after one.
3. The earlier queue is unchanged: the discard-arm reachability census, and `DATA_INIT` for the one
   stage that does not elide.

## FOR YOU

The exchange with the `v0.3.0` line produced more than either line's code did. Six instances of one
error class in a day, catalogued jointly, each found by the other line rather than by the line that
made it. The general form: **a true measurement quoted as though it ranged wider than it did**, where
the scoped and unscoped statements are the same sentence minus a clause, so nothing looks missing.

Worth knowing when you weigh what either line reports: neither of us caught our own instances.

