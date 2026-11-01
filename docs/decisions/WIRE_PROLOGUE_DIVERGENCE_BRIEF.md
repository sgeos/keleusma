# Brief: The Two Chunks Where `wire.kel` Diverges

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Drafted**: 2026-08-27 (session 55)
**Status**: working brief

## The goal

`wire.kel` compiles through the self-hosted pipeline — 486 chunks — and is **not**
byte-identical. Two chunks diverge. Closing them puts the largest stage into the
byte-identity oracle, which is the last structural gap in Order 1's verification story.

## What is measured

| chunk | self-hosted | reference | direction |
|---|---|---|---|
| `emit_prologue` | 40 operations | 59 | **19 fewer** |
| `prologue_disagreed` | 16 operations | 50 | **34 fewer** |

**Fewer operations means a construct is DROPPED, not mistranslated.** That is the single
most useful fact available and it should shape every probe: look for something the stage
declines to emit, not something it emits wrongly.

Both compile **byte-identically when extracted verbatim** into a small program. The
divergence is therefore **context-dependent**, exactly as the previous two `wire.kel`
blockers were.

## What has already been tried and did not reproduce

Recorded so the next attempt does not repeat them. All returned IDENTICAL:

- a bare `for` over a constant literal range, which both functions contain
- the same with a variable bound, and with an explicit `limit`
- a bare `for` whose body holds two statements
- `prologue_disagreed` verbatim with minimal scaffolding
- the chained `bor` reduced to a single `bor`
- chained `bor` with call operands and no loop

**Six probes, all negative.** That is the guessing method, and on this file it has now
failed seventeen times across three blockers.

## The method that has actually worked here

**Prefix bisection over the real file, then an isolating experiment.** It found the previous
blocker in two runs after eleven guesses failed: truncate `wire.kel`, find the exact line
where the verdict flips, then construct a synthetic program that changes only the suspected
variable and confirm the flip outside `wire.kel` entirely.

The bisect predicate here is different and must be stated: not "does it compile" but
**"do these two chunks match the reference"**. A predicate that only checks for a panic will
report every prefix as passing.

## Prior failures this work must not repeat

- **A number in a message is not a cause.** Three occurrences in two increments on this very
  file, twice published as findings and retracted.
- **The reported chunk name is a label until shown otherwise.** It proved accurate last time,
  but only because it was checked against the reference's own chunk table.
- **I derived a family of three that was four, and of seven that was 26.** Derive sets from
  the source and assert the derivation is non-vacuous.
- **A guard that cannot fire is worse than none**, and two written last session could not fire
  as first drafted.
- **Three independent signals over one feature set are still one feature set.**
- **Read cargo's own exit status, from the log, never from a reported process exit code.** A
  run reported "exit code 0" while its log said 101.
- **Watch the cost of the probe itself.** Three runs were killed before I noticed a test was
  compiling four whole programs where two sufficed.

## The specific wrong turns to avoid

1. **Do not bisect the function bodies.** Both compile identically in isolation; the cause is
   not inside them. `the_diverging_functions_compile_identically_in_isolation` exists to stop
   exactly this.
2. **Do not assume the two share one cause.** They differ by 19 and 34 operations and may be
   two defects. Establish whether closing one closes the other.
3. **Do not widen any capacity.** Nothing has been shown too small, and the last two
   `wire.kel` diagnoses that reached for a capacity were both wrong.
4. **Do not put `wire.kel` into the byte-identity corpus until it IS identical.** A pin last
   session instructed exactly that on the strength of "it compiles"; following it would have
   corrupted the oracle. Compiling and being identical are two claims.
5. **Do not claim byte-identity on a partial result.** That false claim was invented once here
   and reached a doc comment, a pull-request body and three channels.
6. **No new opcode. No `BYTECODE_VERSION` change.**

## What a complete increment looks like short of closing the gap

The mechanism named and demonstrated, with the pins updated to whatever the tree then does.
**A named cause with the repair deferred is a complete increment**, and saying so is better
than stretching scope until the gap closes.
