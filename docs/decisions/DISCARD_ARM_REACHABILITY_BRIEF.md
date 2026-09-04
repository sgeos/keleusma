# Brief: the driver's discard-arm reachability census

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: brief, self-directed. **MEASURED 2026-09-04; the result is below and the headline is that
the instrument under-measures, proven by a failed positive control.**

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

## The census, run 2026-09-04

Nineteen arms confirmed by re-derivation, matching the brief. Instrumented with per-arm counters,
driven over 27 corpus sources (15 under `examples/scripts/`, 12 stage sources under
`src/selfhost/kel/`) through nine public driver entry points, then reverted.

| arm | line | hits |
|---|---|---|
| 0 | 646 | 94,392 |
| 1 | 771 | 659,696 |
| 2 | 1090 | 3 |
| 3 | 1862 | 19,068 |
| 4-18 | 2400, 2727, 2861, 3113, 3226, 3357, 3570, 4068, 4303, 4572, 4641, 4692, 4755, 5302, 6899 | 0 |

**Four reached. Fifteen at zero.**

## THE POSITIVE CONTROL FAILED, AND THAT IS THE RESULT

**Arm 18, at line 6899, is the one arm the brief names as already measured** -- the `_ => continue`
in the windowed region loop, whose consequence `tests/selfhost_region_coverage.rs` exists to
quantify, and which the brief calls "the model for what the other eighteen lack". Its own comment
states that five kinds reach it.

**This census reports it as zero.**

An arm with independent evidence of reachability, and a test that counts what flows through it,
measured as unreached. The cause is identified and specific: the arm's driver is
`wire_windowed_via_kel`, and the census never called it. The nine entry points probed cover the
lexing, parsing, reconstruction and codegen path; they do not cover wire emission.

**So the fifteen zeros are a statement about this workload's reach, not about those arms.** Four is
a FLOOR on reachability, not a count of reachable arms. Reporting "fifteen arms are never reached"
would have been the confident guess dressed as an audit that the brief warned against -- arrived at
by measurement instead of by reading, which makes it more persuasive and no more true.

## What the next run needs, so it is not re-derived

Add the wire-emission drivers, `wire_windowed_via_kel` foremost, which need a compiled `Module` and
a region list rather than a source string. `tests/selfhost_region_coverage.rs` has the setup to copy.
Until then the census covers one half of the driver.

## Two incidental findings worth keeping

**The corpus is dominated by two sources.** `parse.kel` and `wire.kel` cost minutes per entry point
-- 243 seconds for a single `try_parse_functions` on `parse.kel`, 138 for `self_host_compile` on
`wire.kel`. That is why the self-host binaries dominate the release gate, and why that gate could
not finish under machine load on 2026-09-03.

**A count of sources that "compiled" is not available from this harness.** After each probe was
given its own `catch_unwind` -- necessary, because one guard around all nine meant a source panicking
on the first never exercised the other eight -- the outer success count became structurally always
true. An earlier, narrower run measured 10 of 27 sources panicking through `self_host_compile`
alone. **Do not read a compiled-count out of this harness; it cannot produce one.**
