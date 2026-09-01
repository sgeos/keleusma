# The float ladder — `f64`, `f32`, `f16`, `f8`

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: ruled.** Received directly from the operator in session on 2026-08-31 by the V0.3.X line.
Not relayed, not read off another branch.

---

## The ladder

| rung | format | status |
|---|---|---|
| `f64` | IEEE 754 `binary64` | shipped |
| `f32` | IEEE 754 `binary32` | backend lowering built 2026-08-31; see [`F32_LOWERING_BRIEF.md`](./F32_LOWERING_BRIEF.md) |
| `f16` | **IEEE 754 `binary16`** | ruled, not built |
| `f8` | **OCP OFP8 `E5M2`** | ruled, not built |

## Why `E5M2` and not `E4M3`

**One semantic model at every rung.** `E5M2` keeps infinities, NaN and ordinary rounding, so the
ladder has no exceptional bottom. `E4M3` has **no infinities**, which would make division by zero a
per-format saturation rule and turn the last rung into a second semantic model.

**And `E5M2` shares `binary16`'s exponent field.** Both carry five exponent bits, so `f16` to `f8` is
a mantissa narrowing with the exponent untouched and `f8` to `f16` is exact. The ladder is one family
rather than three unrelated formats.

**Infinity earns its place, and this is the operator's stated reason.** A total language wants a
representable over-range outcome. `Fixed` has none: a sentinel steals a code point and is not
ordered, so comparisons stop meaning what they say. In `E5M2` a value beyond 57344 becomes infinity,
which compares, propagates and can be tested.

## What `f8` costs, recorded so nobody is surprised by it

**Three significand bits.** The relative step is **25 per cent at the bottom of each binade** and
about 14 per cent at the top. An earlier note in this line's correspondence said 12.5 per cent; that
is the BEST case, and the figure is corrected here.

**`Fixed<8>` remains the recommended default for control work**, and the two types are complementary
rather than competing. `Fixed<8>` gives uniform, exact resolution of roughly 0.4 per cent over a known
range. `f8` gives thirty orders of magnitude, 25 per cent steps, and an over-range value. **A
derating curve or a trip threshold wants `Fixed`.** Documentation that offers both without saying
which fits what will get `f8` used where it does harm.

## Arithmetic strategy, as ruled

> *"On architectures where an FPU is available, widen-calculate-truncate is preferred strategy.
> Minimizing rounding is ideal, but not worth spending excessive effort on. On architectures without
> an FPU, like the 6502, support just needs to be emulated."*

**Widen, compute in the wider type, narrow on the way back.** No mainstream hardware computes in
fp8, so this is what an accelerator does anyway. On a target with no floating-point unit at all the
whole surface is emulated, and the ruling explicitly accepts that.

### The one sub-decision that still has to be a single answer

**Not whether to minimise rounding, but what "narrow" DOES.** Round-to-nearest-even and
round-toward-zero differ by up to one unit in the last place, which at a 25 per cent step is not a
rounding nicety. The reference and the backend must do the same thing or the differential fails.

**It resolves for free.** LLVM's `fptrunc` and Rust's `as` both round to nearest, ties to even, so
**taking each toolchain's default makes the two sides agree at no cost**, which is exactly the
proportionate answer the ruling asks for. Recorded so it is a decision rather than a coincidence.

## Reserve the encoding space for a second eight-bit format

Pinning exactly one eight-bit format makes `float_bits_log2 = 3` unambiguously `E5M2`, which
dissolves an objection raised earlier: the header encodes a WIDTH and not a FORMAT, so two eight-bit
formats would have been indistinguishable.

**Reserve space for a discriminator anyway.** `E4M3` is what machine-learning silicon emits, so
interoperability may want it later, and reserving costs nothing now while adding it after the number
ships costs a `BYTECODE_VERSION` authorization.

## Preconditions, in order

1. **The runtime's arithmetic width must track its declared width.** **Until that is fixed the
   differential oracle cannot validate any new rung**, at any width. **This precondition is assigned
   to the `v0.2.3` line, and their record of it is now the better one.** See the retraction below.
2. **Each rung costs a full float differential surface.** `f32` surfaced four real defects on this
   line, every one a plausible wrong number rather than a fault. `f8` costs more than `f16` because
   emulation adds the narrowing question above.
3. **Linkage, for the ahead-of-time path.** On a target without hardware support LLVM lowers narrow
   float operations to compiler runtime calls. A linked C host will need those symbols, which is a
   packaging question the JIT path never asks. Worth checking when `f16` lands rather than at the
   link failure.

## A retraction on precondition 1, and where the authoritative statement now lives

**Ownership.** On 2026-09-01 the operator stated to this line that their understanding is that the
V0.2.X line has been assigned the runtime `f64` to `f32` and `f16` support, and `f16` as IEEE
`binary16`. **They framed it as their understanding rather than as a fresh instruction**, so it may
be a recollection of an assignment made elsewhere. Relayed to `keleusma-39` on the same day with that
qualification attached, and recorded here as received rather than as confirmed.

**The mechanism as this file first stated it was too narrow, and the correction is theirs.**
`docs/decisions/FLOAT_FORMAT_LADDER.md` on `origin/v0.2.3` is the authoritative statement of what the
ladder demands of the runtime. That reference is plain prose and not a link because the file does not
exist on this branch and a link would fail the Markdown link gate.

**Two things it establishes that this file had wrong or missing.**

**The defect is not confined to `narrow-float-32`.** `check_runtime_widths` rejects only bytecode
declaring *wider* than the runtime and admits narrower deliberately, so a **stock build with no
features**, loading a module that declares a 32-bit float, computes in `f64` as well. Naming the
bundled alias and scoping the symptom to the feature described how the defect was noticed rather than
what causes it, and a fix scoped to the feature would leave the common case untouched.

**Storage already narrows and arithmetic does not.** Three sites in `src/bytecode.rs` write and read
a declared four-byte float through `f32` while no site narrows an arithmetic result, so the same
expression can yield two answers depending on whether an intermediate passed through a composite
field. **That is the class of divergence this line's oracle exists to catch, and it would present as
a native-versus-virtual-machine mismatch attributable to the lowering**, which is the wrong package to
search. Their record marks it inferred from those sites rather than witnessed by a test. Constructing
that differential is this line's instrument to offer once the arithmetic width is live.

## The double-rounding question is settled, and neither of us was right first

**Resolved with the `v0.2.3` line on 2026-09-01.** Their record required narrowing directly from the
wide value and never through an intermediate rung. This file replied that the standard innocuous
double-rounding condition, an intermediate carrying at least `2p + 2` significand bits for a target
precision `p`, is **met exactly** at binary32 into binary16, so the chain is safe for normals and the
prohibition is over-broad. **They withdrew the prohibition. The replacement is better than either
version**, and it is theirs.

> **Narrow from an intermediate carrying at least `2p + 2` significand bits for the target `p`.**

**Stated as a condition on the intermediate rather than a prohibition on chaining, it forbids the one
case that actually threatens this ladder, and neither line had named it.** Routing 64 to **bfloat16**
to binary16, on hardware that has bf16 and not f16, is a plausible implementation choice on
machine-learning silicon. **bfloat16 carries 8 significand bits against a requirement of 24 and fails
by two thirds.** A prohibition on chaining would have been withdrawn as over-broad before it ever
caught this.

### My underflow caveat is vacuous at the rung I raised it for

I argued the condition assumes no underflow and that binary16's narrow exponent range is where it
would lapse. **They showed it does not lapse at 64 to 32 to 16**: binary32's exponent range covers
every binary16-representable magnitude, subnormals included, as a **normal** with the full 24 bits, so
the theorem's hypothesis holds across the whole target range. I cannot construct a witness either.

**The rung where the ranges do touch is 16 to 8**, and for the reason this file gives for choosing
`E5M2`. The shared five-bit exponent that makes the ladder one family also puts the two subnormal
ranges in contact, since `E5M2`'s subnormals at `2^-16` to `2^-14` sit inside binary16's. They worked
it and it holds, with the margin shrinking from three to roughly two.

**Recorded position, weaker than either line started with**: the caveat is real in form and neither
line can construct a witness for it on this ladder. Held deliberately in preference to a tidier claim.

### The larger hazard is not double rounding at all

**Their observation, and it is the one worth spending test effort on.** The likely defect is not
rounding twice. It is rounding **once, at the end of a computation, instead of after every
operation**. That is missing rounding, and it is what the tree does today, with three storage sites
narrowing and no arithmetic site narrowing. **At `E5M2`'s three significand bits, round-at-the-end
against round-per-operation is a different function rather than a precision detail.**

## Why the two implementations may be compared at all

**Established with the `v0.2.3` line on 2026-09-01, and it is the justification for this line's
differential being a proof rather than a coincidence.**

The two sides do not compute the same way. This backend lowers float arithmetic **natively at the
declared width**, because LLVM has the type. The reference **widens to `f64`, computes, and narrows**.

They agree by the same condition. **`f64` carries 53 significand bits against a requirement of
`2 x 24 + 2 = 50` for an `f32` target, so computing in `f64` and rounding once per operation to `f32`
is equivalent to native `f32` arithmetic.** Margin 3. It holds a fortiori at `f16`, which needs 24,
and at `E5M2`, which needs 8. **Widen-compute-narrow from `f64` is therefore sound at every rung on
this ladder.**

**Two limits, on the record.** The equivalence covers **five operations** — addition, subtraction,
multiplication, division and square root. It does **not** cover a fused multiply-add, whose purpose is
to round once where the theorem assumes twice, and it does not cover transcendentals. And it assumes
the intermediate neither overflows nor underflows, which `f64` makes remote but not impossible.

## The five-operation precondition is now pinned, and the obvious instrument for it reads green

`native_codegen/tests/float_no_contraction.rs`. **The backend sets no fast-math flags**, so nothing
downstream is licensed to fuse, and the equivalence above applies to what is emitted.

> ⚠ **AN IR-LEVEL SEARCH FOR CONTRACTION IS A FALSE NEGATIVE.** The first version of that file looked
> for an FMA intrinsic and for fast-math flags in the emitted LLVM IR after a full `default<O2>`
> pipeline. **It passed on the first run and it was measuring nothing.** Fusion is not an IR
> transform. Granting `contract` to the `fmul` and the `fadd` and changing nothing else leaves both
> instructions flagged and **still separate** after O2. The fusion happens in code generation, and the
> **machine instruction is the first place it is visible**. Only after emitting assembly did the
> mutation produce an `fmadd` while the unmutated backend produced none.

**The mutation is what exposed it**, and it perturbs the lowering rather than the probe. Without it,
"no FMA found" would have been indistinguishable from "this shape does not fuse here" and from "the
instrument cannot see fusion" — and the third was the true one.

**Two defects in the guard's own machinery, both caught before it landed.** The mutation first
replaced `"fmul "`, which also matches the `%fmul` on the left of the assignment, because the backend
names its values after the mnemonic. And the flag detector sliced a span from the first occurrence of
the mnemonic and bounded it on `" float"`, when the default float type prints as `double`. It returned
the right answer for the wrong reason, which is the least useful way to be wrong.

**Handed back to the `v0.2.3` line**, and answered by them the same day. Their construction rounds per
operation only if the Rust compiler does not itself contract a multiply feeding an add inside the
arithmetic path. **Measured from generated code**, at `-O` and at `-C opt-level=3 -C
target-cpu=native` on `aarch64-apple-darwin`: **zero fused instructions** for a plain `a * b + c` at
either width, and **one** for an explicit `mul_add` as a must-fire control.

**The target makes the null result mean something.** On aarch64 the fused multiply-add is **baseline
rather than a target feature**, so the compiler had the instruction throughout and the control proves
it emitted one in the same compilation unit. It cannot be that the feature was unavailable, which is
the ambiguity the same measurement would carry on x86. **Their stated limit: one target measured.**
Rust's freedom from fast-math is a language-level property, so it is expected to generalise, and
expectation is not measurement.

They also verified something neither line had checked: `(a * b) as f32 as f64` emits **two `fcvt`
instructions**, narrow then widen, neither elided. **That single operation is what their whole
construction rests on**, and had the round trip been treated as a no-op the per-operation rounding
would have vanished silently while every test at equal widths stayed green.

**Pinned portably on this line** in `float_no_contraction.rs`, because assembly inspection establishes
a fact and is a poor instrument for keeping it true. Two numeric witnesses assert that a two-step
multiply-then-add differs from an explicit fused one, which fails the moment a toolchain starts
contracting. **Both were re-derived here rather than taken on report**, and the second is **2 ulps**
apart where their message said one. **The operands must pass through a black box**, their caution: the
compiler otherwise constant-folds the expression and the test pins its constant evaluator instead of
its code generation, and since constant folding does not contract either, **it would still pass while
testing nothing**.

## A prediction recorded before the format-fingerprint absorption

**The `v0.2.3` line gave advance notice on 2026-09-01** that `origin/v0.2.3` will shortly carry a
format fingerprint in the auxiliary header's reserved word, refused at load on mismatch. It exists
because `BYTECODE_VERSION` is frozen at 2 across releases, so the version check admits every release
declaring 2.

> ⚠ **THEY RETRACTED THE MECHANISM AFTER THE OPERATOR REDIRECTED IT, AND THIS RECORD IS CORRECTED
> RATHER THAN LEFT.** It is **not** derived from the scalar size table. It is a **random value rolled
> once per release**, held in a constant beside `BYTECODE_VERSION`, with a script to read the working
> tree's value, read any commit's or tag's, and roll a new one. **A derived value only ever covers
> what it hashes**, so a release that changed an opcode's meaning or the reading of an existing field
> would leave it unmoved while genuinely differing. Per-release covers the release rather than a proxy
> for it.

**Prediction: zero movement in the `native_codegen` figures.** The backend consumes a module through
the runtime's own reader and derives everything from opcodes and the auxiliary tables rather than from
the header's reserved word, and both sides of every differential are produced by the same reference
compiler, so a changed header reaches them identically. `corpus_fingerprint.rs` pins the CONTENT of the
corpus source roots and states in its own header that it does not cover the reference compiler's
behaviour, so it should not fire.

**Falsifier, named in advance**: any test on this line that pins an encoded module length or hashes
emitted bytes. **Swept, and none found.** The only fingerprint on this line is over corpus SOURCE
files; the single test that reads emitted bytes is `aot_linkage.rs`, which searches an object file for
a symbol needle and does not read the Keleusma module encoding at all. **A named falsifier nobody
looked for is a weaker claim than one that has been looked for**, so the sweep is part of the
prediction rather than a follow-up. **If the absorption moves a figure anyway, the movement is
reported and the pin named rather than the figure adjusted.**

**The prediction is unaffected by the redirect**, because the mechanism is one changed word in the
header either way. Only the description of what moves the word changes.

> ⚠ **A CONSEQUENCE THEY TOLD THIS LINE AND THEN WITHDREW.** That `Text<N>` collapsing
> `ScalarKind::Text` from two words to one address would move the fingerprint, and that this would be
> the detector's first evidence against a real change. **False under the redirected design.** The
> fingerprint moves at RELEASE, and two builds within one release cycle share it deliberately. What
> catches an unintended layout change is the golden wire-byte test, which fired on the fingerprint's
> own arrival twice at exactly eight changed bytes.
