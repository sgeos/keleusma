# BRIEF — dispose of the `Op::Len` witness family on measured evidence

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: working brief for the increment. Superseded by
[`OP_LEN_PRODUCER_CENSUS.md`](./OP_LEN_PRODUCER_CENSUS.md) once that records the result.

---

## The situation

Absorption 51 brought the `v0.2.3` line's `Op::Len` root repair. Twelve tests across five
files went red. They are guards firing as designed and they must not be patched green.

## What was measured, before any recommendation was formed

| subject | before | now |
|---|---|---|
| the `if`-source witness | emitted `Op::Len`, refused a bound | **compiles, folds, is bounded, loads, RUNS `Finished(Int(0))`** |
| the equal-arm variant | emitted `Op::Len` | **folds; no opcode** |
| `refused_witness.kel` | witnessed `Len` | **emits no `Len`** |
| corpus refusals | 2 | **1, only `Stream`** |

## THE HANDOFF'S STATED MECHANISM IS WRONG, AND THIS IS THE FIRST THING TO FIX

Both the handoff and `REVERSE_PROMPT.md` say *"`static_for_in_length` gained an `Expr::If`
arm"*. **It did not.** `structural_for_in_length` still matches exactly `ArrayLiteral`,
`Call`, `FieldAccess`, `Ident`, `ArrayIndex`, `Match`, then `_ => None`. There is no
`Expr::If` arm.

What closed the form is `static_for_in_length`'s **fallback to `infer_expr_type` plus
`array_length_of_type`**, which consults the authoritative per-span type table and therefore
answers for expression forms whose structural arms are absent. `OP_LEN_ROOT_REPAIR.md`
already recorded that this fallback closes six forms rather than one.

**A mechanism repeated across three documents without being read is exactly the failure this
line keeps recording.** Correct it wherever it appears.

## The real finding is larger than the repaired form

`src/compiler.rs` contains **no `Op::Len` construction expression at all** — only comments
and two of its own test assertions. Both fall-back sites were deleted, each replaced by a
fold or a compile error. So the question is not "which construct reaches it now"; it is
whether the compiler can emit it **at any construct**, and the answer available from the
source is no.

## Wrong turns, named

1. **Do not restore green by editing assertions.** Every one of these tests names, in its own
   message, what to do when it fires. Follow that text.
2. **Do not write "unreachable".** This tree carries a retraction on exactly that word:
   `Op::IsStruct` was declared producerless and four producers were found within the hour.
   Record **what was searched and by what method**.
3. **Do not settle it with a grep.** Three textual censuses on this line were falsified by
   their own controls. A textual scan is admissible only as a *supporting* leg, and only with
   a must-fire control proving the scan can see a real emission.
4. **Do not delete a witness claim to restore green.** `witness_integrity.rs` says so
   explicitly: amend the claim and re-measure the census.
5. **Do not rebuild twelve tests on one witness.** That coupling is this line's own design
   fault and it has now rotted three times — `Op::Call`, `Op::IsStruct`, `Op::Len`. One
   instrument owns the verdict; everything else depends on the instrument.
6. **Count with `--no-fail-fast`.** A fail-fast run reported one failure when there were
   twelve.
7. **`Op::Len` still exists in the ISA and the machine still refuses it.** Removing an opcode
   is a wire change and the operator's call. Nothing here proposes that.

## What the disposition must preserve

The VM's flat-array refusal arm is still live and still correct — it defends against a
corrupt or hand-built module. That fact survives the loss of a *source-level* witness, and
pinning it through injected bytecode rather than through a compiled program is what decouples
it from the next fold.
