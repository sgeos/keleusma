# BRIEF — three module counts, one corpus

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Reconcile the module counts 74, 71 and 69, which describe the same corpus | yes |
| 2 | Attach a population to each surviving figure | yes |
| 3 | Absorption 29 | yes |
| 4 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |

## Rationale

`bound_transfer.rs` prints **"modules examined: 74"** and **"modules compared: 71"** in two censuses in
the same file, while every other census on this line reports the four-root corpus at **69 compiling
modules** and the fingerprint pins **74 files**.

**Three numbers for what a reader takes to be one population.** That is the third instance of this
exact shape: **239 against 256** for composite sites, and **91 against 67** for modules, both of which
turned out to be a stale figure and a duplicated directory respectively. **Neither was visible until
two numbers were placed side by side.**

**74 is suggestive**: it is exactly the number of `.kel` files the fingerprint pins, so one census may
be counting files where another counts modules that compile. **71 fits neither**, which is the part
that cannot be explained by that guess and is therefore the reason to measure rather than reason.

## Prior failures to avoid repeating

1. **A figure without its population** is how two numbers get compared that measure different things —
   stated three increments ago and immediately useful again here.
2. **Two measurements agreeing over different populations is not corroboration**, and by the same
   token two disagreeing may both be right.
3. **The first explanation offered for a discrepancy has twice been wrong** — the extra roots "do not
   compile" (they do), and the timing suspicion about a skipped test (it ran). **Offer the
   explanation, then test it.**
4. **A scanner counted prose** and reported 33 for 10, caught only by disagreement.
5. **Eighteen recorded premises found false or overstated.** Predict: 74 counts files or attempted
   loads, 69 counts compiled modules, and 71 is a filtered subset whose filter is not in its label.

## Specific wrong turns to avoid

- **Do not assume one census is wrong.** Three populations can all be correct and merely unlabelled,
  which is the likeliest outcome and still worth fixing.
- **Do not reconcile by reading the loaders.** Run them and print what each set contains, then read to
  explain the difference. Reading first is how the last two explanations came out wrong.
- **Do not change a figure to make two agree.** If they measure different sets, the repair is to say
  what each measures.
- **Do not stop at the counts.** A difference of two modules is worth naming to the module, because
  "71 versus 69" hides which two, and the answer may be more interesting than the arithmetic.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
