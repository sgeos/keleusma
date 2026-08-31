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
