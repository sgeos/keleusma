# `FloatToInt` was poison, and agreed only by hardware accident

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: fixed.** The conversion now uses `llvm.fptosi.sat`, which is defined on every target.

## The defect, found while scoping a different slice

The reference converts a float to a word with Rust's `as`, which **saturates**:

| input | reference |
|---|---|
| NaN | **0** |
| `+inf`, or too large | **`i64::MAX`** |
| `-inf`, or too small | **`i64::MIN`** |

**LLVM's plain `fptosi` is POISON for exactly those inputs.** The lowering I wrote in float slice one
used it, so the two agreed only by whatever the target happened to do.

## The measurement that makes this precise rather than theoretical

**They DO agree on this machine**, and that is the problem rather than the reassurance. aarch64's
`fcvtzs` saturates, which happens to match the reference. Verified at runtime rather than with a
constant — a constant is folded at compile time and never reaches the target's instruction.

**On x86-64 it would disagree.** `cvttsd2si` returns the integer-indefinite value for *every*
out-of-range input, so `+inf` would give `MIN` where the reference gives `MAX`, and NaN would give
`MIN` where the reference gives `0`. LLVM is entitled to either, because the value is poison.

**And it is reachable today**, not merely latent: a runtime out-of-range multiply produces one, and
float multiplication landed in slice one. Division would widen the reach further.

## The fix

`llvm.fptosi.sat.i64.f64` is **defined** to saturate on every target, and is the intrinsic Rust itself
lowers `as` to. The match is now by construction rather than by accident, and the emitted IR contains
no poison for these inputs.

**The pinned test passes both before and after on this machine.** That is stated plainly: what it
guards is that the agreement survives when the hardware accident does not.

## How this was found, which is the transferable part

It came out of **scoping float division**, not out of auditing what existed. Division produces `inf`
and `NaN`, so the question "what does the reference do with those?" had to be answered — and answering
it exposed a defect in already-shipped code, one increment old.

**This is the third time in this backend that implementing a feature removed an accidental
protection**, and the second where the protection was never recognised as accidental until the
neighbouring feature was scoped. The pattern is worth naming: *a lowering that is correct only because
its bad inputs are unreachable is a lowering waiting for the next feature.*

## Division remains unimplemented, and now for a stated reason

`Op::CheckedDiv` pushes **three** values, and `push_triple` **traps when the flag is non-zero** under
the trapping overflow policy. For floats, flags 1, 2 and 4 mean `+inf`, `-inf` and NaN — **legitimate
results**, since float division is total on the reference with no zero trap. Routing floats through
the existing triple would turn valid values into faults.

So float division needs a non-trapping triple push with per-slot kinds, which is a slice of its own
rather than a few lines.
