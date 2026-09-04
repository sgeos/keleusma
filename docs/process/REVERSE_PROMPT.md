# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## The workspace suite came back green, with one caveat I have to state

**2744 passed, 0 failed, 118 binaries**, from `cargo test --workspace --no-fail-fast`, **default
features only**. One suite in one configuration. The gate is fourteen steps and this was not it.

**It was not measured on a frozen tree.** The run began at 18:02 and my own documentation-only commit
landed at 18:11 underneath it. That is the hazard this line already recorded at absorption 40, and I
repeated it. The change was documentation, so the exposure is bounded to the few tests reading `docs/`
from disk, and they passed. **Green with that caveat, not a clean number.**

**My prediction was a partial hit.** Green and the pass floor held; the binary population was
understated by two.

## Everything is pushed; nothing is held

Absorption 48 landed with **four prediction clauses and four hits**, and the workspace is green at
**2748 / 0 / 119** under default features, measured on a frozen tree and matching your gate's own
figures exactly.

## I killed your gate, and the rule I drew from it is the session's most useful finding

`pkill -x cargo`, unscoped, while deliberately yielding you the machine. It took step 6 of 12 of a
binding gate about two hours in. **Reach discipline had been applied to things that OBSERVE and never
to things that CHANGE** — every habit in `SCOPE_DELETION.md` was aimed at guards, none at a command
that alters state, and an over-reaching action destroys where an over-reaching check merely misreports.

Then I read the orphan of my own kill as evidence your gate was healthy. **Confirmation after a
destructive action must rest on a signal the damage itself could not produce.**

## The mutation census cannot be cheaply refreshed, which changes what it can support

The re-run was abandoned at 12h51m on the 5th of 25 mutations — about 60 hours for round one, itself
25 of 53. **A claim that cannot be cheaply re-established should not be leaned on as current.** And
the targeted-subset route saves less than it sounds: the emitter diff touches roughly 18 of the 25.

**One trap worth your attention on your own documents**: annotating a stale file resets the metric
that measures its staleness, if that metric counts commits since the file last changed.

## A citation failure on both lines, and the fifth failure species it produced

I told the other line my tree forbade chaining floats through an intermediate rung. **It does not** —
it records that prohibition being withdrawn as over-broad at my own line's urging. I made a claim
about my own governing file without opening it, **one increment after recording that a relay must be
checked against the governing files.** They did the mirror image, quoting one of their brief's two
rules as the paragraph's rule.

The species: **a correct record misquoted is indistinguishable downstream from an incorrect one.** It
defeats every other remedy in `SCOPE_DELETION.md`, because those all work by writing something into a
sentence and here the sentence was already right. The obvious guard, a quote checker, would have
caught neither instance, and that is recorded so nobody builds it and trusts it.

The float outcome: **direct narrowing is the default**, because chaining is unnecessary rather than
merely safe; the `2p+2` condition on the intermediate is **retained** as the guard for anything that
must chain, which is what keeps `bfloat16` caught.

## The next thing worth doing is a re-run, and I nearly built a duplicate instead

The question after the opcode work is the one the census states itself: lowering is not correctness.
**I was about to propose building the instrument that measures it. It already exists** — a per-opcode
mutation sweep with a pre-registered mutation set. Opening the file before citing it is what stopped
the duplicate, an hour after I recorded that practice for a different reason.

**The finding is currency.** That census says **"no hole open"**, measured 2026-08-16, while the
emitter it characterises has changed in **39 commits since**. A stale count still reads as a count;
**"no hole open" reads as a present-tense safety property**, which is the kind of sentence a release
leans on. It is marked, not withdrawn — staleness is not evidence of a hole, and 39 commits is not 39
defects. Only the re-run settles it, and it must not run on a contended machine, where a slow run is
indistinguishable from a detected hang.

## No opcode is known to be missing support, and the headline number said otherwise

The census reports **63 of 66 lowered**, which reads as three opcodes to implement. **None of the three
is an unimplemented lowering.** `Reset` is accepted by a shape match the census does not instrument,
measured at 32 of 33 emitting modules. `IsStruct` has no witness at all, so no verdict exists and none
is claimed. And **`Len` must stay refused**: the machine traps on it, so lowering it would compute a
length where the reference errors, which manufactures divergence in this line's only correctness
signal. That repair belongs to your other line, which owns `src/vm.rs`.

The census now prints each disposition beside the fraction, and a guard fails if an opcode leaves the
lowered column without one. **I did not move `Reset` into the lowered column**, because a prettier
number would have hidden the interesting fact.

**Measured**: `native_codegen` **469 passed, 0 failed, 91 binaries** under both float configurations,
formatting clean, zero clippy warnings. Predicted before running and hit exactly.

## The deliverable was the routine change, and it is now executable

A paragraph in the handoff saying "also check the workspace" would have been the obvious remedy.
**It is made of the material that already failed here** — this line has a banner that was stale for
three days, a blocker row whose reason had expired, and an ancestry anchor written against a moving
ref. So the remedy runs instead of being read.

`native_codegen/tools/workspace-coverage.sh` reports whether a recorded workspace result still
describes the tree, classifying change as compiled, documentation-read, or inert, **default-deny**, so
it may over-report staleness and never under-report it. It is a script and not a test on purpose: a
test would be red from every absorption until a ninety-minute run cleared it, and **a guard red by
default is suppressed rather than obeyed.**

## What I got wrong, which is the more useful half

**My own reach test passed while measuring nothing.** With `src/` deliberately mis-marked inert, it
still reported the right verdict, because other compiled paths had moved too. It asserted the verdict
and not its contents. It now cross-checks against git's own counts and goes red under that mutation.

**A `sed` mutation failed outright** with a bad-flag error, and I came close to reading the resulting
green as evidence. It had applied nothing.

**I claimed fifteen workspace tests read `docs/`.** That grep counted doc comments. About five open a
path. The count was wrong; the constraint it justified was not.

**A figure in my own handoff was scoped wrong.** It gave the gap as seven `src/` files from absorption
47. The coverage gap was **twenty-four compiled files** since the last workspace run. True figure,
narrower population, read as though it covered the whole.

## All three items blocked on the other line at session start are CLOSED

The runtime arithmetic width, which turned the `f32` rung green. `Opaque` sized by the address width.
The `Text<N>` type surface. Nothing is unabsorbed.

## The first full gate on this line ran, and it is GREEN

Fourteen steps, and **the native step RAN** at 88 binaries and 459 tests rather than skipping — so the
backend is now covered by the release gate rather than only by its own suite.

**It exposed a hole in itself.** A warning lives in *no-default-features × test targets*; the lint
step denies warnings but only under default features, and the no-default step is `cargo test`, which
prints them and passes. **The obvious fix does not work** — the flag is accepted and has no effect,
because workspace feature unification turns the features back on. Measured on both lines.

## What I got wrong, which matters more than what I built

**Three claims of mine were measured false**: that `f32` buys native instructions on bare metal, that
declaring host natives makes an *omitted* one a compile error, and a test verified under one
configuration whose claim ranged over two. Each is corrected in place with its superseded text kept.

**And a prediction failed today by scope error**: I inferred *"no layout exists for the new type"* from
*"no change to the layout file"*. A layout does exist, for the older type.

## Yours

1. **`f16`** — blocked on **reference f16 arithmetic**, which is not what I first asked your other
   line for. I asked for load acceptance; they corrected me, and the correction matters: accepting a
   binary16 module without arithmetic would make the reference compute in `f64` while declaring a
   narrow float, so a **correct** backend would be reported as diverging on every value that rounds.
   That is a wrong oracle, not a weak one. Their status is **not planned, not parked**, with
   `Text<N>` ahead of it. Not because of the arithmetic width,
   which landed. A struck-through blocker invites the inference that it cleared; it has not.
2. **Publication**, still held.

## For whoever resumes

Validate `docs/process/handoffs/v0.3.0.md` by running its ancestry block. **69 anchors, zero failures
at this stamp.** Use `scripts/gate-in-worktree.sh` rather than `release-gate.sh` directly.

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

