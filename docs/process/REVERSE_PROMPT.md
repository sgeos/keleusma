# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-24 (session 52 close) — eight rulings, six done, and a third line now exists

## NOTHING IS WAITING ON YOU

`origin/v0.2.3` is at `dadbce7e`, **139 merges**, no open pull request, working tree clean, operator
queue empty. Publication remains held.

**Eight rulings landed. Six are implemented.** The two that are not are *work*, not decisions:

| ruling | state |
|---|---|
| floating-point entry ABI — yes, FP feature-gated, `Fixed` always available | **authorized, not started** |
| confinement analysis — add it, useful-and-sound standard, shared crate | **commissioned, not started** |
| Theorem B2 adoption | **unruled in either direction**, and recorded so it is not read as declined |
| publication | held |
| `GRAMMAR.md` cross-reference | done |
| CI `Doc` job covering `self-host` | done |
| merge sequence — proof line into this one, `v0.3.0` rebases | relayed; both lines took it to their own operators |

## The two open items, and why neither is small

**The floating-point ABI has an asymmetry the other line had not seen.** Your gating maps onto
`floats`, an existing default-on feature — but the two halves gate *differently*. The FP entry ABI
may assume `floats`, so a `--no-default-features` build must keep the un-floated signature **valid
rather than replaced**; while **`Fixed` is unconditional**, so their `slot_entry` cannot keep
refusing it behind a float gate. That is the harder half and it is not feature-gated. Both lines have
started nothing and both said so.

**The confinement analysis has its interface settled and two day-one requirements.** Per-site,
three-valued — *yes / no / cannot establish*, with the third distinct, because folding it into `no`
loses the measurement that says whether the analysis is improving. And the other line measured why
two features are mandatory rather than optional: with the corpus extended, **zero of three composite
sites survive a crude escape test** — disqualified by `SetLocal` and by `Call`. A predicate lacking
either admits nothing at all.

## What this session actually did

**Both directed items are done.** `verify()` now floors loop-body pops at the entry height, which
cost **zero of 588 loop instances** as measured beforehand. And the corpus — which turned out to
contain **no `loop main`, no data segment, and not one composite built inside an iterating loop** —
now carries four scripts covering the shapes, pinned so a gap cannot silently return.

**An adversarial audit of the proof arrived late and asked two questions about my surface.** Both
answers were fine. `Op::Reset` placement is **enforced**, not emission-only. Break edges genuinely
are never compared to the loop entry stack — and that is **load-bearing**, since 18 dispatch scopes
carry `match` arm values across the break and comparing them to entry would refuse `match`.

**Chasing the second found a defect in my own table**, which the proof cites: I had classified
`Break` as carrying no region, when it consumes nothing and transfers control *with the whole
operand stack*. Reclassified — and the reason it is still not an escape is not that it cannot carry
a region but that it **ends the scope**.

## The thing I would flag if you read only one paragraph

**Three separate checks I wrote this session could not fail**, each satisfied by a different part of
the document from the one it was about — a translation clause, a test citation, a README index.
**Mutation caught all three; reading caught none.** I have written the rule into the handoff, because
the individual fixes are worth less than the pattern.

## A language decision is on the record

[`../decisions/YIELD_OWNERSHIP_MODE.md`](../decisions/YIELD_OWNERSHIP_MODE.md) — `ref`/`out` on a
yielding declaration's return signature, accepted in principle, **not scheduled**, V0.3.0 or later,
no new opcode. It names six open questions it does not settle, the buffer-size query and the
`Text`/opaque depth limit being the two that would bite an implementer first.

## Next intended step

**Order 1, which did not move this session.** Bare-`for` support in `parse.kel` is the largest single
win — a second lowering at 24 ops against 68, and closing it would let `wire.kel` self-compile and
join the byte-identity corpus for the first time. The header machine is located and the failing
phase identified.

The two commissioned items above would displace it if you would rather they came first.
