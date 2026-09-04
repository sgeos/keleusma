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

## The one thing the attempt did establish

`InvalidIr` has exactly **one** construction, wrapping LLVM's own module verification. It is an
internal-soundness path that should never fire, and no test names it. **That is defensible for a
should-be-impossible guard and is recorded as an observation, not a gap.**
