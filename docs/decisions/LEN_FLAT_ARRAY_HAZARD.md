# `Op::Len` on a flat array: a trap held shut by a refusal that is meant to be lifted

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: reported, NOT repaired. The repair is not this line's to make** — it lies in `src/vm.rs`
and `src/verify.rs`, owned by the `v0.2.3` line. Every fact below was measured for this document.

**This is NOT an exploitable hole today, and must not be read as one.** Leg 3 is the reason, and it
was measured after an earlier reading of this hazard assumed the opposite.

## The four legs

| leg | fact | pinned by |
|---|---|---|
| 1 | `verify()` **accepts** a module emitting `Op::Len` on a flat array | `leg_1_...` |
| 2 | executing it yields `InvalidBytecode("Op::Len on a flat array; ...")` | `leg_2_...` |
| 3 | **`Vm::new` itself refuses it**, at every arena size, so the supported path cannot load it | `leg_3_...` |
| 4 | that refusal is **second category** — provable in principle, analysis not implemented | `leg_4_...` |

All four are in `native_codegen/tests/len_flat_array_hazard.rs`, each with its own control.

## The falsified premise, both halves exhibited

`src/vm.rs` justifies returning `InvalidBytecode` with:

> array length is a fixed-size, compile-time constant the compiler folds to a literal (**it never
> emits `Op::Len` on an array**), so a flat body here is a mis-compilation rather than a script error.

The reference compiler emits exactly that, from:

```
for x in if c { a } else { b } { let _d = x; }
```

`Op::Len` fires when the for-in source has no statically known length, and `static_for_in_length` has
no `Expr::If` arm. **The error classification rests on a premise the shipping compiler contradicts.**

## Why it is worth the owning line's attention

`InvalidBytecode` is **the class `verify()` exists to exclude at load time.** This project has had one
instance already: the `Op::IsStruct` witness verified, took a bound, loaded, and trapped. It was
repaired at both root causes.

This is the same class one guard away — but **the guard is not `verify()`, which accepts the module.
It is the resource-bound check**, and leg 4 places that refusal in the category the project defines as
liftable. Giving both arms the same length makes the trip count two on every path and provable by
inspection; it is refused anyway, because neither the length guard nor the bound extractor looks
through an `Expr::If`.

**So an unambiguous improvement to the bound extractor, made by someone with no reason to look at
`Op::Len`, converts a rejected program into one that loads and traps.** The improvement is silently
gated on an unrelated repair. Leg 4 fails on the day that happens, which is the point of writing it.

## What this line did NOT do

- **Did not repair it.** The plausible fixes — a load-time rejection of `Op::Len` on a statically flat
  operand, or a corrected error class — are both in read-only files here.
- **Did not claim exploitability.** Leg 3 was measured, not assumed.
- **Did not treat `new_unchecked` as evidence of admissibility.** It is the documented trust-skip, used
  only to ask what the runtime arm does, and the test says so in those terms.
- **Did not pursue lowering `Len` for coverage.** `refused_witness.kel` and
  `probe_len_reachability.rs` settle that: the property that makes the opcode reachable is the
  property that makes the loop unbounded. One fact, not two liftable limitations.

## Possible dispositions, for the owning line

| option | note |
|---|---|
| reject `Op::Len` on a statically flat operand at load time | closes it in the guard that is supposed to close it; the typed pass already reconstructs flat shapes |
| fold the `if`-expression source's length when both arms are statically known | removes the emission, but only for the arms-known case |
| correct the `src/vm.rs` premise | smallest change, and leaves the trap in place |

**No disposition is recommended here.** The trade belongs to the line that owns the files.
