# The linkage census on a bare-metal target

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Measured 2026-09-01**, LLVM 22.1, host `aarch64-apple-darwin`
against `thumbv8m.main-none-eabihf` — the target `examples/rtos/` actually builds for.
Brief: [`NARROW_TARGET_LINKAGE_BRIEF.md`](./NARROW_TARGET_LINKAGE_BRIEF.md).
Host result: [`LINKAGE_SYMBOL_CENSUS.md`](./LINKAGE_SYMBOL_CENSUS.md).

---

## The result, and the part neither the prediction nor its falsifiers anticipated

**67 objects on each target. The two toolchain requirements are DISJOINT.**

| | count | symbols |
|---|---|---|
| host only | 2 | `__divti3`, `bzero` |
| **shared** | **0** | — |
| narrow only | 11 | `__adddf3`, `__aeabi_unwind_cpp_pr0`, `__divdi3`, `__fixdfdi`, `__floatdidf`, `__gedf2`, `__gtdf2`, `__moddi3`, `__udivdi3`, `__unorddf2`, `memset` |

**The host census is not a lower bound on the embedded one.** Neither set contains the other. That is
a stronger and more useful statement than "the host is unrepresentative", and it means a host
measurement cannot be used to reason about an embedded link at all — not even conservatively.

## Attribution by construct

| construct | `aarch64-apple-darwin` | `thumbv8m.main-none-eabihf` |
|---|---|---|
| `Word` division | clean | **`__divdi3`** |
| `Word` multiplication | clean | clean |
| **`Float` addition** | clean | **`__adddf3`, `__fixdfdi`, `__floatdidf`, `__gedf2`, `__gtdf2`, `__unorddf2`** |
| `Float` comparison against a constant | clean | clean |
| **`Fixed` division** | **`__divti3`** | **clean** |
| `Fixed` multiplication | clean | clean |

**`Fixed` division inverts, and it was checked rather than assumed.** On the 32-bit target the emitted
object has **zero undefined symbols and zero branch-and-link sites**, so the backend expands the
128-bit division **inline** instead of calling a helper. That is the whole of why the two sets are
disjoint rather than nested.

`Float` comparison against a constant is clean on both because LLVM rewrites `(double)w > 1.5` into an
integer comparison. **It is not evidence that float comparison is free**; the corpus sweep finds
`__gedf2`, `__gtdf2` and `__unorddf2` in the narrow set, reached through the saturating conversion.

## The prediction: confirmed by count, refuted as a containment claim

**Recorded before measuring**: the narrow target requires *strictly more* runtime symbols than the
host, dominated by 64-bit integer and double-precision floating-point helpers. Three falsifiers.

| falsifier | fired? |
|---|---|
| the narrow set is a subset of the host set, or equal | **no** |
| `Word` division is still clean on the narrow target | **no** — `__divdi3` |
| float arithmetic is clean on the narrow target | **no** — six symbols |

**No falsifier fired, and the prediction is still partly wrong.** "Strictly more" is true by count,
11 against 2, and **false as containment**: the narrow set does not include `__divti3` or `bzero`.
**The prediction was ambiguous between a count and a superset and I did not notice when writing it.**
The falsifiers only tested the containment reading in one direction, so a disjoint result passed all
three while contradicting the natural reading of the claim.

The "dominated by" half is confirmed exactly: three 64-bit integer helpers and six double-precision
ones out of eleven.

## What this means for the embedded example

`examples/rtos/` links a bare-metal binary. **Eleven symbols must come from somewhere**, and only one
of them, `memset`, is a C-library name an embedded project routinely provides. The other ten are
compiler-runtime, and a `no_std` Rust build normally gets them from `compiler_builtins` — which the
example links for its own Rust code and which therefore probably covers the Keleusma object too.
**That is an inference and it is not measured here.** What is measured is that the object needs them.

**`__aeabi_unwind_cpp_pr0` is the one to look at first.** An unwinding personality routine is an
unexpected requirement for a language with no exceptions, on a target with no unwinder. Its origin is
**not determined here**, and it is recorded as an open question rather than explained.

## What this means for the float ladder

**A single `f64` addition, with its conversions, pulls six runtime symbols on the flagship embedded
target.** Every double-precision operation there is already a function call.

> ⚠ **THIS CLAIM WAS ASSERTED HERE AND WAS FALSE AS WRITTEN. MEASURED 2026-09-01, LATER THE SAME DAY.**
>
> It read: *"So `f32` and `f16` buy native instructions rather than merely narrower storage."* **That
> was an inference from the `f64` figure and from the target having a floating-point unit. The `f32`
> cost was never measured.**
>
> | CPU | `f32` | `f64` |
> |---|---|---|
> | **`generic`, no features** — what this census used | **6** | **6** |
> | `cortex-m33` | **2** | 6 |
>
> **With the CPU this census actually used there is NO GAIN AT ALL** — six symbols either way, just
> the single-precision routines instead of the double-precision ones.
>
> **The claim holds only with the unit enabled, and only partly.** At `cortex-m33` the add and the
> three comparisons go native, and **the residual two are `__fixsfdi` and `__floatdisf` — the
> conversions between a 64-bit integer and a float.** Those cannot go native because **Keleusma's
> `Word` is 64 bits and the unit is single-precision.**
>
> **The corrected statement**: on a target whose floating-point unit is enabled, `f32` moves the
> arithmetic and comparisons into instructions and leaves the `Word` conversions as calls. **The
> residual is a property of the WORD width, not the float width** — which points at the narrow-word
> work rather than at the float ladder.
>
> **And nothing here is measurable about `f16`.** Nothing implements it and the reference refuses the
> width at load, so there is no oracle. The original sentence covered both rungs; only one can be
> measured, and the measured one must not carry the other.
>
> Pinned by `the_narrow_float_width_costs_fewer_runtime_symbols_only_with_the_unit_enabled`, which
> asserts both rows — the no-gain case and the corrected one — so neither can quietly change.

It also bears on **worst-case execution time**, which is this project's value proposition. An inline
expansion is analysable; an opaque call into a compiler runtime is not, unless the runtime's own
bound is established. **No instruction counts were measured here**, so that is a direction rather
than a finding.

## Two instrument defects, both of which produced a confident wrong answer

**1. `nm -u` does not print the same shape on both formats.** Mach-O gives a bare name; ELF gives
`U <name>`. Taking the line verbatim put **every** ELF symbol into the unclassified bucket, because
`"U __divdi3"` begins with neither `kel_native_` nor `__`.

**2. Round-tripping the host triple through a `String` emitted ZERO host objects, silently.** The
comparison then ran an empty set against a full one and **reported the prediction refuted**. It was
reported refuted by a broken instrument while the underlying data supported it.

**Both were caught because the comparison prints its inputs**, so a host count of 0 beside a narrow
count of 67 was visible on the same line as the verdict. **A test that had only asserted would have
reported "prediction refuted" and been believed.**

## Limits

One machine, one LLVM, two targets, `generic` CPU on each, `RelocMode::PIC`, `default<O2>`.

> ⚠ **THE CPU SELECTION IS NOT A FOOTNOTE, AND THIS SECTION TREATED IT AS ONE.** It read that a
> different CPU "could change which operations are native". **Measured: it changes the `f32` figure
> from six symbols to two**, and it is the difference between the claim above being false and being
> true. **Every count in this document is under `generic` with no features**, which is a CPU with no
> floating-point unit — so the eleven-symbol figure describes the worst case rather than a realistic
> deployment.

No other bare-metal target was measured.
