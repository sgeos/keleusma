# BRIEF — the instrument built to check a claim had an overclaiming proxy

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Correct the property that is named "yields a composite" but tests co-occurrence | yes |
| 2 | Re-derive the distribution and restate the outlier's real property count | yes |
| 3 | Keep the gate green and the branch published | yes |
| 4 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |

## Rationale

Last increment measured six structural properties across 69 modules to test a clustering claim, and
reported `14_frame_log.kel` as the corpus's outlier at **four of six**.

**Reading that module shows one of the four is wrong.** Its entry is `loop main(tick: Word) -> Word`
— it yields a **Word**. The property counted as "yields a composite" is implemented as *a chunk
containing both a `Yield` and a `NewComposite`*, which is co-occurrence, not the claim in its name.

**This is the defect class the whole session has been finding, now in the instrument built to correct
a claim.** A test named for a canary firing did not make it fire; a guard's population was narrower
than its subject three times; and here a property's name asserts more than its body checks. **An
instrument is not exempt from the scrutiny applied to the claims it measures.**

**The correction is available and already used elsewhere on this line.** For a `loop` chunk the
declared return type IS the yielded type, so the signature's `ret` shape distinguishes a composite
yield from a `Word` one — the same reasoning that refined the interprocedural residual.

## Prior failures to avoid repeating

1. **A name that asserts more than the body checks.** That is exactly what is being fixed; do not
   introduce another while fixing it.
2. **Thirteen recorded premises found false or overstated in consecutive increments.** Write the new
   prediction down: the outlier drops to three properties and the "yields a composite" count falls.
3. **A figure without its population.** State the corpus with every number.
4. **`stack_growth`/`stack_shrink` are the peak model.**
5. **Selection by attention.** The re-measurement must still be over all modules, not the ones in
   mind.

## Specific wrong turns to avoid

- **Do not simply rename the property to match the weaker test.** "Constructs and yields" is a real
  property but it is not the one that mattered to the escape question, and renaming would keep a
  number that answers nothing.
- **Do not assume the other five properties are sound because one was not.** Check whether each name
  matches its body, and say which were checked.
- **Do not re-run the distribution and report only the new numbers.** The previously published counts
  are in the tree and must be visibly corrected.
- **Do not treat the outlier's demotion as removing the finding.** Whether it remains the outlier at
  three properties is a separate question the numbers answer.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
