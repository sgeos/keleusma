# BRIEF — the exact cause of the last two composite refusals

**Written**: 2026-08-27, nineteenth loop iteration. **For this line's own use.**

## Where the frontier is

The backend lowers **1070 of 1074 chunks (99.6%)**. **Five modules are refused end to end**: two on
`NewComposite`, one each on `Len` and `Stream`, one chunk-level. **`NewComposite` is the largest
single group.**

## A hypothesis measured and refuted, cheaply

The obvious guess was the **`Boxed`** composite form, which the backend does not lower — it handles
only `Flat`.

**Measured: the corpus contains ZERO non-`Flat` composites.** All **256** sites are `Flat`. **The
guess was wrong, and one probe cost less than an increment built on it would have.**

## What the refusal must actually be

The `Flat` arm has exactly three refusal conditions:

1. the region pointer is absent — `lower_chunk` did not receive one;
2. no region placement exists for the site;
3. **an operand has unknown packed width** — *"a guess here mispacks silently, and a `Byte` and a
   `Word` are indistinguishable on this stack."*

**Condition 3 is almost certainly it, and there is a standing prediction to test against.**
`spike_composite_split` argued the composite class is **two blockers, not one**:

> *"Every composite READ op already bakes what a lowering needs… **No shape recovery is involved.**
> Only `NewComposite::Flat` is short. It carries the TOTAL body size, not the per-field breakdown, so
> packing `count` popped values requires knowing each one's width. **That, and only that, is what
> type recovery buys.**"*

**So the increment is: establish which condition fires, and report it against that prediction.** If
it is condition 3, the spike's split is confirmed on the residue it predicted. If it is 1 or 2, the
spike was right about the general case and something else is happening here.

## Prior failures this is exposed to

1. **Building on an unmeasured hypothesis.** Already avoided once this iteration; the same trap sits
   behind "it must be the width one".
2. **Reporting a cause without naming which condition produced it** — the standing rule that
   "refused" without a stage is not a result.
3. **A vacuous probe.** Thirteen guards or filters broke this session. **Assert the module actually
   refuses on `NewComposite`** before reading any cause.
4. **Confirming a prediction by restating it.** The spike's claim is the thing being tested, so it
   must not also be the evidence.
5. **Reporting a figure without the command that produces it.**
6. **Running the two suites in parallel** — invalidates the perf canary. Sequential.

## Specific wrong turns to avoid

- **Do not edit `src/` or any read-only file.** If the cause needs operand-width information the
  backend cannot obtain, that is a finding, not a licence to change the verifier.
- **Do not implement width recovery in this increment.** Establishing the cause is the goal;
  `FixedDiv` showed that a lowering plus its differential is a full iteration by itself.
- **Do not report "2 modules" without naming them.** A count is not a finding when the population is
  two.
- **Do not treat 99.6% as "nearly done".** These are the residue precisely because they are the hard
  cases, and the remaining four chunks may each need different work.
