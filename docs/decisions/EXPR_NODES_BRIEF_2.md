# Brief — the last extraction, resized by reading the CONSUMER

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: brief for the increment. Written 2026-08-28, session 56. **Supersedes the sizing in
`EXPR_NODES_BRIEF.md`, which was wrong for the third time.**

## I MIS-SIZED THIS THREE TIMES BY READING PRODUCERS

| attempt | what I read | what I concluded | verdict |
|---|---|---|---|
| first | the reference extraction's line count | "eight node kinds, three composite" | wrong about the obstacle |
| second | the reference's visitor discipline | "preorder against postorder" | wrong about the obstacle |
| third | the forest's child channels | "a walk over six side-tables, and `codegen`'s order is the wrong order" | true, and **not required** |

**The answer was in the CONSUMER, and it took twenty minutes.** `verify_types.kel` reads the
expression table in exactly two places:

- `tyb_node_tag(e)`, indexed by `ty.btag[b]` for a binding whose form is 2 (derived);
- `tyc_row_rejects` at `p == 4`, an **independent per-row predicate** examining row `i` in isolation.

Nothing sweeps the table in order. Nothing else indexes it. **The stage does not depend on the
table's ORDER at all.** It needs each row internally coherent, and `btag[b]` pointing at that
binding's row.

**So any consistent numbering works.** The pipeline may emit rows in whatever order its own walk
produces. The order-reproducing walk sized in the previous brief is not required.

## THE GENERALISATION, WHICH IS THE MIRROR OF ONE THIS SESSION ALREADY PAID FOR

Earlier: *read the RECORD STREAM, not the producer's internals* — twice, when the parser's data
structures said a slice was large and the wire already carried the answer.

Now: **read what CONSUMES the data, not what produces it.** Both are the same rule — **the
requirement lives at the boundary, not in either implementation.** I read a producer three times and
was wrong three times; the consumer answered it immediately.

## WHAT THE WORK ACTUALLY IS

Emit, from the pipeline, rows of `(kind, a, af, b, bf)` and, for each derived binding, the index of
its row. Both channels are the pipeline's own numbering and need agree only with each other.

**COMPARE THE RELATION, NOT THE INDEX.** The natural test is: for each binding the reference calls
derived, does the pipeline associate it with a row of the same CONTENT? That is order-free and tests
what the stage actually consumes. **A test comparing index-for-index against the reference would be
testing a coincidence of traversal order that the stage does not require** — the definition of a
check that constrains more than the thing it checks.

## PRIOR FAILURES THIS INCREMENT MUST NOT REPEAT

**DO NOT REPRODUCE THE REFERENCE'S ORDER.** It is not a requirement, and building it would be work
spent satisfying a test rather than the stage.

**A COUNT OF FIVE OF FIVE WOULD READ AS COMPLETENESS.** Every previous slice moved with a residual
and said so. If only part moves, do not give it the name the count pin matches.

**EACH OF THE EIGHT KINDS NEEDS A PROBE OR THE TEST IS SILENT ABOUT IT**, and the test should ASSERT
its corpus covers each rather than assume it. The three composite kinds are where the
representations most plausibly diverge — the occurrences slice showed the two sides disagree about
what an occurrence IS for a composite.

**A MUTATION HAS THREE FAILURE POINTS**: confirm it APPLIED, then that it COMPILED, then believe the
result, and PRINT what changed. BSD `sed` has no `\b` on this machine.

**BEFORE PINNING A VALUE, ASK WHAT ITS WIDEST INPUT IS AND WHETHER THAT INPUT IS PINNED TOO.**

**CHECK THE INSTRUMENT BEFORE THE FINDING.** Five times this session an extractor was wrong rather
than the tree, once inside an audit.

## THE WRONG TURNS SPECIFICALLY

- **Do not start the two-pass parser work** — that is the operator's call and is recorded as such.
- **Do not add an opcode; do not bump `BYTECODE_VERSION`.**
- **Do not gate a source-text guard behind an off-by-default feature.**
- **Account for every test binary** rather than sampling.
