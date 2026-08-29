# BRIEF — tests that can pass without testing

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Determine which skippable tests are actually skipping on this machine | yes |
| 2 | Make a skip visible in the suite's own accounting, not only in captured output | yes, if cheap |
| 3 | Absorption 28 | yes |
| 4 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |

## Rationale

The closed name audit surfaced a different shape on its way out: `retcon_m2`'s buffer test **skips
silently when no C compiler is present**, passing without testing anything. That is not a
name-versus-body defect; it is a test whose green is conditional on the environment.

**Measured: 10 of 325 tests can return before asserting anything.** They cluster in the families that
need a toolchain — linking a native object, timing a resumption, computing a stack bound end to end.

**The number that matters is not 10.** It is how many are skipping **right now**, because a skipped
test reports as *passed* and contributes to the 353 this line quotes as evidence. **A suite's pass
count is only as meaningful as the fraction of it that executed.**

**This is the same class as everything the last three increments found**, arriving from a different
direction: a green that does not mean what a reader takes it to mean. The name audit asked whether a
test proves its claim; this asks whether it ran at all.

## Prior failures to avoid repeating

1. **A test named for a canary firing did not make it fire.**
2. **A guard whose population was narrower than its subject**, three times.
3. **A gate run killed by a signal reported "51 passed"** — plausible numbers that were not a result.
   **A skipped test is the same hazard in miniature.**
4. **Seventeen recorded premises found false or overstated.** Predict: at least one of the ten is
   skipping on this machine.
5. **An edit that silently did nothing** because its assertion was omitted.

## Specific wrong turns to avoid

- **Do not count a skippable test as vacuous.** The set of ten is the population at risk; how many
  actually skip is a separate measurement and the only one worth quoting.
- **Do not "fix" a conditional skip by making it fail.** A test that needs a C compiler should not
  break a machine without one; the defect is invisibility, not the skip.
- **Do not claim the suite is compromised.** If the skips announce themselves and the count is small,
  say so plainly rather than dramatising a bounded finding.
- **Do not change what the pass count means without saying so.** If a skip becomes visible in the
  accounting, the recorded figures must be restated with that change named.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
