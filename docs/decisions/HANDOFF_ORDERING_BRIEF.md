# BRIEF — the resume document's own reading rule inverts, and four headings are false

## What is wrong

`docs/process/handoffs/v0.3.0.md` is **3906 lines, 253 KB, 76 top-level sections** and is the
primary resume deliverable of this line. Two defects, and the second is the serious one.

**1. Four headings state facts that later sections correct.** A reader skimming headings — which is
what a 76-section document invites — collects wrong figures:

| heading says | actually |
|---|---|
| THE BACKEND LOWERS **56 OF 66** | **60 of 66** |
| THE ARITHMETIC CLUSTER LOWERS, AND THE CORPUS DOES NOT EXERCISE IT | it does now |
| THE MIS-COMPILATION CLASS IS **THREE SITES** | **seven** |
| `Op::IsStruct` IS **NOT** PRODUCERLESS | the `v0.2.3` fix closed every known producer |

**2. The ordering convention is REAL, LOAD-BEARING, AND NEVER STATED.**

**I got this wrong twice while diagnosing it, and both corrections are the finding.** I first read a
rule at line 3315 — *"the later ones win"* — as the document's own; it governs a DIFFERENT file. I
then concluded the document has no convention at all. **It has one, in active use**: earlier sessions
marked a stale heading `## SUPERSEDED: …` and pointed at corrections as *"above"* and *"at the top"*,
which establishes NEWEST-AT-TOP.

**So the defect is mine, not the document's.** The convention existed, was followed by previous
sessions, and I added four corrections this session WITHOUT marking what they corrected. A reader
skimming headings meets my stale claim with nothing saying it is stale.

## What the fix has to be

**State the convention where a reader meets it**, since it is currently inferable only from the
wording of old supersession notes — and follow it for the four.

## Prior failures and specific wrong turns to avoid

- **Do not delete the superseded sections.** They record how a wrong conclusion was reached, which is
  the most reused content in this file. Mark them; do not prune them.
- **Do not renumber or reorder.** A reordering makes every prior reference to "the section above"
  wrong, and there are many.
- **Do not merely fix the four.** The mechanism produces more every increment. Fix the ORDERING RULE
  so a future stale heading is survivable, then fix the four.
- **A heading is what gets skimmed.** Marking the body while leaving the heading assertive fixes
  nothing for the reader who never opens the section.
- **Do not claim the document is now navigable.** 3906 lines is still a lot; this makes it
  non-misleading, not short.

## What a good outcome looks like

The ordering rule states **absolutely** which end is authoritative, every heading whose claim has
been corrected says so in the heading itself and names where the correction lives, and the superseded
reasoning is retained.

**Stating the convention is the load-bearing half.** Marking four headings fixes four facts; writing
the rule down means the next session inherits it instead of re-deriving it from old notes — which is
what I failed to do.
