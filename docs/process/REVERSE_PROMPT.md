# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-09-04 (session 63) — the `Op::Len` trap is closed, and one of its two sites was live

## THE TWO QUESTIONS THAT BLOCK EVERYTHING LARGE ARE STILL YOURS

Neither moved this session, and neither is mine to decide. They are stated in full at the top of
[`HANDOFF.md`](./HANDOFF.md).

1. **How does a value enter a `Text<N>`?** A literal is static text, `Text<8>` is dynamic text, and
   the two deliberately do not unify. `GRAMMAR.md` states that no implicit coercion exists, so
   emission needs a surface form — a cast, a constructor, or a method. It appears in every program
   anyone writes with the type.
2. **Is the width bundle worth a breaking change?** `addr_bytes` is taken by 33 signatures across
   five files, 14 of them public, in a crate published at 0.2.2.

## WHAT LANDED: THE COMPILER NO LONGER EMITS AN OPCODE THE MACHINE REFUSES

Both emission sites for `Op::Len` are gone. Each folds the length from the operand's type, or fails
with a compile error naming the unfoldable length. The virtual machine keeps its refusals, which now
defend against a corrupt or hand-built module rather than against the compiler.

**Every iterable form that can carry an array type folds and runs** — eleven tried, all pass, each
asserted on its ITERATION COUNT rather than on compilation, so a wrong bound fails rather than
passing quietly. A wrong bound would have been worse than the trap: a trap is loud and a wrong
iteration count is silent, and the worst-case execution time analysis consumes it.

**Four mutations, four caught**, each by the guard that should catch it: removing the multi-word
length arm, removing the delegation fallback, folding a bound one short, and reinstating an emission.

## THE FINDING I DID NOT GO LOOKING FOR, AND IT IS THE MORE SERIOUS ONE

The recorded hazard was latent: held shut by a loop-bound refusal that the taxonomy calls liftable,
so it needed someone else's future improvement to open.

**The second site needed nothing.** The checked-index construct over a `Multiword` folded its length
through a helper that answers only for array types, fell back to the opcode, and a multi-word body
is flat. Measured against the pre-change baseline by stashing: the program **compiled, passed
`verify()`, took a memory bound, loaded, and trapped `InvalidBytecode`** — the class `verify()`
exists to exclude, reachable on the day it was measured.

It is repaired by folding the multi-word width, so the construct now works rather than being
refused. **Found by enumerating every emission of the opcode**, not by following the witness already
in hand.

## AN ISA OBSERVATION FOR YOU, DELIBERATELY NOT ACTED ON

**No producer for `Op::Len` was found in the reference compiler**: zero emissions in `src/`, and none
in the self-hosted `codegen.kel`. On a project whose opcode count is a design constraint that reads
like a removal candidate.

**I am recording it, not proposing it.** Removing an opcode is a wire change and your call. And the
tree records `Op::IsStruct` being declared producerless and having four producers found by another
line within the hour, so the claim here is "no producer FOUND", never "unreachable". The machine
keeps both refusals regardless.

## TWO CONCERNS I COULD NOT CLOSE

**A native's declared array length is trusted and not checked.** The fold reads a function's declared
return type for the element count, and I found no check validating a native's returned element count
against it. A native declaring `[Word; 3]` and returning five elements yields a folded bound of three
and iterates three times, silently. **Pre-existing and not introduced here** — the call arm already
folded from the declared return type — but the delegation extends the same trust to method calls and
pipelines, which are also calls. Not investigated further.

**The floor has no witness.** No source form reaches the compile error behind it, so it is pinned by
a source scan for the emission form. Its reach is stated in the test: one written shape, in one file.
It would not see an emission written through a differently named binding or built by pushing to the
op vector directly. Recorded as *not found*, never as *unreachable*.

## WHAT THE PROCESS COST THIS TIME

**I corrected every stale claim in `docs/` and `src/` and did not grep `tests/`.** The corpus run
found two tests asserting the old behaviour. The suite caught what my scan did not — the fourth
instance this week of a scan scoped to where its author was looking, and the first where the thing
missed was a test rather than a source file.

Both tests carried their own instruction for this moment and were right on both counts. Neither was
deleted.

## THE INTENDED NEXT STEP

Nothing large without your answer to the two questions above. Absent that, the remaining
self-directed work is the discard-arm census pass six, which
[`DISCARD_ARM_REACHABILITY_BRIEF.md`](../decisions/DISCARD_ARM_REACHABILITY_BRIEF.md) establishes is
a **fixture** problem rather than a harness problem: the pass-five harness is reusable as-is, and
what is missing is source constructs.
