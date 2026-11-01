# Brief — chained array indexing, the last known silent miscompile

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: opened 2026-08-21 (session 50, continued).
**Scope**: `src/selfhost/kel/parse.kel`, plus whatever driver support the records need.
**Constraints**: no new opcode, no `BYTECODE_VERSION` change, and the stage sources must still
self-compile byte-identically.

## What is wrong

`a[0][1]` does not lower to a chained index. `parse.kel` emits records, and they are the WRONG
records:

```text
  a[1]      ->  Local(0), Literal(1), Index                 -- correct
  a[0][1]   ->  Local(0), Literal(0), Index, Literal(1), ArrayLit
```

**The second `[1]` parses as an ARRAY LITERAL.** The postfix index phase (`ps.aa_phase`) arms only
after a let-bound array `Local` is emitted, and nothing re-arms it once an index completes, so the
next `[` falls through to the array-literal branch.

**The chain is not the trigger.** `let b = a[0]; b[1]` diverges too, which rules out chaining and
points at indexing a nested array AT ALL. Both forms are in the boundary table because the split
form is what discriminates the two hypotheses.

## THIS IS THE LAST KNOWN ONE, WHICH IS WHY IT IS WORTH DOING

Four silent miscompiles were closed earlier in this session, and after them the shipping compiler
agrees with the construct-support boundary on all 95 cases. The five that remain non-identical are
all already LABELLED `Diverges` — this is the only one whose repair is specified rather than
open-ended.

**Proportionality, and state it every time.** `self_hosted_compile` cross-checks ops, constant pool
and local count against the reference and refuses on divergence, so a CLI user gets a loud error
rather than a wrong module. The exposure is to direct callers of `self_host_compile*`. Omitting
that sentence overstates this badly, and an earlier revision of the handoff did exactly that.

## What a fix needs, from the recorded specification rather than the symptom

Three coordinated pieces, which is why the previous session stopped rather than started:

1. **A binding record for an array-typed ELEMENT.** There is `let_array` for a scalar element kind
   and `let_array_struct` / `let_array_size` for a struct element. There is nothing for an element
   that is itself an array.
2. **A nested-variant postfix phase.** The machinery already exists as `step_structarrayaccess`
   with `da.fa_index_variant`; this is reuse, not invention.
3. **Re-arming after an index completes**, so a chain continues instead of falling into the
   array-literal branch.

**This is a FEATURE, not a defect fix.** The previous session recorded that judgment explicitly and
it still holds; the difference now is that the specification exists and the surrounding class is
closed, so the work is bounded rather than exploratory.

## The wrong turns, named in advance

1. **DO NOT report it as truncation.** My own first note called the body TRUNCATED — no `SetLocal`,
   no `GetLocal`, no `GetIndex` — which implies codegen dropped ops. It does not. The truncated op
   stream is downstream fallout from a malformed node forest, and that note would have sent the
   next reader to `codegen.kel` when the defect is two stages upstream in `parse.kel`.

2. **DO NOT change both sides of the differential comparison in one increment.** This is how a
   `bool`/`Bool` regression shipped: the oracle was adjusted alongside the thing it judged, so it
   agreed with itself and stayed green. `tests/selfhost_codegen.rs` is the control here.

3. **DO NOT treat a passing boundary as proof.** The boundary now measures BOTH compilers, but its
   reach is still its 95 cases. A repair must be witnessed by a program whose reference output is
   compared BYTE-WISE — ops, constant pool AND local count — because an ops-only comparison called
   the string-literal case clean when it was not.

4. **DO NOT let the `ArrayLit` element-size logic regress.** A nested array LITERAL was repaired
   earlier this session by carrying element size PER NESTING LEVEL; a flat "last array closed" flag
   leaks across siblings and produced 64 where 32 was right — worse than the bug. The index work
   touches adjacent state and must not undo that.

5. **DO NOT write probe sources from memory.** `let mut` is not in this language and I have now
   written it four times. Take probe sources from the corpus or from existing tests, verbatim.

6. **CONFIRM THE REFERENCE ACCEPTS A GENERATED PROGRAM** before concluding anything about the
   stage. Five earlier probes measured something other than what was intended.

7. **STOP AND RECORD IF IT WIDENS.** Three coordinated pieces of parser state machinery is exactly
   the shape of work that grows. If the second piece reveals a fourth, the honest outcome is a
   sharpened specification and a recorded stop, not a half-built feature.

## Why the boundary table is the acceptance test

`nested/array_of_array_index` and `nested/array_of_array_split_index` are both recorded `Diverges`.
A real repair moves BOTH to `Ok`, and the split form moving alone would mean the nested-element
binding was fixed while chaining was not — a partial result worth having, and worth reporting as
partial rather than as success.
