# Order 1: what is measured, and the judgement that is not mine to make

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: assessment, 2026-09-07, V0.3.X line. **This does NOT declare the gate met.** It converts
*"nothing has ever said whether it is met"* into a statement of what is measured and what remains a
judgement.

---

## The gate, as the roadmap words it

[`V0_3_X_ROADMAP.md`](../roadmap/V0_3_X_ROADMAP.md), order 1:

> *"The self-hosted compiler's own bytecode runs correctly as native code, differential-tested against
> the VM."*

## What the harness measures today

From `corpus_differential`'s Order-1 report, re-derived rather than quoted from an earlier stamp:

| column | count | which |
|---|---|---|
| stage sources found | **12** | the `src/selfhost/kel/` pipeline |
| EXECUTE and AGREE | **10** | driven at 3 to 5 subjects each |
| agree but VACUOUS | **1** | `verify_datalayout.kel` |
| EXEMPT | **1** | `wire.kel` |
| **DISAGREE** | **0** | — |

**Strength of the agreement, because a stage count alone is not it.** Every stage entry is a stream,
so the argument-vector count is the wrong measure. The comparison happens per TICK: **2460 result
comparisons across the ten, minimum 180 and maximum 300 per stage.** The thinnest seeded stage carries
three subjects, and no seeded subject was declined.

## The two that do not execute, and why neither is a backend failure

**`verify_datalayout.kel` cannot be driven AT ALL, by joint agreement between the lines.** Its verdict
accumulates across three differently-encoded phases in the retained buffer, so a single seeded buffer
cannot produce a verdict. It has no seed accessor deliberately. **This is a property of the stage's
interface, not of native lowering.**

**`wire.kel` faults, and the two sides fault IDENTICALLY.** It is exempt as *fault-comparable*:
`IndexOutOfBounds(1570808, 65536)` on both. So the backend and the virtual machine **agree about the
fault**; what is missing is an execution, not an agreement. The bound is `wire.bytes: [Byte; 65536]`
and the cause is under investigation by the owning line — see
[`NATIVE_MUTATION_CENSUS.md`](./NATIVE_MUTATION_CENSUS.md).

## THE JUDGEMENT, NAMED RATHER THAN MADE

**Zero stages disagree.** Every stage that can be driven is driven, and agrees at every tick. That is
the whole of what has been measured, and it is stated without a verdict attached because the verdict
turns on a question of interpretation that this line should not settle alone:

> **Does "runs correctly as native code" require a stage to EXECUTE, or is agreeing about a fault, and
> being undriveable by design, consistent with the gate?**

Three readings are available and they give different answers:

- **Strict**: 10 of 12 execute, so the gate is not met until `wire.kel` runs and
  `verify_datalayout.kel` becomes driveable. Under this reading the gate may never be met, since one
  of the two is blocked by design rather than by effort.
- **Agreement-based**: 11 of 12 agree with the virtual machine — ten by executing, one by faulting
  identically — and the twelfth produces no verdict on either side, so there is nothing to diverge.
- **Divergence-based**: the oracle's purpose is to catch native output diverging from verified
  semantics; **nothing diverges**, so the gate is met.

## Why this document does not choose

**`corpus_differential` deliberately declines to assert the gate**, and its reason is on the record:
*"'Eleven of twelve agree' is the shape of headline this file already inflated once"* — when
`is_vacuous` asked whether a seeded segment was non-zero, which it is before a module executes a
single operation, and three stages left the vacuous set for no reason at all.

**That caution is right and this document keeps it.** What was missing was not a verdict but a
statement of the position, so that whoever does decide is choosing between named readings rather than
re-deriving the figures. **A milestone declared met on the wrong reading is worse than one left open.**

## What would move it, without deciding anything

- **`wire.kel` executing** would take the strict reading to 11 of 12 and remove the sharpest objection.
  It is also what gates mutation coverage for four opcodes.
- **A ruling on `verify_datalayout.kel`** — whether a stage that cannot produce a verdict on either
  side counts against a differential gate — would settle the remaining member either way.
