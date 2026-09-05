# The refusal-surface census was attempted, found unsound, and is NOT published

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **2026-09-04.** **Status: no census. The instrument does not measure what it appears
to measure, and the table it produced would have been wrong in both columns.**

---

## What was attempted

Both module-level guards had just been covered, so the same method — enumerate a class, check each
member carries the discipline — was pointed at the **refusal surface**: `LowerError`'s eleven variants.
The question was which are produced and which are guarded by a test.

The instrument was two greps per variant: occurrences of `LowerError::V` in `src/lib.rs`, and test
files naming it.

## Why the result is not published

**Both columns count MENTIONS, and a mention is not what either column claims.**

**The "produced" column is wrong.** `Diagnostic` scored 4. Those are a `Display` arm and two
`matches!` filters — **no constructions at all** at those sites. `InvalidIr` scored 3: a doc comment,
a `Display` arm, and **one** genuine construction, wrapping LLVM's own module-verification failure.

**The "tested" column is wrong in the other direction.** `EscapesItsIteration` and
`YieldEscapingLoopComposite` scored zero. **Their behaviour is tested** — `interproc_yield_escape.rs`
drives `yield_escape_hazards` and `loop_body_sites` directly, exercising the analysis without ever
naming the error type. **A refusal can be thoroughly guarded and never mention its own variant.**

So the table said five variants were unguarded. **That would have been a fabricated finding**, and it
was the obvious reading of a table that took two minutes to produce.

## The rule this leaves

> **A grep for a type's name measures MENTIONS. Display arms, doc comments, `matches!` filters and
> imports all match, and behaviour can be tested through an analysis that never names the type.**
> Neither direction of the error is conservative: the count over-reports production and under-reports
> coverage, so it is wrong in the flattering direction on one axis and the alarming direction on the
> other.

## What a sound instrument would cost

The honest measurement is the one the mutation sweep already implements for opcodes: **disable the
refusal at its construction site and see whether any test fails.** That is a rebuild and a suite run
per variant — on the order of fifteen minutes each, so roughly three hours for eleven.

**That is affordable, unlike the 60-hour opcode sweep**, and it is the shape a future increment should
take. It is not attempted here because attempting it badly is how the unsound table happened.

## CALIBRATED 2026-09-04: the method works, the cost is measured, and the extrapolation has an assumption

One variant was run as a **control** — `UnsupportedWordWidth`, whose single construction site is known
and which had just been guarded, so a failure to detect would indict the method rather than the tree.

**The refusal was disabled entirely and the suite run with `--no-fail-fast`.**

| | |
|---|---|
| cost, one variant, one configuration | **13m 0s** |
| result | 469 passed, **3 failed**, 93 binaries |
| detection set | `a_module_level_refusal_is_visible_to_module_refusals`, `exactly_one_word_width_is_accepted_and_the_partition_is_complete`, `the_embedded_targets_are_refused_for_word_width_not_float_width` |

**The method detects, and detects specifically**: a control, plus two independent catchers, with no
unrelated collateral failures.

### The cost prediction missed, and the cause is worth more than the number

The estimate recorded ABOVE, before the calibration, was *"fifteen minutes each, roughly three hours
for eleven"*. **That was accurate.** The in-flight prediction made at calibration time was 7 to 8
minutes — **a good recorded estimate revised downward, without consulting it, into a worse one.**

**That is the record-not-consulted shape applied to this line's own figure from the previous
increment.** The remedy already written in `SCOPE_DELETION.md` is to search before asserting; it was
not applied to an estimate because an estimate did not feel like a claim.

### The extrapolation, and the assumption it rests on

Eleven variants at 13 minutes is **about 2.4 hours** for one configuration, which is affordable.

> ⚠ **THAT ASSUMES EVERY VARIANT DISABLES AS CLEANLY AS THIS ONE.** `UnsupportedWordWidth` has a
> single construction site returning a `Result` that is trivially replaced by `Ok(())`.
> `UnsupportedShape` and `MalformedInput` are constructed in many places, and disabling a refusal may
> fail to compile or change behaviour so that tests fail for unrelated reasons. **The per-variant
> mutation is engineering work, not a uniform transformation**, and the wall-clock figure does not
> cover it.

## The one thing the attempt did establish

`InvalidIr` has exactly **one** construction, wrapping LLVM's own module verification. It is an
internal-soundness path that should never fire, and no test names it. **That is defensible for a
should-be-impossible guard and is recorded as an observation, not a gap.**
