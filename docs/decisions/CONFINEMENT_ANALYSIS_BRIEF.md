# Brief — the confinement analysis, day one

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Written 2026-08-24 for autonomous execution. The operator commissioned this
analysis on 2026-08-19 ("Confinement analysis needs to be added, as it is
important for the backend native code generation. General confinement analysis
has not been solved, but there are solutions that are good enough to be
practically useful"), and again in ruling 4. **The interface was settled before
this brief and is not reopened here.**

## The goal, in one line

For each composite construction site in a chunk, answer *is the region this site
builds unreachable once its enclosing iteration ends?* — as **yes / no / cannot
establish**, per site, over a chunk the caller already holds.

## Why the third value is load-bearing

Soundness is identical whether an unestablished flow is reported `no` or
`cannot establish`: both are treated as escaping. **The difference is
measurement.** Folding them together makes it impossible to tell an analysis
that improved from one that did not, because the aggregate `no` count moves for
two unrelated reasons. The operator's standard is *useful and sound, not
complete*, and a completeness gradient cannot be observed without the third
value.

## Where it lands, and why not the other candidate

`src/` in the `keleusma` crate. The `v0.3.0` line's reasoning, accepted: one
predicate with two consumers, sound over `verify()`'s acceptance surface which
is this line's, and `src/` is covered by continuous integration where
`native_codegen/` is not.

## The two day-one features, and the measurement that made them mandatory

The other line ran a crude any-`Escapes`-opcode test over the extended corpus.
**Zero of three composite sites survived**: 1 disqualified by `Yield`, 3 by
`SetLocal`, 3 by `Call`. A predicate lacking either feature admits *nothing at
all*, which is sound and worthless.

- **`SetLocal` into a boundary-dead slot.** Without it, a `let` inside a loop
  body disqualifies its own iteration — the ordinary shape of every such
  program. The slot outlives the iteration but is rewritten before any read that
  the next iteration or the code after the loop can perform. This is the proof's
  B1r.
- **A callee summary.** Treating every `Call` as escaping is sound and useless.
  The call graph is acyclic, so a bottom-up summary terminates with no fixpoint.

**`examples/scripts/15_pixel_blend.kel` is the isolate**: a per-iteration
composite with no call in its body. It exists precisely so the predicate can
admit *something* before the callee summary is written. If the analysis cannot
return `yes` for that site with only the `SetLocal` feature, the `SetLocal`
feature is wrong and the callee summary will not rescue it.

## The specific wrong turns, every one already taken in this tree

**Do not classify a dispatch scope as an iterating loop.** `Op::Loop` marks
both. Two separate sessions made this error, one of them twice. The
discriminator is an unconditional `Break` targeting the scope's own exit, and it
is already written down and used by `tests/corpus_pattern_coverage.rs`. A site
inside a `match` filed as a loop site produces a confident wrong answer.

**Do not derive the escaping set from the opcodes that come to mind.**
`tests/composite_escape_routes.rs` classifies all 66 against the `Op` enum and
fails when the set changes. The escaping five are `Yield`, `SetLocal`,
`Return`, `CallExternalNative`, `CallVerifiedNative`. Consume that enumeration;
do not restate it, because a restatement drifts silently.

**Do not report a `CopiesOut` route as escaping.** `SetData` and
`SetDataIndexed` copy bytes; both are backed by execution, not by reading
dispatch. Treating a copy as an alias is sound but it disqualifies
`14_frame_log.kel`, and the corpus then measures nothing.

**Do not write a check whose green is satisfied by a different part of the
subject than the one it names.** Three instances of this landed in one day
during session 52: a scope narrower than the class it claimed, a mutation that
failed to compile and read as silence, and a corpus test measuring presence
where admissibility was the question. A checker can be total, correct, and green
while its predicate is not the property anyone needed. **State the predicate the
test enforces, then confirm the test fails when that predicate is violated** —
by an edit that compiles.

**Do not claim a verdict the analysis reached by defaulting.** If a site comes
back `yes` because a route was never modelled rather than because it was ruled
out, that is unsoundness wearing a verdict. Every `yes` must be traceable to an
exhaustive case over the escape classification.

## What is out of scope

Whole-module verdicts. Slot assignment or any actual reuse — this brief
produces the *predicate*, not the transformation. Recursive call graphs, which
the language does not admit. Completeness.
