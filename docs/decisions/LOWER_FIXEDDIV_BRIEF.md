# BRIEF — lower `FixedDiv`, whose blocker stopped being true

**Written**: 2026-08-27, thirteenth loop iteration. **For this line's own use.**

## Why this, and why now

The arena thread is closed. After several measurement-only increments the roadmap-aligned work is
**capability**: the backend lowers **60 of 66** opcodes, with two named refusals, `FixedDiv` and
`Len`.

**`FixedDiv`'s recorded reason is stale.** It reads:

> *"Reproducing that natively needs the runtime-fault lowering, which `RUNTIME_FAULTS.md` defers to
> V0.4.0 — and the existing trap branch here is gated on `OverflowPolicy::Trap`, so it is a policy,
> not the unconditional fault the VM raises."*

**`Op::Div | Op::Mod` already emits exactly that unconditional fault.** It compares the divisor to
zero and does `build_conditional_branch(zero, trap_bb, cont)` — no policy gate. **The runtime-fault
path was built and the refusal was never revisited.**

> **This is the THIRD recorded blocker this session found to have stopped being true**, after
> `seed_reconstruct_shared` and the single-head reconstruct route. **A stale blocker is more
> expensive than a stale figure**: a wrong number misleads a reader, a wrong blocker stops work.

## What the lowering must reproduce, read from the VM

`src/vm.rs:6365`:

1. **Static**: `frac_bits >= word_bits` → `InvalidBytecode`. The count is a compile-time operand, so
   this becomes a **lowering-time refusal**, exactly as `FixedMul` and `FixedToWord` already do.
2. **Zero divisor** → `VmError::DivisionByZero`, unconditionally.
3. **Otherwise**: widen to `i128`, **shift the dividend LEFT** by `frac_bits`, divide, then
   **SATURATE** to the word range — not wrap.

**Both idioms already exist in the file**: `FixedMul` does widen/shift/saturate, `Div` does the
zero-check trap. This is composition, not new machinery.

## The one asymmetry to get right

`FixedMul` shifts the product **right**; `FixedDiv` shifts the dividend **LEFT** before dividing.
Copying `FixedMul`'s shift direction would produce a plausible, wrong, silently-agreeing-on-zero
result. **Read the VM line, do not pattern-match the neighbour.**

**`i64::MIN / -1` is NOT the same case here.** In `Div` it is handled by substituting a divisor of 1,
which is exact for the wrapping integer forms. **`FixedDiv` saturates**, and the dividend is widened
to `i128` first, so the quotient cannot overflow the wide type and the clamp handles the range. **Do
not copy `guard_min_div_neg_one` in without establishing it is needed** — and if it is not, say so.

## Prior failures this is exposed to

1. **Assuming a neighbour's shape.** The shift direction is the trap.
2. **Deriving a quantity and naming it.** Committed two iterations ago.
3. **A vacuous test.** Eight guards or filters broke this session. A differential that never reaches
   the new opcode asserts nothing — **check the corpus actually emits `FixedDiv`**, and if it does
   not, say so and use a constructed subject.
4. **Reporting a figure without the command that produces it.**
5. **Running the two suites in parallel** — invalidates the perf canary. Sequential.
6. **Claiming a census moved without re-deriving it.** Lowering an opcode moves `isa_lowering_census`
   from 60 to 61 and the refusal list from 2 to 1. **Re-derive; do not assert.**

## Specific wrong turns to avoid

- **Do not edit `src/`.** The VM is read-only here; it is the specification being reproduced.
- **Do not lower `Len` in the same increment.** Two opcodes, two reasons, one measurement each.
- **Do not weaken the static refusal** to make a test pass. The VM fails closed there and so must the
  backend.
- **Do not claim byte-identical agreement without a differential that reaches the opcode.** If no
  corpus module emits it, the honest claim is narrower and must be stated as such.
