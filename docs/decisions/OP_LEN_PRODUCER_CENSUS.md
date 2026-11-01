# `Op::Len` has no producer that could be found, and the family that witnessed it is disposed of

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: measured 2026-09-05 on the `v0.3.0` line, after absorption 51.
**Verdict**: no producer found, by four methods, with the limits stated below.
**Not** a claim of unreachability. See [Why that word is not used](#the-word-that-is-not-used).

---

## What happened

Absorption 51 brought the `v0.2.3` line's `Op::Len` root repair. **Twelve tests across five
files went red at once**, every one of them this line's. They are guards firing as designed,
and each named in its own failure message what to do when it fired.

## The mechanism, and this line recorded it wrongly first

Both [`HANDOFF`](../process/handoffs/v0.3.0.md) and [`REVERSE_PROMPT.md`](../process/REVERSE_PROMPT.md)
stated that *"`static_for_in_length` gained an `Expr::If` arm"*. **It did not.**
`structural_for_in_length` still matches exactly `ArrayLiteral`, `Call`, `FieldAccess`,
`Ident`, `ArrayIndex` and `Match`, then `_ => None`.

What folds the form is `static_for_in_length`'s **fallback to `infer_expr_type` plus
`array_length_of_type`**, which consults the authoritative per-span type table recorded by the
post-monomorphization type-check pass, and therefore answers for expression forms whose
structural arms are absent.

**[`OP_LEN_ROOT_REPAIR.md`](./OP_LEN_ROOT_REPAIR.md) stated this correctly on the day it
landed**, under a heading admitting the prediction had been wrong. This line restated it
incorrectly from a document it had already absorbed, without reading it. That is the
relay-without-provenance failure in a fresh costume, and it propagated into three artefacts
before anyone read the source.

## And the finding is larger than the repaired form

**Both `Op::Len` emission sites in `src/compiler.rs` are gone.** Each folds the length from the
operand's type or fails with a compile error naming the unfoldable length. So the question is
not which construct reaches the opcode; it is whether any can.

## The instrument: four legs, each with a control

[`native_codegen/tests/len_producer_census.rs`](../../native_codegen/tests/len_producer_census.rs).

| leg | establishes | cannot establish | result |
|---|---|---|---|
| detector control | the detector reports TRUE on a module that really carries the opcode | anything about the compiler | passes |
| construct battery | these source shapes do not emit it | that no other shape does | 14 probed, 10 reach codegen, **0 emit** |
| corpus sweep | no program anyone has written emits it | anything about unwritten programs | **69 modules across four roots, 0 carry it** |
| source scan | the compiler holds no construction expression today | that construction cannot arrive indirectly | **11 occurrences, all comments or absence assertions** |

**A grep was not accepted as the answer.** This line ran three textual censuses of the
backend's refusal surface and had all three falsified by their own controls. A scan counts
mentions; it cannot tell a construction from a match arm from a sentence. It is admitted here
only as a supporting leg, and only carrying a control — the same scanner over
`src/wire_format.rs`, which really does construct the opcode in its decoder, and finds it.

## The word that is not used

**Unreachable.** This tree carries a retraction on exactly that word: `Op::IsStruct` was
declared producerless and four producers were found within the hour. What is recorded is what
was searched and by what method.

## Disposition of the twelve failures

| file | was | now |
|---|---|---|
| `len_flat_array_hazard.rs` (4) | the hazard is latent and liftable | **inverted**: the fold is asserted; the runtime arm is pinned through injected bytecode; the folded program's VALUE is asserted |
| `probe_len_reachability.rs` (5) | five assertions about one witness | **four retired as superseded**; the chain is asserted once; a resolved prediction is recorded |
| `miscompilation_reach.rs` (1) | not reached because the bound is refused | **restated**: not reached because nothing produces it |
| `remaining_refusals.rs` (1) | 2 refusals | **1**, with the departing chunk named |
| `witness_integrity.rs` (1) | `refused_witness.kel` claims `Len` | **claim amended** and the census re-measured |

**Nothing was patched green.** Every inversion asserts the new fact in the direction that makes
a regression fail.

## Two consequences worth keeping

**The corpus witness file left the exempt set, and the predicted panic did not occur.**
`probe_len_reachability.rs` predicted that if every refused opcode in `refused_witness.kel`
became lowerable while the module still could not be given an arena, the corpus differential
would panic in `arena_for`'s `expect("arena capacity")`. Measured: `module_refusals` returns
empty, the module takes an arena of 600 bytes, loads, and runs to `Int(4)`.

**The prediction was sound and its premise was what failed.** It assumed the two properties
could move apart. They could not — the property that emitted `Op::Len` was the property that
denied the bound, which is the structural claim the file argued from the start. The argument
held; the contingency planned around it never arose.

**A refusal leaving the list because its input vanished is not backend coverage.** The backend
did not learn to lower `Op::Len`; it still refuses it, and
[`isa_lowering_census.rs`](../../native_codegen/tests/isa_lowering_census.rs) still records that
refusal with its reason. Conflating the two would overstate coverage. `65 of 66` remains the
honest figure, for a new reason: the last opcode is unobtainable because nothing emits it, not
because what emits it cannot be admitted.

## Three failures of my own surfaced while doing this, and they are recorded here

**1. The mechanism was restated from an absorbed document without reading it.** Covered above. The
cost was three artefacts carrying a false claim.

**2. A measurement was taken while the tree moved under it.** Documents were edited during a running
suite — the absorption-40 hazard, which this line had already recorded twice. That run is discarded
rather than quoted, and the reported figures come from a frozen tree.

**3. THE SWEEP'S CORPUS LOADER WATCHED THREE ROOTS WHERE THE CENSUSES READ FOUR.** It was copied from
`remaining_refusals`, whose three-root population is correct *for that test*.
[`corpus_fingerprint.rs`](../../native_codegen/tests/corpus_fingerprint.rs) watches four, because
`spike_corpus_coverage`, `isa_lowering_census` and `bound_transfer` read `examples/rtos/scripts` and
`compiler/kel` as well.

**That file's own header records this defect at three granularities**, and this is the fourth
occurrence — committed by a loader written in the same session that read the warning. A sweep looking
for an opcode wants the widest population available, since a negative over a narrow corpus is a weaker
negative. Widened to four roots: **69 modules, not 67.** The tree already recorded both figures side
by side; the loader simply used the wrong one.

## Two process lessons with a cost attached

**A pin must be the last edit to what it pins.** `corpus_fingerprint` fired, was updated, and fired
again — because the witness file was edited once more afterwards. Updating a digest early guarantees
re-work.

**A uniqueness token is only unique within the scope that mints it.** `linkage_symbol_census` failed
on *different tests* in two consecutive runs of a frozen tree. Its scratch-directory helper
disambiguated with a process-local counter, which separates tests under `cargo test` but not under
`cargo nextest`, where each test is its own process and every counter starts at zero. **"The test
harness" is not one scope.** Repaired with the process id, and the reasoning kept beside the helper.

## The design fault, which is this line's own

**Twelve tests, one witness.** Every one keyed to the same construct, so a single upstream
improvement invalidated the family at once. That is the coupling that rotted the `Op::Call` and
`Op::IsStruct` versions before it, rebuilt at larger scale.

**The repair is structural, not a resolution to be careful.** The witness text now has a single
definition in `native_codegen/tests/common/mod.rs`, and the reachability verdict has a single
owner in `len_producer_census.rs`. Tests may still be numerous; the witness definition may not
be duplicated across them.

## Residual concerns, stated rather than closed

- **The source scan is textual and is the weakest leg.** It is written as a ratchet on a
  known-inert set, not as a verdict. A construction arriving through a helper in another module
  would not be seen by it — the corpus sweep and the battery are what cover that, imperfectly.
- **The battery is shapes this line thought of.** Fourteen is not the `Expr` enum, which has 27
  variants.
- **`refused_witness.kel` is now misnamed**: nothing in it is refused. Recorded rather than
  renamed, because several tests and documents reference it by path.
- **`Op::Len` remains in the instruction set.** Removing an opcode is a wire change and the
  operator's call. Nothing here proposes it, and the machine's refusal is still needed for
  decoded or hand-built modules.
