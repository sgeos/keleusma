# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-11 (session 41, continued)

## Where things stand

| | |
|---|---|
| `v0.2.3` | `6715d424`, pushed, CI confirming |
| PRs #9-#13, #15, #17, #19, #21, #22 | all **MERGED** on 22/22 green, each at the commit CI ran |
| Machine | idle throughout; every gate ran on hosted runners |

`tests/selfhost_wire.rs` is **148 tests**. **All five** of the values the driver owed are now
computed on the Keleusma side, the last of them in PR #11, and `CHUNKS` emits in batches with its
three running totals relayed across them (PR #12), into a low window so a real stage's region is
reachable at all (PR #13). The window is now GENERAL: `emit_at(k, n, at)` serves all seventeen record
kinds, with `emit_in_region` the absolute caller and `emit_in_window` the windowed one (PR #15).

## A guard that documented a check it did not make

`assert_no_other_contributors` claimed to refuse modules whose names come from "data slots, natives,
struct templates or composite constants" and checked only the first three. Nothing hid it: no source
in `INTERNER_CASES` reaches a named constant, so the missing clause had nothing to refuse. **That is
a fact about the corpus, not about the guard** — the same distinction that overturned two plan
conclusions yesterday, arriving from the opposite direction.

The first fix was wrong and the failing test explained why. **Two models share that guard and only
one needs the clause**: `fx_input` appends the constant walk's names to the `interner_input` prefix
and so covers the class by construction. The comment described the union of what both models need
while the code implemented the intersection. The clause now lives in its own
`assert_constants_are_modelled` at the two `interner_input`-only sites.

**I overclaimed the consequence and a probe caught it.** From reading `encode_aux_body` I concluded
constants intern BEFORE chunk names and therefore that an unmodelled constant shifts every index the
model produces — a correctness hole. Dumping the reference's actual `NAMES` order refuted it:
`["main", "hi"]`, `["main", "take", "P", "x", "y"]`. Chunk names come first, an unmodelled constant
costs a **suffix**, and the failure is loud rather than silent. The clause buys a named diagnostic
plus insurance if that ordering changes, which is a smaller claim than the one I began writing.

## A recorded fact was wrong in both of its particulars

"A dispatch chain caps at NINETEEN arms, and exceeding it is a stack overflow, not a parse error."

The cap is **not an arm count**. It is a **depth budget of 24 shared between chain position and
arm-body nesting**, so each level an arm body nests costs one arm off the chain. Two earlier sessions
recorded 19 and 23 for the same parser; both were right for their arm shape and neither generalises.

**The failure mode depends on execution context, and my first measurement used the wrong one:**

| context | no-arg call body | nested-call body | failure |
|---|---|---|---|
| **test harness** (2 MB threads) | **20 arms** | **18 arms** | stack overflow, SIGABRT |
| CLI (larger main stack) | 23 | 20 or fewer | clean `ParseError` naming the limit |

The harness binds, because that is where `wire.kel` is compiled. `dispatch_driver` is at 18 arms, so
headroom is **two arms or none** depending on what the arm calls — not the four the CLI figure
suggests. **Do not size a chain from a CLI measurement.**

## Concern raised, not acted on

**`MAX_PARSE_DEPTH` does not do its stated job on a small stack, and this is a runtime concern rather
than a workflow one.** The constant is 24 (`src/parser.rs:98`) and its message says deeply nested
expressions are "rejected to prevent stack overflow". On a 2 MB thread the stack blows before the
guard fires, so the process aborts with SIGABRT instead of returning a `ParseError`. The limit is
evidently calibrated for a main thread's larger stack.

An embedder that parses untrusted source on a small-stack thread therefore gets an **abort rather
than a rejection**, which is an availability failure at the trust boundary the guard exists to hold.
**Not changed unilaterally**: lowering the constant narrows the admitted language surface and one
measurement is not grounds for that. Operator's call.

## Two of my own errors this session, since both generalise

- **An f-string collapsed `}}` into `}`**, silently dropping nine closing braces from a probe. I
  caught it only because the probe's **unmodified baseline** also failed. A probe whose no-op case is
  not asserted to be a no-op cannot distinguish a real finding from a broken harness.
- **I made the exact naive-grep error the loop document warns about**, counting `Gap` inside a
  comment that reads "This is a Gap by design" and nearly recording a false staleness. Excluding
  comment lines gives **79 Ok / 4 Gap / 1 RefRejects, 84 cases** — the recorded figure is current.

## The thing I would most want a reader to take from this session

**I published a confident, derived number and had to retract it the same day.** Probing the plan's
residency section — which carried an explicit "confirm this" caveat — I measured that a declared byte
costs about 40.7 bytes of artifact and concluded the 77% projection was "refuted by a factor of
forty", with a ~321,000-slot budget to go with it. Both are withdrawn by `69a32862`.

**The budget divided a byte-addressing ceiling by a figure in bytes of ARTIFACT per slot.** Different
quantities. The factor of forty was the units error itself. `MAX_DATA_ADDR` bounds a byte offset and
a slot index, not the artifact, which the container addresses with u32 words and so may reach ~34 GB.
Against the real ceilings `lexer` needs 59.2% — the 58.3% the plan already recorded. **The projection
was right.**

**Why it survived the checking I did do.** `2^24` is a byte offset, AND a slot index, AND
coincidentally close to `lexer`'s own artifact size. The wrong reading was self-consistent from three
directions, so every sanity check agreed with it. **A constant that appears in several places for
several reasons is where this goes wrong**, and the only thing that catches it is asking what a
number BOUNDS rather than reusing one of the right order of magnitude.

**What survives is real** and is what the plan omitted rather than got wrong: one data slot per array
element with exact deltas, ~40.7 bytes of artifact per slot, and ~2.4 s of compile time per megabyte
declared. Declaring `lexer`'s accumulator costs a ~400 MB body and a 25-second compile — a serious
practical cost, not a limit violation.

## The mechanical arc is complete

**PR #21 was the capstone**: Keleusma's own output builds `verify_datalayout`'s entire 105,848-byte
auxiliary body, byte-identical to the reference. All five computed values, batching on both paths,
window positioning across all seventeen record kinds, multi-window assembly, and whole-artifact
composition.

**One grep decided that increment's size.** The artifact's only checksum is `crc32(&prologue[..12])`
— twelve bytes, not the body. Had it covered the body the driver would have needed an incremental CRC
carried across windows, since 105,848 bytes never fit a 65,536-byte buffer. Fourth consecutive gap
here that needed a caller rather than an emitter, and the first where the check could plausibly have
gone the other way.

## Next intended step

**Do not reach for another emitter slice — there is no obvious one left**, and inventing one is how a
programme starts producing mechanisms that work and were not needed. In order:

1. **A second stage through the capstone.** `verify_yield` at 303,464 bytes exercises multi-window
   assembly *inside* whole-artifact composition; `verify_datalayout`'s regions all fit one window.
2. **The residency question, which is yours** — ~40.7 bytes of artifact per data slot, one slot per
   array element, ~2.4 s of compile time per megabyte. A representation decision, not an increment.
3. **`parse` and `verify_typed` end to end**, which need (2) answered first.

## A local check of mine could not fail, and the gate caught what it missed

The pre-push gate rejected a `clippy::empty_line_after_doc_comments` that a refactor introduced. My
own check had reported clean throughout because it read `$?` after a **pipeline**:
`cargo clippy ... | tail -2; echo "LINT_RC=$?"` reports **tail's** status, never clippy's.

**That is the defect class this suite's vacuity tests exist to guard against** — a control that
cannot report a failure, reading as evidence — committed in my own tooling, against a rule I had
already written down after making the same masked-exit-code mistake earlier in the session.
**Recording a rule is not following it.** What hid it was that the check kept returning the answer I
expected.

CI ran real clippy on every merged pull request, so nothing unsound shipped; the local signal was
worthless. Exit codes now go through `PIPESTATUS`.

## A rationale I recorded wrongly, caught by reading the code

The handoff ranked "a second stage through the capstone" on the claim that a larger stage would
exercise multi-window assembly inside whole-artifact composition. **It does not** — every batch is
emitted at window base zero and spliced immediately, so no window accumulates however large the
region. The increment bought breadth instead, which is a smaller and true claim. The wrong reason sat
in the handoff for a day before anyone checked it.

## A worthless mutation, recorded as worthless

Verifying the `DATA_SLOTS` path I inserted an inert assignment to a scratch field. It changed no
behaviour, the test passed, and that momentarily reads as a coverage gap. **A mutant that perturbs
nothing proves nothing in either direction.** The real mutations fire in two different regions.

## A note on telling the two sessions apart

Both use the same GitHub account, so `gh pr list --author @me` matches the other line's pull requests
too. I reported "zero open PRs" more than once today on that basis. **Distinguish by BASE BRANCH**:
anything based on `v0.3.0` is theirs.

## Two defects from one reading habit, and a third of the same shape

`stride_of_kind` returns **three** things and only two are strides: a positive record stride, **0 for
a byte pool**, **-1 for an unknown kind**. Its own comment says so. I read `<= 0` as "not a record
kind" and refused `STRING_POOL`, `PARAM_TYPES` and `DEBUG_POOL`; the same zero would have bounded
every pool write at zero bytes. It surfaced as `kind 30 refused with -222`.

**The regression test I wrote to pin that fix then found a pre-existing hole**: `emit_at` has no arm
for `DEBUG_POOL`, which appears only under `emit_debug` and had always been driven through a
different caller. That is the strongest argument I have met for pinning a fix rather than merely
making it.

**This is the same shape as the day's retraction, one level down.** There, `2^24` carried two
meanings — byte offset and slot index — and I used one where the other was needed. Here `0` and `-1`
carry different meanings and I merged them. **When a function documents its return values in prose,
the prose is the specification.**

## A process slip, caught before push

I committed PR #15's code directly onto `v0.2.3` rather than a feature branch. `origin` never saw it;
repaired locally by branching at the commit and hard-resetting the version branch to match origin. No
shared history rewritten.

The cause is mechanical: a merge had left me on the version branch, I wrote a legitimate docs commit
there, and then started editing code. **Cut the branch as the first action of an increment.** I did
that correctly five increments running and skipped it exactly when a merge had already put me on the
version branch — that is the moment to guard.

## A timing scare that was mine, and the operator caught it

The suite read **1456.76s against a 150s baseline** and I began composing an explanation about the
cost of compiling a real stage in-suite — plausible, self-consistent, and it would have led me to
redesign a test case that was fine. The operator asked whether the two running shells were
productive. **One was a stale run of my own**, started before an edit that invalidated it, which I
had noticed at the time and left running anyway. Killed it; clean measurement is **150.66s,
unchanged**.

**Kill a run the moment its inputs change** — leaving it beside a new one is the exact machine
contention this project moved to hosted runners to escape, reproduced by me on the same day. And **a
10x anomaly is a claim about the environment until proven otherwise.**

## A second lesson, from the batching slice

**Both wrong turns in PR #12 were the same mistake: copying the nearer of two adjacent precedents.**
`STRING_POOL` routed down the record path emitted silent zeros, because the generic emitter treats it
as a byte pool while the interner tests do not — two `is_pool` lines four hundred apart, and I took
the wrong one. And the failure assertion compared two 13,664-byte vectors, which is what every
neighbouring test does and is fine when the artifact is 912 bytes; it printed 85 KB and located
nothing.

**A precedent is scoped to the case that produced it.** That is the same shape as the day's other
corrections, where a number was reused past what it actually bounded. Worth noting too that the
diagnostic had to be fixed *before* the mutation check was readable enough to trust, so improving the
tooling was a precondition for the verification rather than a detour from it.

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **`MAX_PARSE_DEPTH` on small stacks**, above.
- **Per-element data slots, now with a measured price.** One slot and one interned name per array
  element, paid three times over in parallel tables plus the pool they index. Measured this session:
  **about 40.7 bytes of artifact per slot**, stable to within 1% across a fourfold range, and compile
  time of roughly **2.4 seconds per megabyte declared**. This is what makes a large declared buffer
  expensive; fixing the representation is what makes those numbers go away.
- **The (72,64) SECDED plane is entirely unexercised** by the shipping encoder.
- **MSRV**: CI checks 1.85 for `keleusma-arena` and 1.88 for `keleusma`.

## Parallel development

`v0.3.0` carries native code generation on the same CI-gated workflow. Their measurement that matters
here: **ten of eleven stage modules refuse native lowering on `Stream`, not on composites**, so Order
1's native path is gated on sub-coroutines. Their caveat stands — `lower_module` refuses on the first
unsupported opcode, so `Stream` is necessary, not provably sole. Their mailbox is
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`; mine is
[`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md). Poll at increment boundaries — there is no wake.

## Method rules this session paid for

- **Read a guard's implementation against its own doc comment.** One such reading found a claimed
  check that did not exist.
- **Measure before writing down a conclusion that upgrades a defect's severity.** The inference was
  cheap and wrong; the probe was cheap and right.
- **Assert that a probe's no-op case is a no-op**, or a broken harness reads as a finding.
- **Measure in the context that binds.** A CLI figure and a test-harness figure differed by two to
  three arms and by failure mode.
- **Check `$?` explicitly; never read success off output.** Held this session, including on the
  merge.
