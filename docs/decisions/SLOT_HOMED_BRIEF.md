# BRIEF — the numerator left unverified last increment

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Re-derive whether any corpus composite is slot-homed, by two independent methods | yes |
| 2 | Restate or retire the claim according to what is measured | yes |
| 3 | Keep the gate green and the branch published | yes |
| 4 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |

## Rationale

Last increment corrected a stale denominator — the corpus's composite construction sites are **256
over the four-root corpus's 69 modules**, not the carried 239 — and **deliberately did not restate the
other half of the same sentence**, that *"0 of them are slot-homed"*. Correcting a denominator does
not license the numerator, and that half has had no producer for as long as the other did.

**It is worth finishing rather than leaving half-corrected.** A sentence with one verified half and
one carried half is harder to use than one that is wholly stale, because the verified half lends it
credibility.

**The claim also does real work.** `region.rs` exists on the premise that composites are temporaries
rather than program state; if any were slot-homed, a chunk-relative placement would be the wrong
model for them. So this is not only bookkeeping.

**Two independent methods are available, which is what makes a valid cross-check** — different
methods over the same population, as distinct from the invalid one this line published two increments
ago:

1. **A producer walk**: a `NewComposite` whose value is stored by `SetData`, classified by the target
   slot's visibility.
2. **A module-level field**: `persistent_composite_bytes`, which is non-zero when a module keeps a
   composite in a private slot.

## Prior failures to avoid repeating

1. **`stack_growth`/`stack_shrink` are the peak model, not pop and push counts.** A producer walk must
   use `verify::op_depth_effect`. A walk built on the wrong tables mis-attributed a stored value once
   already this session.
2. **A heuristic walk gave a confident wrong answer.** Do not take "the nearest preceding
   `NewComposite`".
3. **Two measurements agreeing over different populations is not corroboration.**
4. **Eleven recorded premises have been found false in consecutive increments.** Write the prediction
   down first: the expectation here is that both methods report zero.

## Specific wrong turns to avoid

- **Do not treat a zero from one method as the answer.** The point of two methods is that a zero from
  a walk that cannot see anything looks identical to a real zero.
- **Do not conflate private and shared slots.** The claim is about private, arena-resident slots that
  survive `Reset`; a composite written to a shared slot is a different fact and should be reported
  separately rather than folded in.
- **Do not assert the claim if the methods disagree.** A disagreement is the finding, and it should
  be reported with both numbers rather than resolved by preferring one.
- **Do not leave the sentence half-corrected again.** Whatever is measured, `region.rs` should end up
  saying something wholly derived from a current walk.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
