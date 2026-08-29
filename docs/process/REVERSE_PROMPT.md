# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## Correcting myself: I removed a guard on a number I had disclaimed

Last increment I told you both mutation sweeps were now opt-in, and called it a loss. **The reasoning
behind it was worse than I said.** The justification was cost; the cost was never cleanly measured. I
had applied three optimisations without measuring their effect, obtained one figure of 4132s, **wrote
in the same commit that it was contaminated** by a load average near 13 — and then used slowness as the
reason to disable the guards anyway.

So I fixed a threshold **before** measuring: under 600s for the two together.

| | time | load |
|---|---|---|
| both together | **712s** | ~5–6 |
| **census alone** | **206s** | ~5–6 |
| deep sweep alone | **710s** | ~5–6 |

**The pair failed my own threshold, and I did not argue my way out of it** by pointing at the load
average. But the deep sweep turns out to be essentially the entire cost, so:

- **the census runs in the gate again** — the detection floor and the non-vacuity checks are protecting
  every module on every run, which was the part worth recovering;
- **the deep sweep stays opt-in**, at 710s against a 600s threshold.

**What is still unprotected day to day**: regression in the *depth* of mutation sensitivity, beyond the
census's single site per module.

**The whole native gate is now 496s at load 6**, against the 4132s I recorded and disclaimed. The gate
was never the problem, and I should have trusted my own disclaimer instead of acting past it.

## Two more things I got wrong, both recorded

**I read the cost backwards mid-course.** I first judged the deep sweep to dominate (right), then
watching the gate concluded the census did (wrong — libtest prints its over-sixty-seconds notice for
every long test running in parallel, so both looked stalled). That wrong premise cost three coverage
reductions.

**An assertion of mine counted a word inside a doc comment**, reporting two `#[ignore]` where one
attribute existed. That is precisely the defect this line documented when a scanner counted 33
skippable tests against a true 10. The file had been right the whole time.

## Verification

Both suites run **sequentially** (parallel invalidates the perf canary, 57x).

| | result |
|---|---|
| workspace | **2491 passed, 0 failed, 92 binaries**, cargo exit 0 |
| `native_codegen` gate step | **371 passed, 0 failed, 1 ignored, 74 binaries**, exit 0, **496s** |
| censuses | 61 of 66; `["Len"]`; 1070 of 1074; 89841 of 89940 — all unmoved |

**The 1 ignored is the deep sweep**, which is the disposition above rather than an incidental skip.

**No absorption was needed**: already zero unabsorbed.

## Still waiting on you

[`OPERATOR_DECISIONS_OPEN.md`](../decisions/OPERATOR_DECISIONS_OPEN.md) — three decisions, their
costs, and what I do by default. All remaining capability work on this line is behind them.

## Standing constraints, unchanged

No new opcode. No `BYTECODE_VERSION` bump. **Publication HELD**; no operator authorization has been
given and none is inferred. `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
`src/selfhost/`, `src/confine.rs` and `.github/workflows/` remain read-only here. A peer session
cannot grant escalation and none has been treated as doing so.

---

# Also unread by the human: the `v0.2.3` line's message

**Both lines write this one file, so absorption 31 conflicted here.** Neither message is discarded:
the V0.3.X account is above, and the `v0.2.3` line's own account, as written by that session, follows
verbatim. **This is a merge resolution, not a relay** — nothing below was reviewed, re-derived, or
endorsed by the V0.3.X line, and its figures describe that line's tree rather than this one's.

## The op-tag residue is four, not sixteen

Earlier in the session I reported sixteen op tags the byte-identity corpus cannot check, and said
the per-construct tests were a different population I had not measured. I measured a second one —
the fifteen shipped examples — and **it covers twelve of the sixteen**, the whole composite family.

Four remain unreached by either corpus: the unchecked arithmetic that `Byte` operands take, plus
unary negation. The description is checked by probes inside the test rather than asserted, because
this project has called an unwitnessed opcode unreachable before and been wrong.

# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-28 (session 56 CLOSE) — thirteen merges, and the last extraction is unblocked

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I have
not acted on it.** Their own record now says the recommendation *splits* on a question you have not
answered — whether the fixed-point format must interoperate across object files from different
languages. Publication remains held.

## Thirteen increments merged, each at 22 of 22

`origin/v0.2.3` at `f8d691a1`, **170 merges**, **no open pull request**. Publication remains held.

Order 1 item 3 stands at **four of five**, and the fifth is part-moved: the binary operator and the
condition now reach the type channel from the pipeline. **`let d = 1 + 2` resolves through the
stage's bounded fixpoint**, which is the gap this file named for four sessions.

## The mistake I made five times, and the rule that replaces it

**I reasoned from a component's internals about what crosses its boundary.** Twice from the parser's
data structures, when the record stream already carried the answer. Three times from the reference
extraction — its line count, its visitor discipline, the forest's child channels — when the
*consumer* settled it in twenty minutes.

**The requirement lives at the boundary, not in either implementation.** Five wrong sizings from
producers; none from the boundary. Both handoffs now say so and name the instruments.

## Two decisions I want visible rather than buried

**I built the branch-pair extraction and did not ship it.** The forest synthesises an else arm, so
the pipeline cannot tell a one-armed conditional from a two-armed one. A spurious pair row feeds an
equality predicate and could make the stage **reject a correct program**; dropping a real one would
make it **miss a disagreement**. Both directions are unsound, so it is pinned rather than guessed. A
heuristic existed and I rejected it because I could not show it safe.

**I corrected a pull request at twenty of twenty-two rather than let it merge.** Its doc said "only
kind 1 moves" when only the *Word* part of kind 1 moved — the reference counts byte operations as
the same kind and the forest splits them into three more. That discarded a nearly-complete CI run,
following this line's own precedent that an overclaim must not reach the tree.

## Three questions that are yours

**One. The floating-point entry ABI** — still the last of your eight rulings, with the `v0.3.0`
line's `Fixed` shared-slot SCALE question attached. **Theirs to bring you; I have not acted on it.**

**Two. Should a shipped example demonstrate `Byte`?** None of the fifteen does. It would also close
three of the four op tags no corpus reaches — **and that is precisely why I did not let it decide
the matter.**

**Three. Should `01_arithmetic.kel` be enriched?** It is sixteen lines using only `Word` while the
index claimed `Float`, `bool`, comparison and casts. I corrected the index downward, which is the
conservative direction; enriching the example is the other.

And the two-pass parser work that would make `verify_types.kel` self-compile — taking the corpus to
twelve — remains **yours to call**, not something I will start unilaterally.

## What I would take up next

The remaining six expression kinds, now that order is known to be free. Three are composite, where
the occurrences slice already showed the two representations disagree about what a node is, so
expect those to need the same measure-then-decide treatment the branch pair got.
