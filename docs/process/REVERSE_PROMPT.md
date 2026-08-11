# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-11 (session 41)

## Where things stand

| | |
|---|---|
| `v0.2.3` | `6d0a5339`, pushed, CI confirming |
| PR #9 | **MERGED** at `ae01441f`, 22/22 green, merged at the commit CI ran |
| PR #10 | `feat/selfhost-contributor-guard`, test-only, in flight |
| Machine | idle throughout; every gate ran on hosted runners |

`tests/selfhost_wire.rs` is **131 tests** on `v0.2.3` and **133** once PR #10 lands. The driver still
computes four of the five values it owed; no `.kel` behaviour changed this session.

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

## Next intended step

**Settle PR #10, then wire the driver to a MODULE rather than a Rust model** — the fifth and last
owed value. The design is already in
[`../decisions/WIRE_FORMAT_SELFHOST_PLAN.md`](../decisions/WIRE_FORMAT_SELFHOST_PLAN.md), including
the minimal module's complete measured input surface and the arithmetic that closes at 912 bytes. It
is gated on PR #10 only because that pull request owns `tests/selfhost_wire.rs`.

The two traps recorded yesterday still stand: **do not** replace the linear dedup scan (batching
first, index second), and **do not** compute the chunk record's name index (`map[j] == j` always, so
it is untestable rather than easy).

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **`MAX_PARSE_DEPTH` on small stacks**, above.
- **Per-element data slots.** One slot and one interned name per array element is why a 21 KB source
  makes a 16 MB artifact, paid three times over in parallel tables plus the pool they index.
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
