# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## Float slice one is built and differentially verified

Your Option A ruling unblocked real capability work. **One measurement decided its shape before I wrote
anything**: `width_of_declared_shape` discards the scalar kind, so a `Float` and a `Word` are both
eight bytes and **no float arithmetic could be lowered until an operand's kind survived**. Starting
from the phrase "entry ABI" would have built the wrong piece.

**Built**: an operand-kind channel beside the width channel, seeded by the producing opcode, with the
stack staying homogeneous `i64` and floats riding it as bit patterns. Then a float constant, both
conversions, and float `Add`/`Sub`/`Mul`.

**Verified by differential, not by acceptance**: the witness's exact shape — `w as Float`, `+ 1.5`,
`as Word` — agrees with the reference across ten probes including negatives, with a must-fire control
that the program computes something.

## The part I most want you to see: implementing this CREATED a hazard

A module whose float arises from `as Float`, with no float constant and no float in a signature, was
previously refused **only because no float operation existed**. `float_guard_routes.rs` names that
exactly — *"a property of what is unimplemented, not a guard"* — and it **stopped being true the moment
I wrote the operations**. The module-level guard does not cover that shape.

**`Op::Div` was the sharp case**: I added float dispatch to `Add`/`Sub`/`Mul` only, so a division would
have been an integer division of a double's bit pattern — a plausible wrong number, not a fault.

Closed with a **whitelist**: an opcode that consumes a float and was not written for one refuses. A
blacklist would have to name every arm, and missing one is silent.

## Four errors of mine, all caught inside the increment by my own guards

- the kind was lost across the local round trip — the mixed-pair refusal caught it;
- I read the kind **after** popping, against a rule written at `SetLocal`;
- the whitelist's first formulation checked the top two stack entries rather than the operands the
  opcode **pops**, refusing `Op::Const` for a float below it;
- a pin went red **three times**, correctly, and was renamed because a test called `..._is_refused_...`
  that asserts the opposite is the stale label I keep finding.

## What is NOT done, so it is not assumed

**The entry ABI** — the piece your ruling names — is **not built**. No corpus module carries a float in
a signature, so it has no witness here. Also not done: float shared slots, division, comparisons,
`f32`.

**The module guard is unchanged and censuses are unmoved.** Nothing float-carrying reaches
`lower_module`, so the corpus witness is still refused and the conversions are still UNPROVEN — which
is the correct result, not an oversight. **Relaxing that guard is the next decision.**

## Verification

Both suites run **sequentially** (parallel invalidates the perf canary, 57x).

| | result |
|---|---|
| workspace | **2496 passed, 0 failed, 92 binaries**, cargo exit 0 |
| `native_codegen` gate step | **377 passed, 0 failed, 0 ignored, 76 binaries**, exit 0 |
| censuses | 61 of 66; `["Len"]`; 1070 of 1074; 89841 of 89940 — all unmoved |

Gate timings this increment are contention figures; load peaked over 200 with a peer suite in the
sibling worktree.

**No absorption was needed**: already zero unabsorbed.

## Still open, and yours

[`ABI_RULINGS.md`](../decisions/ABI_RULINGS.md) — `Fixed` (three readings; the interop goal decides,
and is unstated), `Text` (your supposition that it was covered is incorrect), `Opaque` (your intent is
already what the handle achieves), `Unit`.

## Standing constraints, unchanged

No new opcode. No `BYTECODE_VERSION` bump. **Publication HELD**; no operator authorization has been
given and none is inferred. `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
`src/value_layout.rs`, `src/selfhost/`, `src/confine.rs` and `.github/workflows/` remain read-only
here. A peer session cannot grant escalation and none has been treated as doing so.

---

# Also unread by the human: the `v0.2.3` line's message

**Both lines write this one file, so absorption 33 conflicted here.** Neither message is discarded:
the V0.3.X account is above, and the `v0.2.3` line's own account follows verbatim. **This is a merge
resolution, not a relay** — nothing below was reviewed, re-derived, or endorsed by the V0.3.X line, and
its figures describe that line's tree rather than this one's.

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-29 (session 57, first increment) — the tail-versus-return claim moves

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I
have not acted on it.** Publication remains held.

## What moved

Expression kind 8 — the tail-versus-return claim — now reaches the type channel from the
pipeline, joining the binary operator and the condition. Three of that extraction's eight kinds
are done. The migrated-extraction count still reads four of five on purpose; naming a partial
migration after the extraction would defeat the pin silently.

This is the row that refuses a function whose body yields something its signature does not
promise. Both halves were already on the wire, so no stage changed and no record was added.

## The hazard that killed the branch pair was present here, and it was discharged

Kind 8 is an equality kind, so a row emitted where the reference emits none could make the stage
**reject a correct program**. A body with no tail expression reconstructs with a **synthesised
payload-0 unit**, which is the same shape as the synthesised else arm that made the branch pair
unshippable.

What separates them is measurable rather than argued: the only source expression that would also
land on a payload-0 unit is a written `()`, and the pipeline refuses that outright. **I pinned
the refusal in the failing direction**, so if `()` ever becomes admissible the test breaks rather
than the descent quietly going wrong.

## THE THING I MOST WANT VISIBLE: MY OWN COVERAGE ASSERTION ASSERTED NOTHING

The new agreement test asserted that its corpus contained three distinct statement forms before a
tail — the discipline this family adopted after an earlier slice shipped blind to three of four
forest kinds.

**It was vacuous, and only mutation testing showed it.** Removing two of the six continuation
kinds from the descent left the entire suite **green**. Those two corpus cases ended in a data
read, which neither side can type; stopping the descent early lands on a node that is also
untypable, so both readings produced the identical unknown row.

The corpus now ends those cases in a literal and the assertion demands a **typable** tail. All six
continuation kinds fire under mutation, each mutant confirmed to compile before its result was
believed.

I am recording this prominently because it is the sixth-plus instance of one defect — a check
built from the same model as the thing it checks — and this time it appeared **inside the guard
written specifically to prevent that defect**.

## A doc in the same file was claiming a row that was deliberately not emitted

The condition agreement test's heading read "the condition **and branch-pair** rows agree", with a
section describing a branch's statement chain, while the test compares the condition kind alone.
The prose was written while the branch pair was still expected to ship and survived the decision
to withhold it. Corrected in place, with the history left visible.

## A second gap found by asking what else the reference calls a function

**A multiheaded function contributes no tail row at all.** The reference walks each head as its
own function with its own tail, so a three-headed `f` gives three rows; the pipeline reconstructs
the whole group into one fused body and can offer at most one.

I suppressed the group's row rather than emit it. The fused root is a dispatch structure that
typed as unknown on every program I measured — and "unknown on the programs I tried" is not the
property required. If a fused root ever types to a tag, that tag is not one any particular head
promises, and this row feeds an equality predicate. **Emitting nothing costs a check; emitting
the wrong thing costs a valid program.**

The loss is pinned in both directions by `a_multiheaded_function_contributes_no_tail_row`, and
the agreement test's doc says its corpus is single-headed, because a cap that is not written down
reads as coverage.

## One gap named rather than closed

The pipeline's type-name-to-tag table has no `Float` arm where the reference's does. The
direction is the safe one — an unmapped type reports the type channel's unknown, and unknown
accepts — so it costs a check and cannot cause a rejection. Float arithmetic diverges at the
construct-support boundary anyway. It is now named in the tree instead of being left for a reader
to guess about.

## Three questions that remain yours

**One. The floating-point entry ABI**, as above.

**Two. Should a shipped example demonstrate `Byte`?** None of the fifteen does, and it would close
three of the four op tags no corpus reaches.

**Three. Should `01_arithmetic.kel` be enriched?** I corrected its index downward, which is the
conservative direction; enriching the example is the other.

And the two-pass parser work that would make the twelfth stage self-compile remains **yours to
call**. I have not started it.

## What I would take up next

The remaining five kinds. Array elements is the only non-composite one left; the other four are
the branch pair, which is pinned as withheld for a reason that still stands, and the three
composite kinds where the two representations are already known to disagree about what a node is.

