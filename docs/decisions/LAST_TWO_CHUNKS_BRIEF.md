# BRIEF — name the last two chunks the backend will not lower

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Absorption 22 | yes |
| 2 | Name the two remaining unlowered chunks and their causes | yes |
| 3 | Lift whichever is cheaply liftable, with execution evidence | yes, conditionally |
| 4 | Keep the gate green, running the gate rather than `cargo test` | yes |
| 5 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |
| 6 | Lower `Stream` | not in one increment |

## Rationale

Corpus coverage is **1072 of 1074 chunks**. The census reports the two survivors only by workstream —
**one "B (sub-coroutines)" and one "other"** — which is a count, not a finding. **"Other" is not a
cause.** This line has twice now turned a condition into an actionable result by naming it to the
module, the instruction, and the reason, and both times the named cause differed from the obvious
guess.

The frontier is now small enough that naming both is cheap, and the answer determines whether there
is any remaining increment here at all or whether the next real work is `Stream` and the
operator-held ABIs.

## Prior failures to avoid repeating

1. **`stack_growth`/`stack_shrink` are NOT pop and push counts.** They are the peak model and their
   own documentation says so, naming `verify::op_depth_effect` instead. A walk built on them
   mis-attributed a value last increment. **If this increment walks a stack, use `op_depth_effect`.**
2. **The obvious neighbouring instruction was innocent.** Adjacency is not provenance.
3. **A negative test asserted something false** — a subject that was never the blocked case. Check
   that a subject actually exhibits the property before asserting anything about it.
4. **Coverage is not correctness.** A wrong width raises coverage and mispacks silently. Execution
   agreement is the evidence.
5. **Four recorded premises have been found false in consecutive increments.** Re-derive.

## Specific wrong turns to avoid

- **Do not report a workstream label as a cause.** "B (sub-coroutines)" and "other" are buckets. The
  deliverable is module, chunk, instruction and reason.
- **Do not assume the two are independent.** They may share a cause, or the "other" may be a
  consequence of the first. Establish that rather than assuming it.
- **Do not lift anything without execution evidence.** The corpus differential and the source-string
  whole-module differential both exist now; a newly admitted chunk should run and agree.
- **Do not treat "the refusal moved" as "the chunk lowers".** Report the refusal text after any
  change, not just the count.
- **Do not start `Stream`.** If the remaining blocker is the sub-coroutine workstream, that is a
  finding and a stopping point, not an invitation to begin a multi-increment feature here.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
