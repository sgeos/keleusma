# BRIEF — what actually blocks the last 2 chunks and 86 opcode instances

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Why this is a measurement rather than a feature

Backend coverage sits at **1072 of 1074 chunks and 89854 of 89940 opcode instances**, and has for
several increments. **Nobody on this line has read what the remaining 86 instances ARE.** The census
already computes a breakdown by workstream and prints it, and the figure has been quoted repeatedly
without the breakdown beside it.

**Choosing the next target without reading it would be the mistake this line has made six times** —
reasoning about a boundary from the component nearest to hand. The last two increments both got
their shape from a measurement taken first, and both were the smoothest on the line.

## What good looks like

The residual is **named**: which opcodes, in which chunks, of which modules, grouped by whatever the
census calls a workstream. Whether the 2 chunks and the 86 instances are the same cause or two
different ones, stated rather than assumed — they are separate counts and nothing has checked that
they line up.

And a recommendation follows FROM the numbers: whether the residual is one cheap gap, a genuine
workstream, or a refusal that should stay.

## Prior failures to avoid repeating

- **Do not parse the census's source text to answer this.** The data is reachable AS DATA through the
  same API the census uses. Parsing source when the data is available is choosing an instrument that
  can be wrong, and this package has already had four instrument errors, one of exactly that shape.
- **Do not conflate populations.** This line has quoted 91 modules where there were 67, and 239
  composite sites beside 256. Every figure recorded here states the population it is over.
- **Do not read a single number as a cause.** "86 instances" could be one opcode in one hot chunk or
  eighty-six scattered ones, and the two imply completely different next moves.
- **Do not append a filter to a command whose status you intend to read.** That trap fired again
  today, for the fifth recorded time, in the command written to summarise a green suite.
- **A clean census is evidence about the census's reach before it is evidence about the tree.**
  If the breakdown reports nothing, establish that it CAN report something before believing it.

## The wrong turn most likely here

**Turning a measurement into an implementation halfway through.** If the residual turns out to be
cheap, the temptation is to fix it in the same breath and report a coverage gain. The measurement is
the deliverable; a fix is a separate decision that a recorded measurement makes cheap to take later.

## Outcome, written after the measurement

**The residual is two chunks, and the two published figures are ONE finding rather than two.**

| | |
|---|---|
| population | **69 modules, 1074 chunks** |
| refused chunks | **2** |
| | `13_telemetry_stream.kel::main` — 45 opcodes — refused for `Stream` |
| | `refused_witness.kel::len_witness` — 41 opcodes — refused for `Len` |
| blocking opcode instances | **86 = 45 + 41, exactly** |

**86 IS THE SUM OF THOSE TWO CHUNKS' LENGTHS.** An opcode instance counts as blocking when it merely
SITS IN a refused chunk, so the instance figure is the refused chunks' combined size and carries no
information the chunk count does not. Confirmed by reading the instrument and then pinned as a
measured identity by `the_blocking_instances_are_exactly_the_contents_of_the_refused_chunks`, with
non-vacuity in both directions.

### The report was readable as a work queue, and it named the wrong work

Three tables were headed as causes — *blocking opcode instances by workstream*, *chunks whose first
blocker is*, *top blocking opcodes by instance count*. The last put `GetLocal` at 18, `Const` at 17
and `SetLocal` at 16 **at the head of what reads like a list of things to implement.** All three
already lower. They top the table because they are the commonest opcodes in any chunk, and these two
chunks are ordinary apart from one opcode each.

Renamed rather than deleted, since the composition is worth seeing once it cannot be mistaken for a
diagnosis. **This is the same class as the four instrument errors already recorded: a signal
answering a narrower question than its label claims.**

### The recommendation, which follows from the numbers rather than from prior belief

**Coverage is saturated under the current design, and further gain is not a matter of effort.**

- `Stream` is refused deliberately, and the refusal is **load-bearing**: the handoff records that the
  region planner's cross-iteration slot reuse is unsound for a composite escaping by `yield`, and
  that the only thing keeping it quiet is that every chunk carrying the shape opens with `Stream`.
  Lowering `Stream` to gain 0.09% of instances would **retire an accidental safety** whose
  replacement needs the planner to consume a confinement verdict — a design decision with a stated
  cost, and the operator's to take.
- `Len` was re-checked against `for .. limit` and holds.

**So the honest next move is NOT to chase the last 0.1%.** Two figures quoted for several increments
turn out to describe two deliberate refusals, one of which should not move without a decision. That
is a better outcome than a coverage gain, because it removes a target that looked available and was
not.

### And the guard's first draft was vacuous, which clippy caught

The reconciliation was first written with both sides computed as `c.ops.len()`, making the assertion
`x == x`. **Clippy's `iter_count` lint is what surfaced it**, which is a lint about style catching a
defect about meaning. The identity really is definitional, so the guard was rewritten to earn its
place differently: the two sides are now computed by different traversals, so it fires if the
report's rule ever changes to count only the CAUSING opcode — the change after which the tables'
labels would need revisiting again.

**The rule this line already carries applied to me**: a passing check is evidence about the checker
before it is evidence about the tree.
