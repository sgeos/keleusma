# Brief — CONDITION and BRANCH_PAIR from the pipeline

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: brief for the increment. Written 2026-08-28, session 56.

## WHERE THIS SITS

Kind 1 (the binary operator) is fully moved, across all four forest kinds the lowering splits it
into. **Seven of the eight remain**: array elements, conditions, branch pairs, field and index
access on a value, struct literals, and the tail-versus-return claim.

Two are now known recoverable: `push_if` in `codegen.kel` shows the If node keeps its condition in
`args` and its two branches in `lhs` and `rhs`. So **CONDITION and BRANCH_PAIR are the next slice**,
and they share one node.

## THE WRINKLE, FOUND BEFORE HITTING IT

The reference does not take the branch EXPRESSIONS. It takes `then_block.tail_expr` and
`else_block.tail_expr` — **the tails**. For a single-expression branch the forest's `lhs`/`rhs`
point straight at that expression and the two agree. **For a branch with statements they do not**:
the forest's child is the head of a `LetIn` chain whose continuation eventually reaches the tail,
and classifying that chain head as an operand yields "unknown" where the reference yields the tail's
form.

**So the slice needs to follow a `LetIn` chain to its continuation before classifying.** That is a
small walk, not a large one, and it is bounded by the chain length.

**AND THE CORPUS MUST CONTAIN BOTH SHAPES** — a bare-expression branch and a branch with a `let`
before its tail — or the test is silent about exactly the case that motivated this paragraph.

## PRIOR FAILURES THIS INCREMENT MUST NOT REPEAT

**A GUARD WHOSE CORPUS LACKS THE CONSTRUCT IS A GUARD FOR A DIFFERENT QUESTION.** Recorded FOUR
times now, most recently within this very slice family: the kind-1 extraction shipped covering only
`Word` operands while its corpus was all-`Word`, and the test passed while silent about three of the
four forest kinds. **Assert the corpus covers each shape rather than assuming it**, as that test now
does for operand width.

**ONLY WHAT MOVES IS CLAIMED.** If BRANCH_PAIR moves and CONDITION does not, say so. The count pin
must not be given a name that lets a partial migration read as complete.

**COMPARE THE RELATION, NOT THE INDEX.** Established for this family: the stage reads the expression
table only through `btag[b]` and a per-row predicate, so order is free and an index-for-index
comparison would constrain more than the thing it checks.

**A MUTATION HAS THREE FAILURE POINTS**: confirm it APPLIED, then that it COMPILED, then believe the
result, and PRINT what changed. BSD `sed` has no `\b` here.

**CHECK THE INSTRUMENT BEFORE THE FINDING.** Five times this session an extractor was wrong rather
than the tree, once inside an audit.

**AN ABSENT SIGNAL LOOKS LIKE A BENIGN ONE.** A killed sweep reported 55 green with 31 never run; a
monitor's stream ended at 20 of 22 without saying so; two waiters were killed silently. **Read the
run's own `conclusion`, never a wrapper's silence.**

## THE WRONG TURNS SPECIFICALLY

- **Do not classify a `LetIn` chain head as the branch value.** Follow it to the continuation.
- **Do not emit a BRANCH_PAIR row when there is no else branch** — the reference does not.
- **Do not start the two-pass parser work**; that is the operator's call.
- **Do not add an opcode; do not bump `BYTECODE_VERSION`.**
- **Do not gate a source-text guard behind an off-by-default feature.**
