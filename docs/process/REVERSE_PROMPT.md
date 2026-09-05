# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-09-04 (session 63) — the `Op::Len` trap is closed, and a second hole is pinned open

## READ FIRST: A MODULE CAN VERIFY, LOAD, AND THEN TRAP, AND IT IS NOT REPAIRED

**A module using floats verifies, loads, and traps `InvalidBytecode` on a runtime built without the
`floats` feature.** Measured with `--no-default-features --features verify`:

| step | result |
|---|---|
| `Module::from_bytes` | accepted |
| `verify()` | **accepted** |
| `Vm::new` | **loaded** |
| call | **`InvalidBytecode`** |

`InvalidBytecode` asserts the artefact should never have been produced. It is the class `verify()`
exists to exclude, so this is a hole in the load-time guarantee rather than a bad program.

**Two independent reasons nothing catches it earlier**, either of which would suffice. `verify.rs`
has **no `floats` gating at all** — not one conditional mentions the feature. And the header check
cannot reject it: loading admits when `got <= max_supported`, and `RUNTIME_FLOAT_BITS_LOG2` is not
gated on the feature either, so a build without floats still advertises the full width.

**Nothing here is corrupt.** The fixture is ordinary reference-compiler output, and omitting floats
is the POINT of the feature — an embedded target is exactly where it is used, and producing bytecode
on one build to run on another is the normal shape for a language that ships precompiled modules.

**Proportionality.** The trap is loud: a clean error at call time, not a wrong answer, a crash, or
memory unsafety. What is wrong is the LAYER.

**Why I pinned it and did not repair it — and what has changed since.** The repair belongs in
`verify()`, is about ten lines, and I prototyped it to validate the pin. My stated objection was that
continuous integration does not run this feature set, so it would be exercised only by the release
gate.

**I have since supplied that verification rather than handing you the objection.** The full
`--no-default-features --features compile,verify` suite was run with the repair and compared against
the same suite without it: **the repair introduces zero new failures.** The only test whose result
changes is the pin itself, which is built to fire when the hole closes.

**And the one semantic worry is moot for locally-compiled code.** The repair refuses a module
CONTAINING a float opcode rather than one that executes it, so a module with unreachable float code
would be refused. Measured: **the LEXER refuses a float literal without the feature**, so no float
program can be compiled on such a build at all. Only imported bytecode is affected — exactly the case
where refusing at load is unambiguously right.

**It is still not landed, and that is deliberate.** I told you in a merged document that this was
your call, and reversing that within the hour would make the record untrustworthy; a deferral is
worth something only if it is honoured. What is left is a one-line semantic judgement, with the
engineering risk removed.

**Getting that evidence required repairing the configuration itself, and that is a finding.**
`--no-default-features --features compile,verify` **did not compile**, and two more tests failed once
it did — five defects, all float-dependent code with no `floats` gate. None was tolerated; every one
was invisible, because the release gate's no-default step does not add `compile,verify` and CI never
omits floats, so **nothing built this combination.** The configuration in which the hole lives was
the configuration nothing exercised. Now green at 105 binaries and 1863 tests. Same family as the
verify-without-floats failure V0.2.2 repaired, which suggests a feature-combination sweep would be
worth more than any single fix in it.

Pinned by `tests/float_opcode_without_floats.rs`; recorded in
[`INVALID_BYTECODE_CENSUS.md`](../decisions/INVALID_BYTECODE_CENSUS.md).

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

## THE CONCERN I RAISED AND THEN MEASURED, WHICH SHRANK IT

I flagged the native-array length as a possible soundness gap and then measured it rather than
leaving it as a worry. **It is not a soundness gap**, and saying so is as much the job as raising it
was. `tests/native_array_length_contract.rs` pins all four rows.

| the native declares | it returns | outcome |
|---|---|---|
| `[Word; 3]` | 3 | iterates 3 times |
| `[Word; 3]` | 5 | iterates 3 times; **the excess is silently dropped** |
| `[Word; 3]` | 1 | **traps `IndexOutOfBounds`**, loudly |
| no signature | anything | **refused at type checking** |

**The iteration bound is not wrong in any row.** The loop runs exactly the declared count or it
traps. What would be unsound is running MORE times than the analysis predicted, and no row does
that. **The memory bound does not come from this type either**: a native's worst-case memory is
host-attested per native, not derived from its declared return type, so an over-allocating native
has broken its own attestation rather than found a compiler defect.

What is left is narrow and real: **an over-long return is silently truncated with no diagnostic.**
Recorded rather than repaired, because validating a native's return against a declared shape at the
call boundary is a design change and your call, not a fix.

**And the fourth row is why the floor cost no capability.** The only native whose array length is
unknown is one the type checker will not admit in iterable position at all. Every row was also
measured against the pre-change compiler by restoring `src/compiler.rs` from the branch point; all
four are identical, so the `Op::Len` removal changed nothing here.

## THE ONE CONCERN THAT REMAINS OPEN

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

## THE CLASS THOSE TWO HOLES BELONG TO IS NOW ENUMERATED

Both were found the same way and neither was predicted, so I enumerated the class rather than
waiting for a third accident. `docs/decisions/INVALID_BYTECODE_CENSUS.md` covers all **46**
construction sites: **17 carry an examined verdict, 29 are explicitly marked not examined.** No site
is claimed unreachable — this tree already carries one retraction on exactly that distinction.

Two defences are worth your attention because neither is visible where it matters:

- **The `Fixed` group is held by two checks that only work together.** `verify()` compares fraction
  bits against the MODULE's declared word width; loading rejects a module whose declared width
  exceeds the RUNTIME's. Loosening the load-time comparison reopens five sites at once, and nothing
  said so anywhere.
- **The composite-form group is held by a canonicalization at the host boundary**, sitting between a
  compiler that bakes a flat access and a marshalling layer that produces a boxed body. Each side
  looks locally correct. If it regresses, seven refusals open at once on legitimate programs. Now
  pinned, after a mutation test in which **my first mutation was aimed at the wrong call site and
  passed**, which would have put a wrong mechanism into the record.

## THE INTENDED NEXT STEP

Nothing large without your answer to the two questions above. **If you want one thing decided, make
it the float repair**: it is small, prototyped, and the only open hole in the load-time guarantee I
know of.

Absent that, the remaining self-directed work is census groups E and I — the structural-index and
operand-range sites, where `verify()` plausibly has a corresponding check and plausibly does not. A
first reading suggests both are mostly corrupt-artefact territory rather than holes reachable from
compiler output, which would make them a defence-in-depth question rather than a guarantee question;
that reading is NOT yet confirmed by a program. After that, the discard-arm census pass six, which
[`DISCARD_ARM_REACHABILITY_BRIEF.md`](../decisions/DISCARD_ARM_REACHABILITY_BRIEF.md) establishes is
a **fixture** problem rather than a harness problem: the pass-five harness is reusable as-is, and
what is missing is source constructs.
