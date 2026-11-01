# BRIEF — figures quoted without the population they were measured over

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Reconcile the composite-site counts, which the tree gives as both 239 and 256 | yes |
| 2 | Attach a population to any figure that turns out to be underspecified | yes |
| 3 | Keep the gate green and the branch published | yes |
| 4 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |

## Rationale

Last increment's correction generalises: **quote the population with the number.** A bare figure was
never wrong so much as underspecified, and underspecified is how two numbers come to be compared that
were never measuring the same thing.

Applying it immediately finds a candidate. The tree gives the corpus's composite construction sites as
**239** in `region.rs` and as **256 in 35 chunks** in the handoff, the latter described as *"the third
independent walk to report it"*. **A count over 35 chunks cannot exceed a corpus-wide count**, so
either the populations differ and neither says so, or one is wrong.

This is worth an increment because it is the same shape as the last three findings: a well-formed
number about the wrong set, invisible because nothing errors.

## Prior failures to avoid repeating

1. **Three increments running, a watched population was narrower than the one that mattered.**
2. **Two measurements agreeing over different populations is not corroboration.** That claim was
   published here and had to be corrected.
3. **The first explanation offered for the agreement was also wrong** — the extra files do compile.
   The test asserting otherwise failed, which is the only reason the second explanation is trustworthy.
4. **A heuristic walk produced a confident wrong answer**; the published stack-effect tables produced
   the right one.
5. **Ten recorded premises have been found false in consecutive increments.**

## Specific wrong turns to avoid

- **Do not assume one figure is simply stale.** Both may be right over different sets, and deciding
  that without measuring is how the wrong one gets deleted.
- **Do not reconcile by reading the code that produced them.** Run both walks and compare, then read
  to explain the difference.
- **Do not "fix" a figure by changing the walk to match the other.** If they measure different things,
  the repair is to say what each measures.
- **Do not treat a subset relation as proof of consistency.** 35 chunks being a subset of the corpus
  makes the inequality suspicious; it does not by itself say which number is about what.
- **Do not widen the audit to every number in a 7000-line document.** Take the ones that are quoted
  more than once with different values, which is where a contradiction is already visible.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
