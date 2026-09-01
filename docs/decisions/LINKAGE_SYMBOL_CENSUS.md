# The linkage symbol census — what a host must supply

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Measured 2026-09-01** on `aarch64-apple-darwin`, host default
triple, LLVM 22.1. Brief: [`LINKAGE_SYMBOL_CENSUS_BRIEF.md`](./LINKAGE_SYMBOL_CENSUS_BRIEF.md).
Instrument: `native_codegen/tests/linkage_symbol_census.rs`.

---

## The result

**67 objects emitted from the corpus. 45 distinct undefined symbols.**

| category | count | symbols |
|---|---|---|
| host contract | 43 | `kel_native_*`, one per registered native |
| compiler runtime and C library | **2** | **`__divti3`**, `bzero` |
| unclassified | 0 | — |

**Attribution, because a symbol without the module that needs it is not actionable:**
`__divti3` is required by `opcode_witness.kel` alone; `bzero` by `verify_typed.kel` alone.

`kel_yield` does not appear. No corpus module that lowers uses coroutines, which is consistent with
the `Stream` refusal being one of the two deliberate residual refusals.

## ⚠ THE DEFERRAL IN `FLOAT_LADDER.md` PRECONDITION 3 IS REFUTED

It reads: *"Worth checking when `f16` lands rather than at the link failure."* The reasoning was that
narrow float operations would be the first thing to need a compiler runtime.

**A compiler-runtime dependency already exists at the shipped `f64` rung.** An embedder linking a
Keleusma native object today needs `compiler-rt` or `libgcc`, not merely the natives they registered.
On a bare-metal target that links neither, this is a link failure **now**, and `f16` would only widen
it.

## The construct, isolated — and two wrong answers reached by reading rather than measuring

**`Fixed` division, and nothing else in the sweep.**

| construct | undefined |
|---|---|
| **`Fixed` division** | **`__divti3`** |
| `Word` division by a runtime divisor | none |
| `Word` modulo by a runtime divisor | none |
| `Fixed` multiplication | none |
| `Byte` division | none |
| `Float` division | none |

Fixed division scales the numerator before dividing, which does not fit in 64 bits, and no target has
a 128-bit divide instruction.

> ⚠ **THE BACKEND'S OWN COMMENT BESIDE THE 128-BIT WIDENING SAYS THE DOMAIN EXISTS FOR CHECKED
> ARITHMETIC, AND INFERRING FROM IT GIVES THE WRONG ANSWER TWICE.** A first probe divided a `Word` by
> the literal 3 and found nothing — LLVM strength-reduces a constant divisor. A second used a runtime
> divisor and still found nothing — the target has a 64-bit divide instruction. **Either wrong answer,
> written up, would have told an embedder to avoid the wrong operation.** The cause was found by
> sweeping candidate constructs rather than by a third guess, and the contrasts are now part of the
> test because the claim means nothing without them.

**This matters more than a `Word` division would have.** `FLOAT_LADDER.md` recommends `Fixed` as the
default for control work, and a derate curve, a ratio or a normalisation is a division. **The single
operation that costs a runtime dependency is among the likeliest to appear in the target domain.**

## The prediction, resolved — and it was too weak to be informative

**Recorded before measuring**: *"At `f64` on a host with hardware floating point, category (b) is
expected to be small and may be empty."* **Falsifier named**: any floating-point helper in it.

**Measured: two symbols, neither a floating-point helper. So the named falsifier did not fire and the
prediction stands as written.**

**That is the problem with it.** The result is consistent with the prediction *and* it establishes
exactly what the deferral denied. **A prediction that is equally consistent with the interesting and
the boring outcome did no work.** "May be empty" cannot fail, and naming a floating-point helper as
the falsifier assumed the very framing the census was written to test.

The prediction that would have been worth recording is the one the census actually answers: *does any
compiler-runtime symbol appear at the shipped rung, of any kind?* Recorded here so the next prediction
on this line is checked against **its own question** before it is written down.

## `bzero`, and why it was reclassified rather than left unclassified

The first run reported `bzero` **UNCLASSIFIED**, and that output is quoted here so the reclassification
is auditable. It is now in the toolchain category **because it is a C-library zero fill**, the class
that arm already covers — Darwin's spelling of what other platforms emit as `memset`. It was not moved
to empty the unclassified bucket, and the record shows the before state so a reader can judge that
rather than take it.

## The limit of this measurement

**One target, one machine.** `aarch64-apple-darwin`, hardware floating point, hardware 64-bit divide.
It says nothing about `thumbv8m` or any target without hardware floating point, **which is the case
precondition 3 actually cares about**. A target without hardware float would be expected to add float
helpers on top of what is measured here; that expectation is not a measurement and a narrow-target
census is separate work.

## ⚠ THE INSTRUMENT PASSED ALONE AND FAILED IN THE SUITE

Recorded because it nearly shipped and because the failure mode is general.

The census was verified by running its own binary with `--test-threads=1`: **five tests, all green.**
The full suite then reported **443 passed, 2 failed** against a predicted **445 passed, 0 failed** —
both failures being these tests.

**The defect was mine and not the backend's.** Three tests sweep the corpus, they run **concurrently**
under the ordinary harness, and they shared one scratch directory with colliding object filenames.
Each deletes its objects after reading, so one test removed another's file between the write and the
read and `nm` reported it missing. Each sweep now gets a directory unique to the call.

**`--test-threads=1` hid it completely.** That flag was used to read ordered output, and the cost was
that the verification did not resemble the execution. **A green run of one binary, alone, is not
evidence about that binary in the suite** — and the prediction is what caught it, because a bare
"2 failed" without a recorded expectation invites the reading that something unrelated broke.
