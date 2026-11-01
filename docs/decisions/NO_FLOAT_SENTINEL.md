# The no-floats sentinel, and five refusals that named the wrong supported set

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Measured 2026-09-01.**
Brief: [`NO_FLOAT_SENTINEL_BRIEF.md`](./NO_FLOAT_SENTINEL_BRIEF.md).

---

## Where it came from

The `v0.2.3` line reported, while preparing unrelated work, that **`float_bits_log2 == 0` is the
no-floats sentinel** used by `Target::embedded_8` and `embedded_16`, not a one-bit format. Their
planned refusal of "any width below 32 bits" would have rejected every module for those targets.

**Checking this package against it found that no test here built a module for a no-floats target at
all.** The path was entirely uncovered.

## What was measured, before anything was changed

| target | float width | outcome |
|---|---|---|
| host | 8 | lowers, verifies |
| `embedded_16` | 0 | **refused for WORD width (4), never reaching the float question** |
| `embedded_8` | 0 | **refused for WORD width (3)**, likewise |
| no floats, 64-bit word | 0 | **lowers and verifies** |
| floats enabled, 16-bit float | 2 | **refused, and no IR emitted at all** |

**A float operation cannot exist under a no-floats target**: the front end refuses it with *"target
does not support floating-point types"*. That is why the zero width is harmless — not an argument,
a measurement.

**The embedded-target refusal is a distinct finding and is kept separate.** Folding it into the float
result would have credited the float handling with a refusal it did not make.

## ⚠ THE PREDICTION FAILED ON THE FALSIFIER THE BRIEF CALLED INTERESTING

**Predicted**: *"`float_type` is never called with a zero width."* **False.** `lower_module` computes
the entry ABI's float type unconditionally, so it is called with zero for every no-floats module.

The result is provably unused — it is consumed only where a signature shape is a float, and such a
module has none. **But the prediction as written is refuted, and the brief said in advance that the
falsifier needed an instrument rather than a reading of the call sites.** The reading had said
unreachable. **The instrument disagreed with the reading, which is the whole reason the brief demanded
one.**

## ⚠ THE DECISION BELOW WAS REVERSED THE SAME DAY, AND THE REVERSAL IS RECORDED RATHER THAN SUBSTITUTED

**What follows was this line's conclusion, and it was changed by an argument from the `v0.2.3` line.**
It is kept because a decision that quietly becomes its opposite teaches nothing, and because the
argument that moved it is worth more than either position.

**Their argument, which is reciprocal rather than abstract:** both storage sites in their
`src/bytecode.rs` use `_ => return Err(UnsupportedWidth)` **at the same junction.** So the convention
being contradicted locally is one this codebase already follows elsewhere — a far stronger objection
than the doc comment it was being weighed against.

**And their design showed the way past the dilemma below.** They could not return a `Result` from
`narrow_float` without threading errors through ten hot-path sites, so **they moved the refusal to
load and left the hot path total.** The analogue here turned out smaller than assumed, and the fix is
recorded in [`REACHABILITY_AND_RESUME_BRIEF.md`](./REACHABILITY_AND_RESUME_BRIEF.md): hoist the
entry-ABI refusal above the declarations, and build the module-level float type only inside the
branches that hold a float shape.

**Measured after the change**: replacing the default arm with a panic and running the whole package
gives **454 passed, 0 failed over 87 binaries with zero panics**. The arm is unreachable **by
execution**, not by a reading of six call sites — and the reading had already been wrong once the same
day, which is why the mutation was run at all.

**The panic was reverted.** A lowering that cannot proceed is a fault in this codebase and not an
abort of the host, on the same ground that `FlatComposite::nested_view` was hardened from a
`debug_assert` into a real error.

## THE SUPERSEDED DECISION, kept for its reasoning: change nothing in the whitelist

`float_type`'s default arm widens any unrecognised width to `f64`, which looks like the
silently-wrong-number hazard the same file warns against two functions above it.

**Measured, it is not reachable through any operation.** Every float route checks
`float_width_lowered` first, and an unlowerable width is refused with no code emitted. Threading an
`Option` through six call sites and into closures, to remove a hazard nothing can reach, **would risk
a real defect in a backend measured correct**. The brief also ruled out a panic: this codebase's
precedent is a fault, not an abort.

## The finding that was not the subject: five refusals named the wrong supported set

The `f32` increment taught the backend to lower a four-byte float **and updated none of the refusal
messages**. Five places told a reader that only an eight-byte float is lowered:

- the shared-slot ABI refusal, and the comment above it saying *"the eight-byte case is lowered above"*
- the entry-ABI refusal — **in a block whose own comment says `Float` is `f32` under
  `narrow-float-32`**, so the message contradicted the comment beside it
- the `GetIndex` float-element refusal
- the `GetField` float-field refusal
- a doc comment reading *"Only 8 is lowered today"*

**A refusal that names the wrong supported set is worse than a bare refusal**: it does not merely
fail to help, it tells someone to stop using something that works. All five are corrected.

**Pinned as a class rather than as instances.** `no_refusal_claims_eight_is_the_only_lowered_float_width`
scans the backend source, because pinning five strings would be brittle and would miss the sixth. It
guards its own premise — if `float_width_lowered` stops admitting four, the test fails loudly rather
than quietly enforcing a stale rule of its own — and it asserts the corrected phrasing is present, so
an absence claim is not satisfied by a reader that sees nothing.

**Two of the five were found only because a test demanded the corrected wording.** A first pass
replaced four occurrences and reported success; the fifth wrapped differently across lines and
survived, and the assertion on the message text is what caught it.
