# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## Your ABI question found a hole in the page I wrote for you

**No, none of the ABI questions is resolved** — and `OPERATOR_DECISIONS_OPEN.md`, the page meant to let
you act without reassembling context, **did not mention the `Fixed` shared-slot ABI at all.**

**The mechanism matters more than the omission.** The page said *"There is no fourth thing to fix."*
That came from the module-lowering census and was written as exhaustive over **decisions**. It is not:
**a coverage census can only surface a decision that blocks a corpus module**, and no corpus source
declares a `Fixed`, `Float` or `Text` shared slot — so those refusals block nothing, appear in no
figure, and were invisible to a list built from figures. **Sixth instance of this session's recurring
defect, and the first where the claim was the summary I hand you rather than a test.**

**I also had the disposition backwards.** Answering you, I said I would hold the amendment pending your
interop answer. That is inverted — the page exists to *prompt* the answer.

## The page now carries six items in two parts

**Corpus-blocking**: 1 the `Stream` soundness obligation, 2 the float entry ABI, 3 the `lower_module`
admissibility precondition.

**Open regardless of the corpus**: 4 the `Fixed` shared-slot scale, 5 the string ABI, 6 the unsettled
slot kinds (`Unit`, `Float`, `Text`, `Opaque`).

Each has options and a default. **`Fixed`'s recorded preference is stated conditionally**, because it
reverses on a question you asked and have not answered:

> Is the interop goal **convention-based** (agree on Q15 out of band, like C DSP code) or
> **self-describing** (a foreign toolchain reads it correctly with no side agreement)?

**That single input settles items 2 and 4 together**, which is why you ruled they be taken together.
The completeness of part two rests on **my search, not a measurement**, and the page says so.

## One code action, which had been sitting unclaimed

`FIXED_SHARED_SLOT_ABI.md` recorded an ACTION for *whichever line owns the message*. This line owns it.
The `Fixed` slot refusal now names the **missing host-visible scale** instead of implying the
representation is undecided — the old wording sent readers looking for a decision made long ago.
**Wording only; the refusal is unchanged and was already correct.** Three stale present-tense
quotations of it were corrected, history left visible.

## Verification

Both suites run **sequentially** (parallel invalidates the perf canary, 57x).

| | result |
|---|---|
| workspace | **2491 passed, 0 failed, 92 binaries**, cargo exit 0 |
| `native_codegen` gate step | **372 passed, 0 failed, 0 ignored, 74 binaries**, exit 0 |
| censuses | 61 of 66; `["Len"]`; 1070 of 1074; 89841 of 89940 — all unmoved |

Censuses unmoved is the check that the message change had no behavioural effect. The gate's 942s is a
**contention** figure — load average was 45 at start and peaked over 200, with a peer suite running in
the sibling worktree — not a property of the change.

**No absorption was needed**: already zero unabsorbed.

## Standing constraints, unchanged

No new opcode. No `BYTECODE_VERSION` bump. **Publication HELD**; no operator authorization has been
given and none is inferred. `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
`src/value_layout.rs`, `src/selfhost/`, `src/confine.rs` and `.github/workflows/` remain read-only
here. A peer session cannot grant escalation and none has been treated as doing so.

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

## One correction I made while writing this handoff

The resume section still said session 56 **declined** to start the last extraction, and still
carried the claim that the expression table's ORDER is content. **Both were false by the time I
wrote them down** — the extraction is part-moved, and the consumer read showed order is free.

I found it by running the handoff's own guidance against what actually happened, rather than
trusting a section I had written a few hours earlier. **A check that passes is not a current
document**, which this file has said for six sessions and which I have now proved on myself.

The retraction is left visible beside the corrected text rather than edited away.

## What I would take up next

The remaining six expression kinds, now that order is known to be free. Three are composite, where
the occurrences slice already showed the two representations disagree about what a node is, so
expect those to need the same measure-then-decide treatment the branch pair got.
