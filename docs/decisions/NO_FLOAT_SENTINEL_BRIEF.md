# Brief — the no-floats sentinel, which this package has never exercised

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Drafted 2026-09-01**, from a hazard a peer's message exposed.

---

## The goal set

| goal | owner | state |
|---|---|---|
| **G7** the no-floats sentinel path | this line | **unblocked, and the subject of this brief** |
| absorption 43 | this line | merged, measuring alone; two predictions recorded in the merge commit |
| the runtime arithmetic width, `f16`, `Text<N>`, `Opaque` | `v0.2.3` / operator | not mine; the peer's next push is the arithmetic width |

## Where this came from, and it is not something I would have looked for

The `v0.2.3` line reported, while preparing their arithmetic-width work, that **`float_bits_log2: 0`
is the NO-FLOATS sentinel**, used by `Target::embedded_8` and `embedded_16` alongside
`has_floats: false`. It is not a one-bit format. Their planned load-time refusal of "any width below
32 bits" would have rejected every module for those targets, and they caught it in preparation.

**Checking my own side against it found two things, one measured and one not.**

**Measured: no test in this package builds a module for a no-floats target.** A search for
`embedded_8` or `embedded_16` across `native_codegen/src` and `native_codegen/tests` returns nothing.
**The sentinel path has no coverage here at all**, and that is true regardless of anything below.

**Not measured, and stated as such:** `float_bytes` is computed as `1 << float_bits_log2 >> 3`, which
for the sentinel gives **zero**. `float_width_lowered` admits only 4 and 8, so float operations
refuse — correct for a module with no floats. But `float_type` is

```rust
match float_bytes { 4 => f32, _ => f64 }
```

**so a zero width silently becomes `f64`**, in the same file whose neighbouring comment says any other
width is *"refused rather than approximated, because a float of the wrong width is a silently wrong
number and not a fault."* **The default arm contradicts the principle stated two functions above it.**

## The order of work, which is the whole of the method here

**Measure first. Decide second.** I believe the default arm is unreachable because every caller sits
behind the whitelist. **That belief is exactly the kind this line has been wrong about repeatedly** —
five recorded cases of a documented or assumed property being false or vacuous when checked, and two
today alone where I inferred a cause from a neighbouring comment and was wrong twice running.

So: **establish what actually happens** when a no-floats module reaches the backend, and only then
decide whether anything needs changing. **Do not harden first and measure afterwards**, because a
change made before the measurement makes the measurement about the change.

## The prediction, with falsifiers

**Predicted: a module compiled for a no-floats target either lowers cleanly or is refused with a
nameable reason. `float_type` is never called with a zero width.**

**Falsifiers:**

1. Lowering a no-floats module panics, or produces a module that fails LLVM verification.
2. `float_type` is reached with a width of zero.
3. A no-floats module lowers and then disagrees with the virtual machine.

**Falsifier 2 is the interesting one**, and it needs an instrument rather than an inspection —
reading the call sites is what I have already done, and it is what I distrust.

## The wrong turns, specifically

**1. Do not turn the default arm into a panic.** This codebase's precedent is the opposite: a
`debug_assert` in `FlatComposite::nested_view` was hardened into **a real fault**, so a release build
never performs out-of-bounds arithmetic on a corrupt offset. **A lowering that cannot proceed returns
`Err`; it does not abort the host.** A panic would be a regression dressed as rigour.

**2. Do not change the whitelist.** `float_width_lowered` admitting only 4 and 8 is correct and is
what makes the sentinel safe. The suspect code is the type function, not the gate.

**3. Do not build a narrow-target test matrix.** The gap is that the sentinel path has *no* witness,
not that it lacks *comprehensive* witnesses. One witness that genuinely exercises it is the
deliverable; a matrix is a different piece of work and would bury the finding.

**4. Do not report "unreachable" without an instrument.** If the conclusion is that the arm cannot be
reached, that conclusion needs something better than my reading of the call sites, because my reading
of neighbouring code has been wrong twice today.

**5. A no-floats target also narrows the WORD.** `embedded_16` sets a 16-bit word. If a module for it
fails to lower for word-width reasons rather than float reasons, **that is a different finding and
must be reported as one** rather than folded into this brief's subject.

**6. Do not edit test sources while a suite runs.** Broken once today, with the rule in the handoff I
validated the same morning.
