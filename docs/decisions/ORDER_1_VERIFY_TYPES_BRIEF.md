# BRIEF — the next unseeded stage, and two things found while reading it

## The target

Four agreeing stages remain unseeded and vary neither axis: `analyze.kel`, `codegen.kel`,
`verify_types.kel`, `verify_yield.kel`. **`verify_types.kel` is the tractable one**, because its input
is synthetic — a table of `(left tag, right tag)` operand pairs — rather than a marshalled module
structure. No accessor is needed; the generic slot route reaches it.

Its output channel is `verdict`, folded across the rows and **sticky on reject**.

## Two findings from reading it, neither of which I can fix

**1. `cmd` IS DECLARED, DOCUMENTED, AND NEVER READ.** The shared block's header says *"`cmd` selects
the operation; `verdict` carries the answer out"*, and `cmd` appears exactly twice in the file: the
comment and the declaration. The bottom of the file states the truth — *"this stage has one job and
no commands"*. **A host following the documentation would write a channel that nothing reads.** This
is `src/selfhost/kel/`, the other line's, so it is REPORTED, not repaired.

**2. `ty_max_steps()` IS 1801, AND THIS HARNESS DRIVES 60 TICKS.** The stage states its own bound so a
host can size a drive loop: one step per row across every table at its cap, one per phase boundary,
one to report. **At full tables the verdict is unreachable within 60 ticks.**

That second one is not a blocker — it is a design constraint on the subjects. With SMALL tables each
unused phase advances in a single step, so a fold over `k` rows completes in roughly `k` plus the
phase count plus one. **Subjects must be sized to complete, and the tree must say that is why they
are small** rather than leaving a reader to think small tables were arbitrary.

## Prior failures and the specific wrong turns to avoid

- **AN ACCEPTING SUBJECT IS VACUOUS, and this is measured across the whole `verify_*` family.** The
  verdict is written as reject and the buffer already holds accept, so a clean table changes nothing
  comparable. **Every subject needs a row that rejects**, or it inflates a count while comparing
  nothing. The accept direction belongs in a separate control, not in the driven set.
- **DO NOT WRITE TO `src/selfhost/kel/` OR `src/selfhost/mod.rs`.** Both are the other line's. The
  `cmd` finding is a report. If a stage cannot be seeded without changing them, say so.
- **DO NOT SIZE SUBJECTS BY GUESSING.** The stage publishes `ty_max_steps()` precisely so a host does
  not restate a sum that drifts. Derive the tick need from the subject's row count and CHECK it
  against the drive budget, rather than assuming a small table is small enough.
- **A SUBJECT THAT NEVER REACHES ITS VERDICT MUST BE REPORTED, NOT COUNTED.** This is the same shape
  as the truncated token stream caught one increment ago: a run that looks seeded and stops early
  compares a prefix. **A floor on "did the verdict move" is the check; a non-empty segment is not.**
- **SLOT ORDER IS LOAD-BEARING.** The file records that inserting fields mid-block broke four tests
  because the shared block is addressed by SLOT and everything after shifted. Resolve slots BY NAME
  through the layout, never by a computed index.
- **DO NOT claim the unseeded set is closed.** Seeding one of four leaves three, and `analyze.kel`,
  `codegen.kel`, and `verify_yield.kel` take marshalled module structure rather than synthetic tables,
  so they are a different and larger problem.

## What a good outcome looks like

`verify_types.kel` is driven from more than one subject, each provably reaching its verdict and each
moving it; the seeded count rises to seven and the unseeded set falls to three, still named; the two
findings above are recorded where the other line will see them. **And the reason the subjects are
small is stated as the step bound, not left as an unexplained choice.**
