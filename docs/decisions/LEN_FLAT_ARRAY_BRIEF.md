# BRIEF — a conservative rejection is the only thing holding a runtime trap shut

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The four measured facts

| # | fact | how |
|---|---|---|
| 1 | `verify()` **accepts** a module emitting `Op::Len` on a flat array | `verify(): None` |
| 2 | Executing it yields `InvalidBytecode("Op::Len on a flat array; ...")` | run under the documented trust-skip |
| 3 | It is **unreachable today**: `Vm::new` itself runs the bound check and refuses | `Vm::new` REJECTED at two arena sizes |
| 4 | That refusal is **second-category** — provable in principle, analysis not implemented | `probe_len_reachability.rs`, `LANGUAGE_DESIGN.md` |

A fifth fact is the one a reader should not skip. `src/vm.rs` justifies returning `InvalidBytecode`
with *"the compiler ... never emits `Op::Len` on an array"*. **The reference compiler emits exactly
that**, from `for x in if c { a } else { b }`. The error classification rests on a premise the shipping
compiler contradicts.

## Why this is worth writing down rather than shrugging at

`InvalidBytecode` is **the class `verify()` exists to exclude at load time**. This project has already
had one instance: the `Op::IsStruct` witness *verified, took a bound, LOADED, and then trapped*. That
was a load-time hole and was repaired at both root causes.

This is the same class one guard away. What differs is which guard holds it shut: **not `verify()`,
which accepts it, but the resource-bound analysis** — and the project's own taxonomy puts that refusal
in the category defined as liftable. So an unambiguous *improvement* to the bound extractor, made by
someone with no reason to look at `Op::Len`, converts a rejected program into one that loads and
traps.

**That is the finding: an improvement is silently gated on an unrelated repair, and nothing says so.**

## What this line may and may not do

`src/vm.rs`, `src/verify.rs`, `src/bytecode.rs` and `src/selfhost/` are **owned by the `v0.2.3` line
and read-only here**. This increment **reports**; it does not repair. The ownership diff over `src/`
and `tests/` must stay empty, so the pinning tests belong in `native_codegen/tests/`.

## Wrong turns to avoid

- **Do not repair it here.** The fix is a load-time rejection or a corrected error class, both in
  read-only files. Reporting is the whole deliverable.
- **Do not report it as an exploitable hole.** It is not reachable through the safe path today, and
  `Vm::new`'s refusal was **measured**, not assumed — an earlier draft of this reasoning assumed only
  `auto_arena_capacity_for` refused, and executing it showed `Vm::new` refuses too. Overstating this
  would spend the owning line's attention on a false alarm.
- **Do not use `new_unchecked` as evidence of admissibility.** It is a documented trust-skip. It is
  legitimate *only* to measure what the runtime arm does, and the test must say so in those terms.
- **Do not chase lowering `Len` for coverage.** `refused_witness.kel` and `probe_len_reachability.rs`
  settle it: the property that makes the opcode reachable is the property that makes the loop
  unbounded. They are one fact, not two liftable limitations.
- **Do not claim the invariant is stale without quoting it.** The claim is that a specific sentence in
  `src/vm.rs` is contradicted by a specific program. Both halves must be exhibited.
- **Do not let the pin be vacuous.** Each of the four legs needs its own assertion, and a leg that
  stops holding must fail loudly rather than pass by never looking — this line has shipped a
  substitution that silently did nothing twice.

## What good looks like

Four independent assertions, one per leg, that fail if any leg moves — including the day the bound
analysis learns to see through an `if`, which is the day this matters.
