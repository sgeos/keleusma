# BRIEF — two unrelated tens, printed adjacent, and the roadmap's own gate is the casualty

## The finding

`corpus_differential.rs` prints the Order-1 gate report, closes it with a `================`
terminator, and then prints:

> `of those, 10 were driven at ONE argument vector and could not have varied`

**"Of those" does not refer to the Order-1 stages.** It continues the corpus-wide observable
breakdown opened ~85 lines earlier at `of these 58, agreement is carried by:`. The Order-1 block is
NESTED INSIDE that breakdown and closes before its parent resumes, so the parent's last line is
orphaned past a section terminator belonging to a different report.

**Both numbers are 10.** The Order-1 gate reports `EXECUTE and AGREE : 10`. The orphan reports `10`
undrivable modules out of 58. **They are unrelated, and the coincidence makes the misreading close to
forced.** A reader concludes the gate's ten agreeing stages were all single-vector — a claim nothing
in this tree has measured.

**I misread it myself on first pass**, which is the strongest evidence available that the layout is
the defect rather than the reader.

## Why this one matters more than its size suggests

The Order-1 gate is **the roadmap's own criterion** — *"the self-hosted compiler's own bytecode runs
correctly as native code"*. It is the single most quotable figure this line produces, it is reported
prominently in the resume document, and this line has already declined once to declare it met. A
qualification that appears to weaken it, but actually measures something else, corrupts the most
load-bearing number here in **both** directions: it makes the gate look weaker than measured, while
leaving the real question — *how many vectors did the stages actually get?* — unasked and unanswered.

## What the fix has to be

1. **The orphan must name its own population** rather than relying on "of those" and on proximity.
2. **The real question must be answered**: how many argument vectors did each Order-1 stage receive?
   Nothing reports this today. Answer it and print it inside the gate block.
3. **Both must be guarded**, because a layout coincidence that reappears after a refactor would be
   invisible again. Prose does not fail; this file's own `witness_integrity` lesson applies.

## Prior failures and the specific wrong turns to avoid

- **DO NOT round the stage figure up.** If the stages turn out to be single-vector, that is the
  answer and it must be stated plainly, not softened. This line has recorded an inflation before —
  *half the agreeing count agreed on a single value* — and the whole point of measuring is to be able
  to say so.
- **DO NOT move the Order-1 block out of the observable loop to "fix the nesting".** It reads
  `executed` and its own stage classification; relocating it to tidy the output risks changing what
  it counts. **Fix the SENTENCE, not the control flow**, unless the control flow is what is wrong.
- **A guard on "the two numbers must differ" is WRONG.** They may legitimately coincide again. The
  guard belongs on the *labelling* — that each figure names its population — not on the values.
- **DO NOT assert the stage vector count as a pinned constant.** It moves with the corpus and with
  `SEEDS`. Assert the property that makes the figure meaningful (that it was derived from the runs
  actually performed, over a non-empty stage set), and REPORT the number.
- **Check the resume document too.** It quotes "Order-1 gate 10 of 12" prominently. If it has
  absorbed the misreading anywhere, that is a second instance of the same defect, not a separate one.
- **DO NOT claim the gate is now met.** This work characterises the gate's strength. Whether 10 of 12
  with N vectors each clears the roadmap's bar is not this increment's call.

## What a good outcome looks like

The gate block reports, from the runs actually performed, how many argument vectors its agreeing
stages received. The orphaned line names its own population and can no longer be read as qualifying
the gate. A guard fires if either figure is printed without its population, or if the stage-vector
figure is derived from an empty set. **And the answer to "how strong is the Order-1 gate really?" is
a number in the tree rather than an inference from an adjacent report.**
