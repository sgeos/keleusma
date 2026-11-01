# BRIEF — the guard's population is narrower than the thing it guards

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Widen the corpus fingerprint to every root the figures actually depend on | yes |
| 2 | Establish why two censuses over different populations report the same total | yes |
| 3 | Correct the "cross-check" claim if that reason is not what was published | yes |
| 4 | Keep the gate green and the branch published | yes |
| 5 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |

## Rationale

`corpus_fingerprint.rs` was shipped last increment covering **three** roots. The censuses that produce
this line's published figures read **four**: `examples/scripts`, `src/selfhost/kel`,
`examples/rtos/scripts` and `compiler/kel`. **Seven files are read and not guarded.**

**This is the third occurrence of one defect at three granularities.** The `v0.2.3` line pinned a
value whose input was a directory scan. This line then scanned three named directories where the
loaders recurse, finding 57 files against 67. Now the guard built from that lesson covers three roots
where the loaders read four. **Each time, the population watched was narrower than the population that
matters**, and each time it was invisible because the narrower scan returned a well-formed answer.

**A second, sharper problem.** Last increment recorded that two censuses "now agree at 1074, a
cross-check that had not existed". **They read different root sets**, so agreement is not the
straightforward corroboration that was claimed. Either the extra roots contribute nothing, in which
case the agreement is real but for an unstated reason, or something else is going on. **The claim was
published and must be settled by measurement.**

## Prior failures to avoid repeating

1. **A guard watching a smaller population than the thing it protects** — twice now, this being the
   third. **Derive the scope from what the consumers read, not from what a directory list looks like
   it means.**
2. **An agreement between two numbers is only evidence if they measure the same thing.** Coincidence
   and corroboration are indistinguishable until the populations are compared.
3. **A test named for a canary firing did not make it fire.**
4. **`DefaultHasher` is not stable across toolchains.**
5. **Nine recorded premises have been found false in consecutive increments.**

## Specific wrong turns to avoid

- **Do not fingerprint only the files that compile.** The loaders skip non-compiling files, but a
  change that makes one compile would move the figures, so the guard must watch what is *read*.
- **Do not assume the extra roots contribute nothing.** Measure it, and if they contribute nothing,
  say why — a prelude is not a standalone program, and that is a reason rather than a coincidence.
- **Do not quietly restate the cross-check claim.** If it was overstated, the correction is the
  deliverable and the superseded wording should remain visible.
- **Do not widen the fingerprint to roots the figures do not depend on**, which would train the reader
  to ignore it.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
