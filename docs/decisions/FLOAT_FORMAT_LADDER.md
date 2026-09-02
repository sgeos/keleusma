# The float ladder as it constrains the V0.2.X runtime

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**This is not the record of the ruling.** The ruling is recorded by the V0.3.X line in
`docs/decisions/FLOAT_LADDER.md` on branch `v0.3.0`, commit `92cdaeda`, received directly from
the operator in session on 2026-08-31. Read that first. It carries the ladder itself, the reason
`E5M2` was chosen over `E4M3`, the arithmetic strategy as ruled, the rounding-mode sub-decision,
and the guidance separating `f8` from `Fixed<8>`.

That reference is deliberately plain prose and not a link, because the file does not exist on
this branch yet and a link to it would fail the Markdown link gate.

This file records only what the ladder demands of **this** line's runtime, which the ruling
record does not cover and should not have to.

## A retraction, first

An earlier revision of this file stated that the plan was undocumented and that this was the
first record of it. **That was wrong.** The document existed, first untracked and then committed
as `92cdaeda`, and it was unpushed throughout, so no search of remote branches could have found
it. The search was sound. The conclusion drawn from it was not, because existence is a wider
claim than any search of the remotes can support.

It also stated the width-versus-format ambiguity as an open gap. **The ruling closes it** by
pinning one format per width, which makes `float_bits_log2 = 3` unambiguously `E5M2`, and
reserves discriminator space against a later `E4M3` regardless. Nothing is open there.

## What this line must fix before any new rung is reachable

The ruling record names the arithmetic-width defect as its first precondition and attributes it
to the bundled alias carrying no `#[cfg]`. That is true and it is not the whole mechanism. Two
refinements, both measured on this branch.

**The defect is not confined to `narrow-float-32`.** `check_runtime_widths` in `src/vm.rs`
rejects only bytecode declaring *wider* than the runtime, and admits narrower deliberately. So a
stock build with no features, loading a module that declares a 32-bit float, computes in `f64`.
The feature is how the defect was noticed rather than what causes it, and a fix scoped to the
feature would leave the common case untouched.

**Storage already narrows and arithmetic does not, so the two disagree.** Three sites in
`src/bytecode.rs` write and read a declared four-byte float through `f32`. No site narrows an
arithmetic result. The consequence is that a value rounds when it passes through a composite
field and does not when it stays on the operand stack, so the same expression can yield two
answers depending on whether an intermediate was stored. This is inferred from those sites
rather than witnessed by a test.

## Why widen-compute-narrow is forced here, and not merely preferred

The ruling states widen-calculate-truncate as the preferred strategy where a floating-point unit
exists. On this line it is the **only** available construction below 32 bits, for a reason
specific to the implementation language.

`src/float.rs` reaches a declared width by pairing it with a real Rust type, with `impl Float for
f32` at `BITS_LOG2 = 5` and `impl Float for f64` at 6. **Rust has no `f16` on stable**, confirmed
against rustc 1.98.0, which rejects the type as unstable, and no `f8` of any kind exists or will.
So `GenericVm` cannot be instantiated at either new rung. The trait-per-width model does not
extend, and no amount of preference enters into it.

The shape to follow already exists on the integer axis. `checked_arith_outputs` in `src/vm.rs`
reads `word_bits_log2` from the module header and narrows each result to the declared width at
runtime, on a wider runtime type. The float axis needs the same thing and has half of it.

## One implementation hazard the ruling does not cover

The ruling settles the rounding **mode**, taking each toolchain's round-to-nearest-ties-to-even
default so the reference and the backend agree for free. A separate hazard survives that choice.

**Narrow directly from the wide value to the declared format. Never through an intermediate
rung.** A 64-to-32-to-16 chain rounds twice and can differ from correct single rounding, and it
is the natural way to write the code, since each step is one `as` cast. Agreeing on
round-to-nearest-even does not help if the two sides chain differently.
