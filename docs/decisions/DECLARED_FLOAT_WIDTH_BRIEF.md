# Brief — a hard-coded float width the absorption forced me to look at

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Drafted 2026-09-02.**

---

## The goal set

| goal | state |
|---|---|
| **G18** the hard-coded float width in `width_of_declared_shape` | **unblocked, and the subject of this brief** |
| `f16` | no oracle — the reference refuses widths 3 and 4 at load |
| `Text<N>` | the other line's, one increment landed |
| publication | held |

The gate is green on this branch and absorption 46 is measured, so both of those are closed rather
than pending.

## Where this came from

Absorption 46 changed `ScalarKind::size_in_bytes` to take an address width, which broke the build at
`width_of_declared_shape`. Repairing it put the line in front of me:

```rust
Some(k) => Width::Scalar(u32::try_from(k.size_in_bytes(8, 8, 8)).unwrap_or(0)),
```

**All three widths are hard-coded.** The address one is defensible — `Opaque` is refused at every
reachable route. **The float one contradicts a comment two functions above it**, which says:

> *"A `Float` and a `Word` are both eight bytes, so a width alone cannot tell them apart."*

**That is false under `narrow-float-32`, where a `Float` is four bytes.** So a declared `Float` shape
is reported as eight when the module says four.

**It was recorded rather than repaired at the time, deliberately**, because folding a behaviour change
into a build repair would have made the absorption's zero-movement result unfalsifiable — I could not
have said whether the zero came from the prediction holding or from two changes cancelling. That
reason is the `v0.2.3` line's and it is better than the one I gave.

## Measure before deciding, which is the method that has worked

**The `narrow-float-32` suite is green at 459/0/88**, so either the path is unreachable for a declared
`Float`, or the width is not consumed in a way that differs. **Which of those is true changes what
should be done**, and I do not know it yet.

**Predicted: it is reachable but unexercised** — no corpus module and no test declares a `Float` in a
chunk signature or native return shape and then has that operand's width consumed, so the wrong width
is computed and never used.

**Falsifiers, either of which changes the answer:**

1. A probe declaring a `Float` return under `narrow-float-32` produces a **wrong value**, not merely a
   wrong width. Then it is a live miscompile, not a latent one.
2. The path cannot be reached at all — the front end or an earlier guard refuses every route. Then the
   hard-coding is safe and the comment is what needs correcting.

## The wrong turns

**1. Do not fix it before measuring.** The whole reason it was deferred is that a change made before
the measurement makes the measurement about the change. That applied to the build repair and applies
again here.

**2. Do not thread a width just because it is in scope.** `float_bytes` is available at both call
sites, so the fix is easy — which is exactly why it should be justified by a measurement rather than
by ease.

**3. A green `narrow-float-32` suite is not evidence of correctness here.** It is evidence that
nothing currently exercises the path. Those differ, and conflating them is what let five refusal
messages name the wrong supported set for a week.

**4. If it is unreachable, correct the COMMENT.** *"A `Float` and a `Word` are both eight bytes"* is
false in a supported configuration whatever happens to the code, and a false statement in a comment
that explains why a type exists is worse than a hard-coded constant.

**5. Report a value difference, not a width difference.** A wrong width that produces a right answer is
a latent defect; a wrong answer is a live one. Say which.
