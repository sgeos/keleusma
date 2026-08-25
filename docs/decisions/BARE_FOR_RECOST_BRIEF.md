# Brief — re-cost the bare `for`, and pin the division of labour

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Written 2026-08-25. The handoff costs bare-`for` support as *"a second lowering
across three stage sources"*. **Measured, it is two, and the hardest one is
already written.** This increment records that with evidence and pins it so it
cannot go stale.

## What the measurement found

| stage | state |
|---|---|
| `codegen.kel` | **DONE.** `push_forin`, node kind 16, consumes a 7-word `for_parts` entry and emits the full lowering. Four bare-`for` cases pass through it. |
| the Rust driver | **DONE.** Reads `for_parts` out of the reconstructed body. |
| `reconstruct.kel` | **DECLARED, NEVER WRITTEN.** `for_parts` is an array in the output block with **zero** writes; `limit_parts`, the counted form's equivalent, appears sixteen times. |
| `parse.kel` | **ABSENT.** No mention of the node, the parts, or the kind. |

So the remaining work is `parse.kel` emitting the records and `reconstruct.kel`
populating the parts — and the *lowering itself*, the part that had to reproduce
the reference byte for byte, exists and is exercised.

## Why the estimate mattered and why it was wrong

The three-source figure was written from the correct observation that the bare
and counted forms are **different lowerings** rather than one with an optional
clause. That observation is right and is pinned by a live ratio assertion. The
error was inferring the *work* from the *difference*: two lowerings means two
lowerings must be written, unless one already is.

**`codegen.kel` already had it because the codegen-only corpus drives the
REFERENCE parser**, so it received bare-`for` nodes that `parse.kel` has never
produced. The same corpus split that hid the gap — a construct present in one
corpus and absent from the one that exercises the failing stage — is why the
lowering got written and never connected.

## The specific wrong turns

**Do not report this as "nearly done".** Two stage sources of Keleusma is real
work in a phase machine and a record stream, and the parts layout must match
what `push_forin` reads, position for position. A better estimate is not a small
estimate.

**Do not pin the division of labour by counting lines or mentions loosely.** The
claim is that `for_parts` is declared and never written in `reconstruct.kel`. Pin
that precisely, so an implementation that starts populating it fails the pin and
the reader learns the state changed.

**Do not delete the pin when the work lands.** It is a gap pin: it should fail
when the gap closes, and its successor should say what became of it. Three
sibling pins were retired this way in #273 and one was moved from absence to
verdict in #275.

**Do not re-derive what a live assertion already holds.** The size ratio is
asserted by `the_bare_and_limit_forms_have_different_lowerings`. Cite it; do not
quote a number beside it. Two numbers for one claim in one file is the defect
this session has recorded four times.

## The failure this session has paid for

Estimates written from a correct observation about the *shape* of a problem
rather than from the *state of the tree*. The measurement took ten minutes and
moved the cost by a third of the work. **Read the tree before costing it.**
