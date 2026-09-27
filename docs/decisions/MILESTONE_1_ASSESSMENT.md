# RETRACTED: this document assessed work a previous session had recorded as DONE

**Written and retracted on 2026-09-27, within the hour.** It is kept, emptied of its claims,
because the way it went wrong is worth more than anything it asserted.

## What it claimed, and why every version was unsound

It set out to assess roadmap order 1 — *"the self-hosted compiler's own bytecode runs correctly
as native code, differential-tested against the VM"* — and state what remained.

- **First version**: "2 of 12 stages driven on seeded input; ten per-stage seeders are the
  work." Wrong. It rested entirely on `probe_stage_vacuity`'s table, which prints
  `seeded: no len/bytes pair` for ten stages. **That describes THAT PROBE'S seeder**, which
  understands one convention, not the project's coverage.
- **Second version**: "5 of 12; seven seeders remain." Also unsound. Five is the number of
  `*_agrees_with_the_vm_*` tests in `stage_differential.rs`, which is not the same quantity as
  stages seeded.
- **Both versions** were written without reading the handoff section headed
  **"► WHAT IS DONE, SO IT IS NOT REDONE"**, which states: *"Order-1 gate: **12 of 12 stage
  sources seeded, 0 unseeded** (was 3 unseeded). The last three went in without the read-only
  accessors that were assumed to be the only route; `codegen.kel` is seeded by chaining
  `reconstruct.kel`'s published AST."*

**A section exists in this line's own handoff for the express purpose of preventing this, and
it was not read before writing a "what remains" document.** That is the finding.

## What is measured and still stands

- `stage_differential.rs` carries agreement tests for **five** stages — `lexer`, `parse`,
  `verify_yield`, `analyze`, `codegen` — each with a negative control, and two of them are
  driven on **the preceding stage's real output** rather than a synthetic seed.
- Under the CORPUS drive, which is an all-zero shared segment and NOT
  `stage_differential`'s seeded one, **8 of 12 stages yield a single repeated value** over 60
  ticks, and `lexer.kel` yields its end-of-source marker `62` sixty times. Corpus agreement
  alone therefore cannot carry the gate.

Those two facts are consistent with each other and with the prior session's claim. **What is
NOT established is how they reconcile** — whether "12 of 12 seeded" counts seeding in the
corpus harness, which is a different quantity from an agreement test in `stage_differential`.
Settling that needs the corpus harness's seeding read directly, which this document did not do.

## The process lesson, which is the reusable part

**Read "WHAT IS DONE, SO IT IS NOT REDONE" before writing anything titled "what remains".**
The section is in the handoff above the pickup list and exists for this.

**And stop answering coverage questions with greps.** Six searches in one iteration returned
the wrong answer to the question asked: a prefix search for `wa.` when slots are addressed
unqualified; an extraction requiring a literal argument when the names are passed through a
loop variable; and four earlier. Each looked like an answer. The instrument for "what does
this harness cover" is the harness's own test list and its own reported counts.

## No claim about milestone 1 is made here

The gate may be met, partly met, or not met. **This document asserts nothing about it**, and
the next session should start from the handoff's done-list and the corpus harness rather than
from anything here.
