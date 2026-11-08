# BRIEF — the drift audit: which recorded findings rest on nothing

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-11.

## The class, established by an instance an hour old

`NATIVE_BOUNDS_TRANSFER.md` recorded `9 of 190 pairs (4.7%)`. The tree measured `24 of 351 (6.8%)`.
The conclusion survived; **every number in it had moved and nothing said so**, because the spike that
produced them printed and asserted nothing.

That was found by accident — I ran the spike out of curiosity. **The deliberate version is this:
which other recorded findings rest on measurements that cannot fail?**

Counted: of eleven research spikes in this package, **one asserts nothing at all** and several assert
once or twice across many printing tests. The decision documents cite their figures as settled.

## The first target, and why it outranks the rest

The same document carries a finding it calls *"the one that matters most, and it is not about native
code"*:

> **`wcmu_region` drives its own running depth NEGATIVE on 17 of 826 shipped chunks**, reaching -5 at
> worst. An operand stack cannot hold a negative number of slots. Wherever this happens the walk is
> not tracking the real stack, and the peak taken from that same walk is not an upper bound on
> anything.

**That is a soundness claim about the reference's own worst-case memory bound** — the project's
stated value proposition — and its denominator is already stale: the corpus was 826 chunks then and
is over a thousand now. Whether the numerator moved, nobody knows.

## The wrong turns

1. **Do not "fix" anything in `src/`.** The walk belongs to the other line. This line's job is to
   re-derive the figure, say whether the finding still holds, and report. **Editing their analysis
   would breach the ownership boundary this line checks on every absorption.**
2. **Do not report a changed number as a changed conclusion.** A different count over a different
   corpus is not evidence of a fix or a regression until the populations are compared. The bounds
   transfer taught this: 4.7% to 6.8% looks like deterioration and is not established as one.
3. **Do not pin every printing spike.** A spike is exploratory by design and several have served their
   purpose. Pin the figures that a DECISION DOCUMENT cites as settled — those are the ones a reader
   will rely on without re-running anything.
4. **Do not confuse "asserts nothing" with "worthless".** `spike_composite_cost.rs` has zero
   assertions and its content is an argument, not a measurement. The audit must distinguish a spike
   whose output is prose from one whose output is a number someone quotes.
5. **Do not let the audit be the deliverable.** A list of unpinned figures is a to-do list. At least
   the load-bearing one must end the increment pinned or re-derived.

## What done looks like

The WCMU negative-depth figure is re-derived against the current corpus and recorded with its date,
the earlier figure stays visible as the dated record it is, the finding's status is stated as holding
or not on evidence, whatever pins it is in the tree, and the decision document says which of its
other figures remain unpinned rather than implying all are current.

---

## OUTCOME — 2026-09-11

**The audit's first target was not stale. It was WRONG, and in the more serious direction.**

The brief expected to re-derive `17 of 826` against a larger corpus and find a moved number. What it
found is that **the defect was repaired by the `v0.2.3` line on 2026-08-17**, that
`spike_bounds_transfer.rs` Q4 has measured and ASSERTED zero ever since, and that
`NATIVE_BOUNDS_TRANSFER.md` went on asserting in three places — its summary table, its section 4
heading, and its verdict's *"still reported, not repaired"* — that the reference's own worst-case
memory bound is unsound.

> **A report that is never re-checked becomes a standing accusation.** This one outlived its defect by
> four weeks, in a document a reader would take as settled, about another line's code.

### What was and was not wrong

| claim | status |
|---|---|
| the operand walk goes negative on shipped code | **false since 2026-08-17**, and the guard has said so |
| the WCMU bound does not TRANSFER to native code | **unaffected and still true** — the bound counts virtual-machine operand slots and the native frame counts something else |

Conflating those two would have been the easy error in the other direction: **the repair fixes the
soundness of the bound, not its relevance to native code.**

### The guard was right and the record was wrong, which is the harder case

`spike_bounds_transfer.rs` Q4 carries a comment explaining that it deliberately asserted nothing while
the defect was open — asserting zero would have failed the suite over another line's code, asserting
the then-current count would have failed the moment they repaired it — and that the reason expired
when the repair landed. **The instrument reasoned about its own lifecycle correctly.** Nothing carried
that reasoning back to the document.

### What this says about the audit's premise

The brief assumed the risk was *numbers drifting*. The measured risk is worse and different:
**a conclusion inverting while the paperwork holds still.** A drifted number misleads about magnitude;
an un-retracted defect report misleads about whether something is broken.

### Residual, stated rather than closed

Only the load-bearing figure was re-derived. The document now says, in a table at its head, which of
its figures are guarded and which are as-of-date — so the remaining stale numbers are visible as such
rather than repaired silently or left to look current.
