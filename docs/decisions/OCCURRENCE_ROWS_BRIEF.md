# Brief — the fourth type-channel extraction, `occurrence_rows`

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: brief for the increment. Written 2026-08-28, session 56.

## I HAVE NOW MADE THE SAME PREDICTION ERROR TWICE, AND MEASURED IT BEFORE ACTING THIS TIME

After `field_sets` landed I wrote, in three places, that `occurrence_rows` should be expected to be
**harder**, because "two of its four declaration kinds are skipped by the driver and its ident
occurrences are keyed by slot rather than by name".

**The first half of that is misleading in exactly the way the last brief was.** The driver does
discard them. That says nothing about whether the data crosses the boundary, and it does. Measured
with `parse_record_trace`, which is a public instrument that exists precisely so the record stream
can be read from outside the driver:

| declaration | record | carries |
|---|---|---|
| function | code 1 | the name id |
| `data` block | code 9 | the name id, packed as `name * 4 + visibility` |
| `enum` | code 12, variants on 13 | the name id |
| `struct` | code 18 | the name id — collected since the previous slice |
| `use` import | code 10 | **the imported name id**, which the driver throws away |

**All five are on the wire.** The declared half of `occurrence_rows` is therefore the same shape as
`field_sets` turned out to be: a receiver, not an emitter.

**The lesson, stated so it is not learned a third time: "the driver discards X" and "X is
unreachable" are different claims, and the first is evidence for neither direction.** Read the
record stream with the instrument built for it before costing the work.

## WHAT IS GENUINELY UNRESOLVED, AS OPPOSED TO ASSUMED

`occurrence_rows` returns `(declared, occurrences, gave_up)` where an occurrence is
`(name, is_local, is_call)`.

- **Calls** are reachable: the reconstructed forest carries a call node whose argument is a chunk
  index, and `decl_call_rows_from_pipeline` already turns that into a callee NAME.
- **Locals** are reachable: the driver holds parameter names and `let` names per function, which is
  what the binding-rows slice added.
- **A bare identifier occurrence that is neither a call nor a binding site** is the open question. I
  have NOT established which record or node carries it, and this brief does not pretend to. **Find
  that with the trace instrument before designing anything.**
- **`ImportItem::Wildcard`** sets a flag in the reference rather than contributing a name. Whether
  the stage distinguishes a wildcard import is unmeasured.

## THE SPLIT THAT WILL BE NEEDED, AND THE PRECEDENT FOR IT

`use` declarations are handled as `in_use = code != 5`, which discards the name on code 10. That is
the same shape the struct collect replaced, and the same precedent applies: `data_records` and
`enum_records` accumulate in that loop and terminate on code 5. **Follow that; do not invent a
mechanism.**

**And do not widen a skip into a collect for constructs that were never made to work.** Trait and
impl still skip. That split is now guarded, and the guard was needed: re-admitting them left the
struct agreement test passing because its probes contained neither.

## PRIOR FAILURES THIS INCREMENT MUST NOT REPEAT

**COMPARE BY NAME. NEVER BY INDEX.** Three slices have hit this. The reference interns occurrence
names into its own space; the pipeline has another. Carry strings.

**AND ASSERT THE COMPARISON IS ORDER-SENSITIVE IF IT CLAIMS TO BE.** For `field_sets` the
declaration-versus-sorted trap did NOT apply, and saying it did would have been borrowed rigour.
Work out which hazard actually applies here rather than copying the previous test's justification.

**A GUARD WHOSE CORPUS LACKS THE CONSTRUCT IS A GUARD FOR A DIFFERENT QUESTION.** The struct
agreement test could not see trait and impl being re-admitted, and only a mutation showed it. Every
declaration kind this touches — function, data, enum, struct, use, wildcard — needs to appear in a
probe, or the agreement test is silent about it.

**MUTATION-TEST EVERYTHING, AND CONFIRM THE MUTANT COMPILES.** Two mutation attempts in a previous
session failed to compile, producing silence indistinguishable from a guard not firing.

**THE PARITY GUARD WILL FIRE IF THE DECLARATION DISPATCH CHANGES.** That is correct behaviour. It
compares COVERED CODES now rather than arm spellings, so a split passes and a dropped code does
not. If it fires, check whether a code was genuinely lost before touching the guard.

**RUN THE GATE IN SEGMENTS AND ACCOUNT FOR EVERY BINARY.** A whole-workspace sweep was killed last
increment having reported 55 binaries green while **31 never ran**, including the guard that caught
a real drift. Enumerate the test files, subtract those that reported, and run the remainder.

## THE WRONG TURNS SPECIFICALLY

- **Do not claim the extraction is moved if only the declared half is.** `field_sets` moved three of
  its four returned values and says so; the same honesty is required here.
- **Do not add an opcode** and **do not bump `BYTECODE_VERSION`.**
- **Do not gate a source-text guard behind an off-by-default feature.**
- **Do not assume the `gave_up` flag is meaningless.** The reference returns it, and a pipeline
  analogue that silently never gives up is not equivalent — it is a claim that the pipeline is more
  complete, which would need evidence.

---

## OUTCOME, WRITTEN AFTER THE WORK

**The brief's opening claim held.** Every declaration kind was on the wire, and the declared half
moved with no driver change at all. The prediction it was written to correct — that
`occurrence_rows` would be harder — was wrong for the declared half.

**Its warning about `ImportItem::Wildcard` being unmeasured turned out to name the real blocker.**
Measured: `use play` and `use host::*` emit the same record shape, one path record each, and the
reference draws opposite conclusions. So `use` is excluded from the declared set rather than
guessed at, and the gap is pinned in the failing direction.

**Two of its process warnings paid immediately.** The demand that every declaration kind appear in
a probe is now an assertion that fails when one is missing. And "confirm the reference accepts the
program" caught a malformed `use sin;` — a `use` takes no semicolon — before it became a finding
about the stage.

**What it did not anticipate:** that a mutation harness could run nothing and report cleanly. The
command variable was escaped inside a quoted heredoc, so three mutants reported zero compile errors
and no test results. That is the same class as "a mutation attempt that fails to compile is
silence", one layer further out.
