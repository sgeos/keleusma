# BRIEF — the handoff's own state table drifted while I stamped it

## The finding

`docs/process/handoffs/v0.3.0.md` carries a table headed *"State, every figure
re-derived at the stamp"* inside `► RESUME HERE`, the one section that declares
itself authoritative. **I stamped that file on five consecutive increments and
never updated the table.**

One quantity, three values in one file:

| where | says | truth |
|---|---|---|
| the banner | 561 tests (551 + 10) | |
| the state table | 562 tests (552 + 10) | **576 (566 + 10)** |
| `test files` row | 120 | **124** |

The file warns about exactly this. It records four cases of records outliving
their subjects, and states plainly: **"Six instruments now fire on drift in code.
None fires on drift in prose."** I then produced a fifth case, in the section
headed *every figure re-derived*.

## Why the earlier attempt was right to be rejected, and this one is different

A general prose-drift census was attempted and **rejected on measurement**: two
candidate matchers returned 169 and 32 lines, both dominated by narrative, where a
sentence about a corrected claim is indistinguishable from the claim. A census
whose population is ten times its signal reads as coverage while being none. That
reasoning stands.

**But this table is not general prose.** It is a fixed set of labelled rows whose
values are numbers, most of them derivable from the tree. The rejected instrument
was a matcher hunting claims across a whole document; this one reads named rows
from one table at one anchor. The population is the table, and the signal is the
whole population.

## The row that is NOT derivable, which is the interesting part

*"576 tests, 0 failed, both float configurations, every half FROZEN"* is a **run
result**, not a property of the tree. No guard can re-derive it by reading files,
because it is the outcome of executing them.

Treating it like the others would be the mistake. A derivable row must be checked;
a run-result row must instead **name the commit it was measured at**, so a reader
can see whether it belongs to the tree in front of them. A figure with no
provenance and no checker is the exact shape that drifted.

## What to build

1. A guard that parses the named rows of that one table and fails when a derivable
   figure disagrees with the tree, saying which row and what the tree says.
2. Non-vacuity: the parse must find the expected rows, or fail as a broken probe.
   A guard that silently matches nothing is the failure mode this session has
   already hit twice — a probe-name extraction that returned zero, and a
   case-sensitive premise matcher that saw two thirds of its subject.
3. The run-result row carries the commit it was measured at.
4. The banner figure agrees with the table, or stops being a figure.

## Wrong turns to avoid

- **Scoping the parse to the whole file.** It is thousands of lines and most of it
  is deliberate history under `◄ RECORD`, full of older numbers that are correct
  *as history*. Anchor on the `► RESUME HERE` table and nothing else.
- **Checking a number the tree cannot answer.** If a row cannot be derived, give
  it provenance instead; do not invent a derivation that approximates it. An
  approximate check that passes is worse than no check.
- **Patching the numbers and calling it done.** The numbers were patched five
  times by hand already — that is how they drifted. The deliverable is the guard.
- **Claiming this closes prose drift.** It closes the FIGURES in one table. The
  next stale sentence will be somewhere this does not look, and the file should
  keep saying so.
