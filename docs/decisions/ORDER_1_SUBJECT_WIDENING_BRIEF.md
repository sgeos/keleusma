# BRIEF — the residual has a name now: one subject chunk, hardcoded

## The goal

Two increments established what the Order-1 gate actually shows: **ten stages, each agreeing at sixty
tick positions throughout ONE execution.** The stated residual was *one input program and one seeded
segment per stage*.

**That residual is not abstract — it is a hardcoded filename.** `stage_seed` builds every seeded
stage's input from exactly one subject: the largest chunk of `02_struct_field.kel`, with a per-stage
defect injected at op 0. Four stages are seeded (`verify_depth`, `verify_typed`,
`verify_structural`, and `reconstruct`, the last listed only so the harness PRINTS why it is
blocked).

**So the gate's strongest stages each see one chunk of one program.** Widening that is the concrete
next move on the roadmap's own criterion, and nothing else queued touches it.

## Why the tick count does not already cover this

Sixty ticks vary a stage's POSITION WITHIN a run. They do not vary what it is looking at. A stage
that mis-handles a construct absent from `02_struct_field.kel` agrees at all sixty ticks and the gate
reports it as agreeing. **This is the same shape as the hole seed 0 left for `Op::CmpLt`** — measured
there, with `SLE` substituted for `SLT` across 126 sites and the whole differential still passing,
because no vector made two comparands equal.

## What the fix has to be

Drive each seeded stage over **several subjects**, not one, and report how many each actually
received. The seed count for a stream stays 1 — that part is correct and must not be touched. What
varies is the SUBJECT, which is a different axis from the argument vector.

## Prior failures and the specific wrong turns to avoid

- **A WELL-FORMED subject makes the run VACUOUS, and this is measured, not hypothetical.** These
  stages write a verdict of 1 for reject and 0 for accept, and the seeded buffer already holds 0. On
  a clean chunk the stage decides ACCEPT and changes nothing comparable — which is exactly why all
  three sat in `KNOWN_VACUOUS` before a defect was injected. **Every new subject needs its defect
  too, or it adds a vacuous run that inflates a count while comparing nothing.**
- **ONE MUTATION FOR ALL STAGES DOES NOT WORK, and that was measured.** `verify_structural.kel`
  correctly accepts an operand-stack underflow: it latches block-nesting malformation, not depth.
  Reusing one mutation would read as "that stage cannot be made to reject", which is false. **Keep
  the per-stage defect mapping; apply it per subject.**
- **DO NOT widen `seeds` for streams.** The harness pins it to 1 deliberately and says why. Adding
  argument vectors to a stream changes what the run MEANS. This increment varies the SUBJECT.
- **DO NOT silently drop a subject that fails to build a seed.** The existing accessor reports WHY it
  declined rather than returning `None`, because a first attempt used a source that did not even
  parse. Preserve that: a declined subject must be visible and counted separately from one that
  seeded and then ran.
- **A NEW SUBJECT THAT DISAGREES IS A FINDING, NOT A THING TO EXEMPT.** Report it. That is the entire
  point of widening.
- **DO NOT claim the gate is met**, and do not present a subject count as if it were a coverage
  proof. More subjects is more evidence, not a different kind of evidence.
- **Watch the runtime.** Four stages times N subjects times sixty ticks, with `verify_typed`'s seed
  alone at ~449 KB. If it grows unreasonably, use fewer subjects and SAY so rather than quietly
  capping.

## What a good outcome looks like

Each seeded stage is driven over more than one subject; the report states how many subjects each
received and how many declined; a guard fires if a seeded stage silently falls back to one subject or
if a widened run turns vacuous. **And the residual is restated accurately at whatever the new
position is** — widening from one subject to a handful is progress, not closure.
