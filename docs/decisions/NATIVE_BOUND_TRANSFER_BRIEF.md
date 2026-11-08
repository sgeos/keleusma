# BRIEF — does the native lowering PRESERVE the bounds the bytecode was verified against?

## Present goals, and why almost all of them are blocked

| goal | state |
|---|---|
| Workstream A, remaining 10 opcodes | **BLOCKED.** 6 wait on the float representation (operator's), `FixedDiv` on runtime-fault lowering deferred to V0.4.0, `Len` and `IsStruct` on witnesses that cannot be loaded or cannot be run, `Reset` structurally unreachable |
| The `Op::IsStruct` load-time hole | **BLOCKED.** `src/verify.rs` is read-only to both lines and its ownership is an open operator question |
| Workstream B, coroutines proper | **DECLINED, not blocked.** The corpus routes around it; the degenerate transform covers nine of eleven stages. Building it now means building an unexercised feature, and this line has repeatedly declined that trade |
| Workstream D, host ABI | **BLOCKED.** String ABI is provisional by operator ruling; float ABI undecided |
| **Workstream E, bounds preserved** | **AVAILABLE, and nothing in it needs an operator** |

**Recommendation: Workstream E.** It is the Order-4 gate, it is the project's entire value
proposition ("definitive WCET and WCMU"), and it is measurable today with APIs that already exist.

## The specific claim to test, and why it is worth testing

The backend's region code carries this in a comment:

> *"Naming the provenance is not decoration. A region of unspecified origin would put the backend's
> memory outside the arena's accounting, and **transferring that bound is the whole property this
> lowering exists to preserve**."*

**That is a claim, and nobody has measured it.** This session has now found five separate cases
where a documented or assumed property was false or vacuous when checked. A comment asserting the
load-bearing property of a workstream is exactly the shape that has been wrong before.

Two checkable sub-questions:

1. **Operand stack.** `MAX_STACK = 64` is what the backend provisions; the verifier computes the
   exact figure as `RuntimeFootprint::max_operand_slots`. **Does 64 cover the corpus?** A chunk
   needing more is refused with `OperandStackTooDeep`, so the answer is currently invisible: the
   refusal would appear in the census as some other opcode's problem or not at all.
2. **Composite region.** `region_total_bytes` is what the backend demands from the host arena;
   `RuntimeFootprint::max_heap_bytes` is what the bytecode was verified to need. **Is the backend's
   demand bounded by the verified figure, or can it exceed it?** If it can exceed it, a host that
   provisioned from the verified bound and then ran native code would under-provision — the bound
   would not have survived lowering, which is the one thing Workstream E exists to prevent.

## Prior failures on this line, and the specific wrong turns to avoid

**Do not conclude "the bound holds" from a green run.** Five times this session a signal answered a
narrower question than the one asked. The controls matter more than the result:

- **A comparison over an empty set passes.** Assert a non-trivial number of chunks compared, and
  print it. `delegated_subject_census` reports 0 subjects and is only meaningful because a must-fire
  control proves the query can return one.
- **A margin measured only where it is comfortable proves nothing.** If every corpus chunk needs 3
  operand slots against a provisioning of 64, that is a fact about the corpus, not about the
  backend. **Report the maximum and the headroom, not a pass/fail.** State plainly that a large
  margin is not a proof.
- **`OperandStackTooDeep` may never have fired.** If so, say so — an untriggered guard is not a
  working guard. Reaching it needs a synthetic chunk, and a must-fire control that constructs one is
  the only way the corpus zero means anything.
- **Do not compare a slot count to a byte count.** `max_operand_slots` is deliberately
  representation-independent; `max_heap_bytes` is real bytes; `region_total_bytes` is real bytes.
  Mixing them silently produces a plausible number. This is the `FixedMul` trap in a new place.
- **Do not "fix" a violation by widening `MAX_STACK`.** If a chunk exceeds it, that is the finding.
  Widening it is a decision about worst-case memory, which is the operator's, and this line has a
  standing rule against trading a correct refusal for coverage.

**Do not edit `src/verify.rs`, `src/vm.rs`, `src/bytecode.rs`, `src/selfhost/`, or
`.github/workflows/`.** All are read-only here, and `verify.rs` additionally has an unresolved
ownership question. Read them; do not repair them.

**Mutate whatever the result rests on.** Every finding this session that survived came with a
mutation showing the instrument was load-bearing. A bound comparison that passes when the
comparison is deleted is not a measurement.

## What a good outcome looks like

Either a stated margin with its denominator, or a violation named precisely. **Both are good
outcomes.** A violation would be the first evidence that Workstream E has real work in it; a margin
with a stated denominator is the first quantitative claim this line can make about bound transfer.

**A bad outcome is a green test that would pass on an empty corpus, a mismatched unit, or a guard
that has never fired.**
