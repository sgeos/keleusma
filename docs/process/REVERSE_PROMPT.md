# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## What this increment did, and one thing I want the `v0.2.3` line to see

**Found a runtime trap whose only guard is a rejection the project defines as liftable.**

`src/vm.rs` returns `InvalidBytecode` for `Op::Len` on a flat array, justified by *"it never emits
`Op::Len` on an array"*. **The reference compiler emits exactly that**, from
`for x in if c { a } else { b }`. The error's classification rests on a premise the shipping compiler
contradicts.

Four legs, each measured and separately pinned:

| leg | fact |
|---|---|
| 1 | `verify()` **accepts** the module |
| 2 | executing it yields `InvalidBytecode` |
| 3 | **`Vm::new` itself refuses it** at every arena size — **NOT reachable through the supported path today** |
| 4 | that refusal is **second category**, surviving even when both arms are equal length and the trip count is provable by inspection |

`InvalidBytecode` is the class `verify()` exists to exclude at load time, and this project has had one
instance already, repaired at both root causes. This is the same class one guard away — but the guard
is **not** `verify()`. It is the resource-bound check, and leg 4 puts that refusal in the category
defined as liftable.

**So an improvement to the bound extractor, made by someone with no reason to look at `Op::Len`, turns
a rejected program into one that loads and traps.** The improvement is silently gated on an unrelated
repair. Leg 4 fails the day it happens.

## Where I was wrong, because it changes how much weight to give this

I hypothesised a second load-time hole, reasoning that a host sizing its own arena would bypass the
bound check. **Executing it showed `Vm::new` runs that check itself and refuses.** Had I written the
report from the reasoning rather than the measurement, I would have spent your line's attention on a
false alarm. The report says plainly that this is not exploitable today.

## What I did not do

**Not repaired.** Both plausible fixes — a load-time rejection of `Op::Len` on a statically flat
operand, or a corrected error class — are in `src/vm.rs` and `src/verify.rs`, which this line may read
and must not edit. Three dispositions are laid out in the report with **no recommendation**; the trade
belongs to the line that owns the files.

**Not pursued for coverage.** `Len` is the last named opcode refusal, and lowering it gains nothing:
the property that makes the opcode reachable is the property that makes the loop unbounded.

## Verification

Both suites run **sequentially** (parallel invalidates the perf canary, 57x).

| | result |
|---|---|
| workspace | **2491 passed, 0 failed, 92 binaries**, cargo exit 0 |
| `native_codegen` gate step | **366 passed, 0 failed, 74 binaries**, exit 0 (fmt, clippy `-D warnings`, test, `doc -D warnings`) |
| censuses | 61 of 66; NAMED REFUSED `["Len"]`; 1070 of 1074; 89841 of 89940 — all unmoved |

**No absorption was needed**: the line was already at zero unabsorbed, recorded as a fact rather than
reported as an absorption performed.

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
