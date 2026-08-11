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
| `v0.2.3` | `cd064e6e`, pushed, CI confirming |
| PRs #9 - #12 | all **MERGED** on 22/22 green, each at the commit CI ran |
| Machine | idle throughout; every gate ran on hosted runners |

`tests/selfhost_wire.rs` is **139 tests**. **All five** of the values the driver owed are now
computed on the Keleusma side, the last of them in PR #11, and `CHUNKS` emits in batches with its
three running totals relayed across them (PR #12).

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

## Next intended step

**The window base.** Record emitters position at `region_base(i) + rec * stride + off`, an absolute
artifact offset, against a 65,536-byte buffer. Measured: **every stage fails, the smallest included**
— `verify_datalayout`'s `NAMES` region starts at byte 81,160. Absolute positioning holds for
artifacts under 65,536 bytes, which is the constructed corpus and no stage at all.

**It is independent of batching**, and the two are easy to conflate now that batching is fresh:
batching fixes how many records reach the emitter per call, the window base fixes where they land.

The two traps still stand: **do not** replace the linear dedup scan, and **do not** compute the chunk
record's name index (`map[j] == j` always).

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
