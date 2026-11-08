# BRIEF — which subjects would actually catch a wrong backend?

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Why now

Last increment put a floor under **61 modules executing and agreeing**. Flooring a number makes its
meaning load-bearing, so the next question is whether it measures what its name says.

`probe_agreement_depth.rs` already sized the doubt and deliberately stopped short — *"This file
REPORTS and does not classify"*:

> Of the 20 modules where ALL THREE are trivial, 15 are single-call and 5 are streams. The streams are
> the ones `is_vacuous` already catches; **the single-call ones are the blind spot this probe exists to
> size.**

`is_vacuous` returns false immediately for a module with fewer than two results, so every single-call
module inside the count is unexamined. **Agreement has never been shown to be evidence for them.**

## The question, made falsifiable

**If the backend were wrong, which subjects would notice?**

Answerable directly: run the VM on the original module and the native side on a **mutated clone**. That
is a simulated backend defect, and a subject that still agrees cannot detect that defect.

The existing mutation sweep answers a different question — which **opcodes** are detected *somewhere*.
This asks which **subjects** carry detection. A subject can be wholly redundant and the sweep would
never say so.

## Three outcomes that must not be conflated

| outcome | meaning |
|---|---|
| mutated and **detected** | the subject is load-bearing for that defect class |
| mutated and **not detected** | agreement did not distinguish a wrong backend |
| **no applicable mutation site**, or the mutant would not lower | **UNMEASURED** |

**Reporting an unmeasured module as undetecting would be a manufactured finding.** This line has
already shipped one guard that counted 33 where the truth was 10 by matching a word in comments.

## Wrong turns to avoid

- **Do not tune the mutation to the modules under test.** `corpus_differential.rs` records the rule:
  picking a value "because that is where the undetected sites are would make this a demonstration
  rather than a measurement". Pre-register the mutation family and the order it is tried.
- **Do not mutate both sides.** Mutating the shared module makes both sides compute the same wrong
  answer and agree — measuring nothing while looking rigorous.
- **Do not delete or exempt a thin subject on this evidence.** Undetected against *one* mutation family
  is not "detects nothing". Scope the claim to the family actually run.
- **Do not put this in a new test binary.** The machinery is in `corpus_differential.rs` and an
  integration binary cannot import another's helpers. A control exercising a different code path would
  validate nothing.
- **Do not let a lowering failure count as detection.** A mutant that refuses to lower produced no
  comparison; that is unmeasured, not caught.
- **Reconcile the counts rather than picking one.** 20 all-trivial, 15 single-call, and the
  differential's 12 "none of the three" describe overlapping populations. This line has been bitten by
  population mismatch four times; state the definitions and where they differ.
- **Do not widen anything** to make a mutant run.

## What good looks like

A per-subject detection figure, with the unmeasured population reported separately and honestly, and a
statement of exactly which defect family the result covers.
