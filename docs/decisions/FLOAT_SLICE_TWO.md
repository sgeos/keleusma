# Float slice two: one route opened, and the differential agreed

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: the constant route is open and verified END TO END. The other three routes still refuse.**

## What changed and why only this

The module-level float guard closes four routes. **Only one had a lowering behind it.**

| route | built? | disposition |
|---|---|---|
| chunk **constant** | **yes** — constant, conversions, float `Add`/`Sub`/`Mul` | **OPENED** |
| chunk signature | no — the entry ABI is unbuilt | still refuses |
| native return shape | no — no native float ABI | still refuses |
| data slot | no — an open ABI question | still refuses |

The route's own message said it was closed because *"the integer arithmetic lowering would silently
miscompile it"*. **That is no longer the lowering.** The coarse route guard was replaced by a finer
one — the operand whitelist — rather than removed: a float reaching division, a comparison, a
composite or a native still fails closed at the use.

## The verification is EXECUTION, not lowering

`float_witness.kel` now runs in the **corpus differential** against the virtual machine and **agrees**.

> **EXECUTED AND AGREEING: 61 → 62**

That is a stronger check than slice one's, which compared a hand-built chunk through `lower_chunk`.
This runs the whole module the way every other corpus subject is run.

## Census movement, stated with cause

| figure | before | after | cause |
|---|---|---|---|
| **opcodes the backend lowers** | 61 of 66 | **63 of 66** | the two conversions are now VISITED, not merely implemented |
| **UNPROVEN opcodes** | 3 | **1** | only `Reset` remains; both float conversions resolved |
| **modules lowering end to end** | 66 | **67** | `float_witness.kel` |
| **chunks fully lowerable** | 1070 | **1072** of 1074 | its two chunks |
| **opcode instances lowering** | 89841 | **89854** of 89940 | the same chunks |
| modules refused by the backend | 3 | **2** | `Len` and `Stream` remain |
| differential executing and agreeing | 61 | **62** | `float_witness.kel` now runs |

**Every one of these moved for the same single cause**, which is why they are listed together: the
module is no longer refused, so its chunks and their opcodes enter every census that walks the corpus.

## Three pins went red, all correctly, and all were updated rather than deleted

- **`float_abi_scope`** required that some module be refused for a float. **Its own message anticipated
  this** — *"Either floats were implemented, in which case this file's premise is spent"*. The scope
  measurement it made was **correct**: exactly one module, reached by a constant and not a signature.
  Recording that the prediction held is why the file is kept.
- **`float_guard_routes`** asserted a float constant refuses the module. It no longer does, by
  intent. Updated, and **renamed** — the old name said `..._refuses_...` while asserting the opposite.
- The conversions pin from slice one continues to hold.

## What is still NOT built

The **entry ABI** — the piece the operator's ruling names — remains unbuilt and has **no corpus
witness**, since no corpus module carries a float in a signature. Also unbuilt: float shared slots,
division, comparisons, and `f32`.

## Three more pins went red, all correctly

- **`remaining_refusals`** pinned the refusal set at 3; it is 2. The count moved with its cause
  recorded rather than quietly edited.
- **The corpus no longer contains a MODULE-LEVEL refusal at all** — the float guard was the only one,
  and `Len` and `Stream` are chunk-level. Two assertions were **inverted to assert the zero** rather
  than deleted, because an unattributable refusal is what makes a coverage figure overstate and it must
  announce itself if it returns.
- **`differential`'s unsupported-opcode subject retired.** Its list already records five predecessors
  that retired as they entered the subset — composite construction, array indexing, nested reads, tuple
  fields, static strings — and the float constant is the sixth. **Its successor is a float in a
  SIGNATURE**, a different route of the same guard, still closed because the entry ABI is unbuilt.
  That is the natural next subject rather than a hunted one.
