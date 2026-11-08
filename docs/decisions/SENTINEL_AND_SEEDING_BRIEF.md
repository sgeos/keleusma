# BRIEF — the negative-sentinel class, and what the last three stages need

**Written**: 2026-08-26, after absorption 12. **For this line's own use.** Not a decision document
and not a report to the operator.

## Why these two, and why now

Absorption 12 produced a confirmed defect of a specific shape: **a test assertion weaker than the
property it named.** `the_single_head_reconstruct_seed_drives_the_stage` asserted `nodes > 0` where
the property was *"is a real node count"*. It received `4` — a stale stack index — and passed. #279
turned that value into `-905`, and only then did the test fail.

**The value never became wrong. It was always wrong. The test became able to see it.**

That matters beyond one test, because #279 also **introduced a convention**: a stage reports a cause
by yielding `rc_fail_base() - code`, a NEGATIVE sentinel, in the same slot that otherwise carries a
count. Measured across the twelve stage sources: `parse.kel` carries 2 such patterns,
`reconstruct.kel` 3, `wire.kel` 91 (exempt from the gate), and the other nine carry none.

**So there is now a live encoding in which a plausible-looking integer means failure**, and this
line has **14 sites across 9 test files** that read a yielded `Value::Int` from a stage. Whether any
of the other 13 conflates the two is unknown, and "unknown" is the whole finding.

### Goal 1 — classify all 14, fix what is weak, keep the class closed

Not "grep for `> 0`". The property is: **does this site distinguish an error sentinel from a valid
result, or is it structurally unable to receive one?** Both answers are fine; an unexamined site is
not.

### Goal 2 — establish what `analyze.kel`, `codegen.kel`, `verify_yield.kel` actually need

The handoff already states the instruction and the reason: all three take **marshalled module
structure** rather than a synthetic table, the accessors live in `src/selfhost/mod.rs` which is
**read-only to this line**, and — verbatim — *"Do not assume the generic slot route reaches them just
because it reached four others. Establish what each actually needs before planning an increment
around it."*

**The deliverable is the establishment, not the seeding.** Three unseeded stages are the gate's
self-declared weakest part; each is one run of sixty ticks with no input, which is vacuity of exactly
the kind Goal 1 is about.

## Prior failures to not repeat

**1. Subject-shopping.** Widening a subject set until something passes is how a real gap becomes an
invisible one. The absorption-12 fix widened a subject set and is only defensible because **every
refusal is printed with its named cause.** Any widening here carries the same obligation.

**2. Unfireable cross-tree guards.** A guard pinned so the other line's repair *satisfies* it teaches
nothing. Pin equalities, not bounds, when the point is to be told about a change.

**3. Vacuous guards.** A clean guard proves its reach before its verdict is believed. An audit that
finds nothing must first demonstrate it can find something.

**4. Self-inclusion.** An audit whose population includes its own output is invalidated by running.
This bit twice: once when an excuse table's string literals satisfied the check they were excusing,
and once today when the provenance-repair commit inserted an unmeasured figure into the file it was
repairing. **A repair pass is a change and enters the tree unmeasured unless re-checked.**

**5. Reading a pipeline's status instead of the command's.** Four occurrences in one day. Do not
append a filter to a command whose exit status matters.

**6. Carrying a figure across a change that moves it.** Re-derive at every split point; splitting for
attribution is worthless without measuring at the split.

**7. Asserting a count without naming the command that produced it.** The "32 commits" figure
reproduced under none of nine measures.

## Specific wrong turns for THIS work

- **Do not edit `src/selfhost/`, `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`,
  `src/wire_schema.rs`, or `.github/workflows/`.** Owned by the `v0.2.3` line, read-only here. If
  Goal 2 concludes a stage needs an accessor that only they can add, **that conclusion IS the
  deliverable** — do not route around it.
- **Do not seed a stage in Goal 2.** The instruction is to establish requirements. A seeding that
  works is a bonus; a seeding that half-works and reports as seeded is the truncated-fold failure
  the handoff already records.
- **Do not treat "the site cannot receive a sentinel" as a pass without showing why.** The reason is
  the evidence; the verdict is not.
- **Do not let the sentinel constants drift silently.** They are the other line's; if this line
  copies them, the copy needs a guard that fires when the original moves.
