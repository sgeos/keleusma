# Brief — the last type-channel extraction, `expression_nodes_resolvable`

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: brief for the increment. Written 2026-08-28, session 56.

## PRESENT GOALS, AND WHY THIS IS THE ONE LEFT TO ME

| goal | state |
|---|---|
| Order 1 item 3 | **four of five** extractions moved; this is the fifth |
| `verify_types.kel` self-compiling | needs a two-pass parser restructuring — **flagged to the operator as their call, not to be started here** |
| the four-tag op residue | closable only by a program exercising byte arithmetic and unary negation; adding one purely to move the number would be gaming the metric |
| the floating-point entry ABI | the operator's, and the `v0.3.0` line's to bring |

So this is the only substantial item that is both unblocked and mine.

## THE DIFFICULTY IS ORDER, NOT COMPOSITES, AND THAT CORRECTS MY OWN EARLIER SIZING

A previous iteration sized this as "eight node kinds, three of them composite" and declined it on
that basis. **That was the wrong reason.** Reading the reference:

- `visit_expr` pushes an operator node **before** walking its operands, which the code documents.
  The output is therefore a **preorder** list.
- `derived` records `(name, index into that list)` — a POSITIONAL reference — so the list's order
  is load-bearing rather than incidental.

The reconstructed forest is built **postorder**. Reproducing a preorder index from it is possible
but is the actual work, and no amount of care about composites addresses it.

**The occurrences slice escaped this by comparing MULTISETS**, which was legitimate there because
nothing referenced an occurrence by position. **That escape is not available here**, and reaching
for it would silently drop the half of the extraction that `derived` exists to serve.

## WHAT TO DO FIRST, AND IT IS A MEASUREMENT

**Do not start writing a mapping.** Probe whether the forest can reproduce the reference's preorder
positions at all, on the five NON-composite kinds, using a corpus that contains each. Compare the
ordered list, not a multiset.

- If the order reproduces, the increment is real and the composite kinds are the remaining question.
- If it does not, that is the finding, and it should be recorded with a witness rather than worked
  around.

Either outcome is a deliverable. **A recorded blocker with a witness is worth more than a partial
mapping that quietly compares the wrong thing.**

## PRIOR FAILURES THIS INCREMENT MUST NOT REPEAT

**"THE DRIVER DISCARDS X" AND "X IS UNREACHABLE" ARE DIFFERENT CLAIMS.** Paid for twice this
session. Use `parse_record_trace` and read the record stream before concluding anything about what
the host can see.

**A COUNT OF FIVE OF FIVE WOULD READ AS COMPLETENESS.** Every previous slice moved with a residual
and said so. If only part of this moves, **it must not be named `expression_nodes_resolvable_from_
pipeline`**, because the count pin matches on that name and would report Order 1 item 3 finished
when it is not. The declared half of `occurrence_rows` moved under a deliberately different name
for exactly this reason; do the same.

**A GUARD WHOSE CORPUS LACKS THE CONSTRUCT IS A GUARD FOR A DIFFERENT QUESTION.** The struct
agreement test could not see trait and impl being re-admitted because its probes contained neither,
and only a mutation revealed it. Each of the eight kinds needs a probe or the agreement test is
silent about it — and the test should ASSERT its corpus covers each, not assume it.

**MUTATION-TEST EVERYTHING AND CONFIRM THE MUTANT COMPILES.** A harness earlier this session
reported "zero compile errors" for three mutants while running nothing, because a variable was
escaped inside a quoted heredoc. Zero errors from a command that never ran looks exactly like a
clean mutant.

**BEFORE PINNING A VALUE, ASK WHAT ITS WIDEST INPUT IS AND WHETHER THAT INPUT IS PINNED TOO.** The
`v0.3.0` line's formulation, adopted: an invariant protects a REGION, and will not protect an
expectation whose widest input lies outside one. A directory scan is not a corpus.

**DO NOT BOLT A BONUS NET ONTO A REPAIR WITHOUT PROVING IT CAN FIRE.** A subset assertion added
while fixing something else was unfalsifiable by its own precondition, four lines below the check
that guaranteed its conclusion.

## THE WRONG TURNS SPECIFICALLY

- **Do not compare as a multiset** to make the ordering problem go away.
- **Do not start the two-pass parser work**, however tempting it looks from here.
- **Do not add an opcode; do not bump `BYTECODE_VERSION`.**
- **Do not gate a source-text guard behind an off-by-default feature.**
- **Account for every test binary** rather than sampling: a killed sweep this session reported 55
  green while 31 never ran, including the guard that then caught a real drift.

---

## OUTCOME, WRITTEN AFTER THE PROBE

**The brief's instruction was followed and it was the right one: probe the ordering question before
writing any mapping.** The probe answered it, and the answer corrects the brief's own framing for
the second time.

**The obstacle is not preorder-versus-postorder. It is that the forest's children are not all in
`lhs` and `rhs`.** A preorder walk over those two fields, measured:

| program | reference sees | the walk saw |
|---|---|---|
| `g() + g() * 2` | two calls | **one** |
| `f(1)` | one call | **hundreds**, until a guard stopped it |

It misses children AND revisits nodes, in the same probe. A call's arguments live in `call_args`;
the loop, match, limit and multihead constructs each keep their parts in their own side-table.

**What landed is the measurement plus a guard**, not the mapping: `tests/forest_child_channels.rs`
pins that the forest has exactly six child-bearing channels, so a walk written against them is
complete by construction and a seventh cannot appear silently. The probe function itself was
deliberately NOT shipped.

**A caution recorded for whoever writes the walk**: `codegen.kel`'s visit order is EMISSION order
for a stack machine, not the reference's syntax-tree preorder. Copying its child sequence would
give a consistent traversal that is still the wrong order for this comparison.

**The count stays at four of five.** Nothing was named `expression_nodes_resolvable_from_pipeline`,
so the pin cannot report Order 1 item 3 finished.
