# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## The third correction in a row, all in the same direction

| I reported | why it was wrong |
|---|---|
| 32 detected, 16 undetected | my census drove subjects with no stage seed |
| 38 / 12 | three mutation sites was too thin a basis |
| **39 / 11** | two copies of the site *selection* disagreed |

**Sweeping sixteen sites instead of three moves six more out of undetected.** Only **three** of the ten
self-hosted stages remain — `codegen`, `parse`, `verify_datalayout` — down from the eight I reported
last increment.

Every one of these corrections found the subjects **better** than I first said. I am reporting that
pattern because it is a bias in my measurements, not a run of luck.

## What the evidence now supports, and where it supports nothing

Two explanations were live: **the site** was never executed, or **the observable** does not reflect the
computation.

| subject | sites | tried | comparisons | verdict |
|---|---|---|---|---|
| `verify_datalayout.kel` | 9 | **9, exhaustive** | 5 | points at the **observable** |
| `rogue_gear.kel` | 1 | **1, exhaustive** | 1 | points at the **observable** |
| `codegen.kel` | 845 | 16 | 15 | **distinguishes nothing** |
| `parse.kel` | 1015 | 16 | 15 | **distinguishes nothing** |

For the first two, exhaustion excludes the sampling explanation. For the last two, 16 of ~1000 sites
supports neither reading, and I have said so rather than letting the table imply otherwise. **3198
sites beyond the cap went unexercised**, which the test prints, because an unprinted cap reads as
exhaustive.

**Six subjects produced zero comparisons**, so their result means *nothing ran*, not *nothing was
noticed*. `wire.kel` is the striking one: 929 applicable sites, not one usable mutant.

## The defect I fixed in my own instruments, which was the same shape twice

The deep sweep re-derived its membership using **its own copy** of the probe, which handled a faulting
mutant differently from the census's copy. They disagreed about `verify_typed.kel`. I extracted a
shared probe — and then the **selection** disagreed too, one copy picking the middle site as `len / 2`
and the other `(total - 1) / 2`. `verify_typed` moved again, which is what took 38/12 to 39/11.

**A disagreement between two copies of the same query is invisible unless something compares them**,
and nothing did. Both are single functions now. I should have reached for that the first time.

## Verification

Both suites run **sequentially** (parallel invalidates the perf canary, 57x).

| | result |
|---|---|
| workspace | **2491 passed, 0 failed, 92 binaries**, cargo exit 0 |
| `native_codegen` gate step | **369 passed, 0 failed, 74 binaries**, exit 0 (fmt, clippy `-D warnings`, test, `doc -D warnings`) |
| censuses | 61 of 66; `["Len"]`; 1070 of 1074; 89841 of 89940 — all unmoved |

**No absorption was needed**: already zero unabsorbed.

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
