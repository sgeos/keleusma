# Brief: the driver's discard-arm reachability census

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: brief, self-directed. **Not started.** Written 2026-08-31 so the next session can pick
it up without re-deriving the reasoning.

## The observation

`src/selfhost/mod.rs` carries **19 silent-discard match arms** (`_ => {}`, `_ => 0`, `_ => None`,
`_ => continue`). `src/wire_schema.rs` carries none.

Exactly one of the nineteen is measured: the `_ => continue` in the windowed region loop, whose
consequence is that unrouted kinds are left as zeros and which `tests/selfhost_region_coverage.rs`
exists to quantify. That arm is the model for what the other eighteen lack -- and it is worth noting
that routing `SHARED_LAYOUT` out of it took one increment once someone looked.

## DO NOT AUDIT THESE BY READING. This is the whole point of the brief.

The obvious move is to read all nineteen and judge each benign or suspect. **That produces a list of
"probably fine" with no evidence attached, which is worse than no list because it looks like
coverage.** This tree has recorded the failure repeatedly: three wrong sizings came from reasoning
about a structure instead of reading its consumer, and this session alone produced eight instrument
errors, one of which reached a merged commit message.

More pointedly: on this codebase, in this session, measurement overturned my reading of what code
must do **three times out of three** -- the borrowed-argument coherence question, the region batch
bound, and the corpus's escape content. A reading-based verdict on nineteen arms would be a
confident guess dressed as an audit.

## The instrument that would work, and it has a precedent here

**Measure which arms are REACHED while compiling the corpus.** This repository has done exactly
this: an emit-command census on 2026-08-14 instrumented every command over the whole corpus and
found two kinds whose emitters existed but which no caller ever selected.

The three outcomes and what each means:

| observation | meaning |
|---|---|
| never reached on any corpus input | **unmeasured**, not safe. Its safety is unknown |
| reached often, discarding | the candidate defect: something real arrives and is dropped |
| reached only outside the corpus | the interesting middle, and where the string-literal class lives |

## The trap, stated because it is the likely wrong turn

**Do not convert this into a rule that a `_ =>` arm is a defect.** Most exhaustive-by-construction
matches want one, and replacing them with panics in `no_std` code that runs a verifier would trade a
silent discard for a crash. **The claim to establish is per-arm REACHABILITY, not per-arm style.**

Second trap: do not fix what the census finds in the same increment. A reached-and-discarding arm may
be correct, may be a defect, or may be a decision that is not this line's to make. Measure first,
record, and let the disposition be its own work with its own evidence.

## What "done" is not

It is not nineteen paragraphs of judgement. It is a figure the tree recomputes, with the unmeasured
arms named as unmeasured rather than assumed benign.
