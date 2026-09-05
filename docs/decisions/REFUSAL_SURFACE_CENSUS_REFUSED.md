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

## ⚠ RETRACTION: the principle below was ALREADY RECORDED, and better

`native_codegen/tests/refusal_classes.rs` opens with **"A refusal's class is carried by its type, not
by its word order."** It records that `LowerError::UnsupportedOp` was once a `String` built at **31
sites** carrying four unrelated conditions, and that `isa_lowering_census` attributed refusals by
taking the leading word of the sentence — so `Const(60000) out of range`, a malformed constant
**index**, was credited to the `Const` **opcode**, which the backend lowers in nearly every module.

Its sharpest line is one this file did not reach: **the corpus never fired a misattributing site, so
every published figure was correct — the column was clean because of what the corpus happens to
contain, not because the query could not go wrong.** That is the distinction between a guard that
holds and a guard that was never reached.

**So the constructive lesson recorded below was re-derived, not discovered**, across three failed
instruments and two increments. **The pointer was in this line's own first census**: `UnsupportedShape`
scored 1 in its "tested" column, and that 1 was this file. It was not opened.

**The remedy for this shape is recorded in [`SCOPE_DELETION.md`](./SCOPE_DELETION.md) as searching
before an increment rather than checking during one.** It was not applied, again, while actively
writing about refusal measurement.

## WHAT IS ACTUALLY NEW: the largest refusal class is one condition in disguise

`UnsupportedShape(String)` is constructed at **nine** sites. Reading all nine:

| what is refused | sites |
|---|---|
| **a float width that is not 4 or 8** — entry-ABI signature, native return shape, division, comparison, constant, arithmetic, negation, and a generic op arm | **8** |
| a native call setting the B35 P7 error-reify flag | 1 |

**The backend's largest refusal class is almost entirely `float_width_lowered` saying no.** The enum
documents it as "a type or feature this backend lacks, not attributable to one opcode", which is true
and hides that eight ninths of it is a single, already-governed condition.

> **This is the next candidate for the treatment `UnsupportedOp` already received**: give the refusal a
> structured subject, so a census reads the float width as data rather than parsing a sentence. The
> blast radius is small — one test file matches the variant and **no test asserts on its message
> text**.

## THREE TEXTUAL INSTRUMENTS, THREE BLIND SPOTS — the cheap path is closed

Each attempt was falsified by a control whose answer was known in advance.

| instrument | blind spot | how it was caught |
|---|---|---|
| grep the variant name | counts `Display` arms, doc comments, `matches!` filters | `Diagnostic` scored 4 with **no constructions**; `EscapesItsIteration` scored 0 tests while its behaviour is exercised through the analysis API |
| grep `Err(LowerError::V` and `map_err` | misses variants built by a **constructor function** | **`UnsupportedOp` scored ZERO.** It is the backend's most common refusal, so zero is impossible — the instrument is wrong, not the code |
| — | `UnsupportedOp` is built by `fn unsupported_op(op, detail)`, so every refusal site calls the helper and never names the enum path | reading the source at the site |

> **There is no cheap textual instrument for "which refusals are produced and where."** Each fix for
> one blind spot introduces another, and the errors are not conservative in a consistent direction.
> **The sound instrument is behavioural** — disable the refusal and see what fails — already calibrated
> at 13 minutes per variant.

### The constructive lesson, which the project already applied once

`unsupported_op`'s own comment says it *"takes the opcode name separately from the prose so the two
cannot drift: the census reads `op`, and the sentence is for a human."*

**That is why the ISA census can measure opcode refusals at all.** The refusal carries a STRUCTURED
field naming its subject, so a census reads data rather than parsing English or matching source text.

> **The way to make a refusal surface measurable is to give the refusal structure, not to scan for it
> afterwards.** `UnsupportedOp` and `UnsupportedWordWidth` carry their subject; `MalformedInput`,
> `UnsupportedShape` and `Internal` carry prose. **The measurable ones are exactly the structured
> ones**, and that is a design property rather than a tooling gap.

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
