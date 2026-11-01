# One error variant was doing four jobs, and a census read it as English

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: resolved, 2026-08-28.** The misattribution is **demonstrated, not hypothesised** — a failing
assertion was produced before the fix and passes after. **No census figure moved, and the reason it
did not is the finding.**

## The defect

`LowerError::UnsupportedOp(String)` was documented as *"An opcode outside the currently supported
subset"* and constructed at **31 sites** carrying four unrelated conditions:

| class | example | leading word |
|---|---|---|
| an opcode with no lowering, or none for this shape | `GetIndex reading {..} is not lowered` | `GetIndex` — an opcode |
| a type this backend lacks | `chunk 0 has a Float in its signature` | `chunk` — not an opcode |
| **the input's own integrity failed** | `Const({idx}) out of range` | **`Const` — a real opcode** |
| **a defect in this crate** | `Call(..) needs the whole module; lower_module resolves it, lower_chunk cannot` | **`Call` — a real opcode** |

`isa_lowering_census` built its **NAMED REFUSED** column by taking the leading alphanumeric run of the
sentence and keeping it when it matched an ISA opcode name. So **the class of a refusal was decided by
English word order.**

## What was demonstrated

Injecting an out-of-range constant index produced, from the census's own query:

```
Named: {"Const"}, lowered: {}
```

`Const` with no lowering credited — landing in **`refused_only`, the published column** — for a module
whose only fault was a malformed operand. The backend lowers `Const` in nearly every corpus module.

## Why every published figure was nonetheless correct

**The corpus never fires a misattributing site.** The column was clean because of what the corpus
happens to contain, not because the query could not go wrong. That is precisely the distinction
between a guard that holds and a guard that was never reached, and it is why the answer had to come
from **firing the site rather than reading the source**.

## The fix

Four typed variants: `UnsupportedOp { op, detail }` carrying the opcode **as data**,
`UnsupportedShape`, `MalformedInput`, and `Internal`. Changing the variant's shape made the compiler
enumerate every consumer; there was exactly one, the census query, which now reads `op` directly.

The census's silent `isa.contains(head)` filter became a **loud assertion**: an `UnsupportedOp` that
does not name a declared opcode now fails rather than being dropped without trace.

`Internal` is kept distinct from `UnsupportedOp` because a consumer that cannot tell *"your program
uses a feature I lack"* from *"I am broken"* is invited to rewrite a program that was never at fault.

## Figures, re-derived

**Unmoved**: 61 of 66 opcodes lowered; NAMED REFUSED `["Len"]`; UNPROVEN 3; NO CORPUS WITNESS 1;
lowers-and-refuses `["Stream"]`; 1070 of 1074 chunks; 89841 of 89940 instances.

## Two sentences that misdescribed their condition

- *"native lowering does not yet support opcode chunk 0 has a Float in its signature"* — naming an
  opcode called `chunk`.
- The first replacement lead-in produced *"does not support chunk 0 carries a Float CONSTANT"*, still
  ungrammatical, because these messages are **clauses rather than noun phrases**. The lead-in is now
  neutral.

## What is NOT established

- **`Internal` was never fired.** Its three sites are reached only when this crate's own invariants
  break. **That is a fact about this search, not a proof of unreachability**, and the test asserts
  only what can be: the class exists, is distinct, and renders as a defect rather than a missing
  feature.
- The classification of borderline sites is a **judgement**, applied by one rule: *does "the backend
  does not lower opcode X" follow from the condition?* An out-of-range index does not; an unlowered
  `GetIndex` shape does.
- The sweep's module floor is loose (**69 observed, 40 asserted**) because
  `module_count_reconcile.rs` already pins the corpus counts, and a duplicate pin would break on
  ordinary corpus growth without adding evidence.
