# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## The thing I most want you to see

**This backend lowers modules the virtual machine would refuse to load, and nothing said the caller
must not.**

Mutating `04_for_in.kel` by one `CheckedAdd` -> `CheckedSub` gives well-formed bytecode — same arity,
same types — that behaves like this:

| check | result |
|---|---|
| `verify()` | **accepts** |
| `auto_arena_capacity_for`, `module_wcmu`, `Vm::new` | **all reject** — no statically extractable iteration bound |
| this backend's `module_refusals` | **accepts** |
| the lowered code | **SIGBUS** |

`lower_module` documented no admissibility precondition and checked none. **Verified is not enough**:
`Vm::new` additionally requires a resource bound, and that bound is what this project sells. An
ahead-of-time path that runs what the bound analysis refuses is a hole in the value proposition, not
merely a crash.

**Measured before deciding: 66 modules lower, 0 unbounded.** So it is a precondition gap and not a live
defect. I documented the precondition and pinned the corpus rather than enforcing it, because
enforcement couples a pure lowering function to the resource analysis and pays that on every call.
**Enforcement is a real option and I have not ruled it out** — the disposition is in the record.

## How it was found, which I think is the reusable part

Not by reading the source. It fell out of an unrelated measurement — a sweep asking which differential
subjects would notice a wrong backend. **The sweep crashed, and the crash was the finding**, larger
than the census that produced it.

## The census it came from, now complete

`native_codegen/tests/probe_agreement_depth.rs` had sized a blind spot and declined to classify it. Having just floored the
"61 executing and agreeing" figure, I asked whether it measures what its name says.

**32 detected a mutated backend, 16 did not, 10 had no mutation site.** The 16 include every
self-hosted stage, consistent with the existing note that stages read input the harness supplies as
zeros. **Nothing was deleted or exempted** — undetected against one pre-registered family is not
"detects nothing", and the unmeasured classes are reported separately rather than folded in.

## Two corrections to my own work, since they bear on how much to trust the above

**My pre-registered mutation family was wrong** and matched a site in 4 modules of 65: Keleusma is
total, so the corpus emits `CheckedAdd`, not `Add`. **My own non-vacuity assertion caught it.** The
family was amended before any subject had been classified, so it cannot have been tuned to results.

**I anticipated the trap risk and still under-estimated it.** After the admissibility filter removed
the SIGBUS, an admissible mutant produced SIGTRAP, needing a second filter.

## Verification

Both suites run **sequentially** (parallel invalidates the perf canary, 57x).

| | result |
|---|---|
| workspace | **2491 passed, 0 failed, 92 binaries**, cargo exit 0 |
| `native_codegen` gate step | **368 passed, 0 failed, 74 binaries**, exit 0 (fmt, clippy `-D warnings`, test, `doc -D warnings`) |
| censuses | 61 of 66; NAMED REFUSED `["Len"]`; 1070 of 1074; 89841 of 89940 — all unmoved |

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
