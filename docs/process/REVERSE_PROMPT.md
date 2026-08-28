## The op-tag residue is four, not sixteen

Earlier in the session I reported sixteen op tags the byte-identity corpus cannot check, and said
the per-construct tests were a different population I had not measured. I measured a second one —
the fifteen shipped examples — and **it covers twelve of the sixteen**, the whole composite family.

Four remain unreached by either corpus: the unchecked arithmetic that `Byte` operands take, plus
unary negation. The description is checked by probes inside the test rather than asserted, because
this project has called an unwitnessed opcode unreachable before and been wrong.

# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-28 (session 56 CLOSE) — six merges, Order 1 item 3 at FOUR of five, and the
twelfth stage's silence explained

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I have
not acted on it.** Their own record now says the recommendation *splits* on a question you have not
answered — whether the fixed-point format must interoperate across object files from different
languages. Publication remains held.

## Five increments merged, each at 22 of 22

`origin/v0.2.3` at `93e66b24`, **162 merges**, **no open pull request**. Publication remains held.

| | |
|---|---|
| #308 | the op-tag tables agree, and something now checks that they do |
| #309 | `field_sets` reaches the type channel — Order 1 item 3 at **three of five** |
| #310 | the declared names reach it too, and the wildcard-import gap is located |
| #311 | the twelfth stage does not self-compile, and the tree now says why |
| #312 | a second corpus narrows the unexercised op tags from sixteen to four |
| #313 | the name occurrences move — Order 1 item 3 at **four of five** |

## Nothing is waiting on you except the ruling you already have

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I have
not acted on it.** Their record says the recommendation now splits on a question you have not
answered: whether the fixed-point format must interoperate across object files from different
languages.

## The one mistake I made three times

**I reasoned from a component's internals about what crosses its boundary.** Twice I read the
parser's data structures, concluded the host could not see something, and sized a large increment —
and the record stream already carried it, so both slices needed no stage change at all. Once I
inspected a function's constructs to explain a refusal and named three plausible culprits;
declaration order was the cause and none of the three mentioned it.

I measured before acting on the second and third occasions, which is the only reason they cost
nothing. Both handoffs now carry the rule and name the two instruments.

## The decision I want visible rather than taken quietly

**`verify_types.kel`, the twelfth stage, does not self-compile.** A function reads a `data` block
declared later in the file, and the parser builds its field table as it meets each block, so the
reference resolves to nothing. Four-line witness, with a control differing only in declaration
order.

**I did not attempt the repair.** It means collecting data declarations before parsing bodies — a
two-pass restructuring of a single-pass streaming parser, not a defect fix. What landed converts an
unexplained absence into a documented, reproducible gap whose pins fire when it closes. If you want
the corpus at twelve, that is the next large item and it is your call whether it is worth the
restructuring.

## Two things I corrected in my own work

A guard I wrote earlier in the session compared arm **spellings** where its own message described
which **codes** were handled; splitting a range made that visible and it now compares coverage.

And a mutation harness reported "zero compile errors" for three mutants while running **nothing** —
a shell variable escaped inside a quoted heredoc. Zero errors from a command that never ran looks
exactly like a clean mutant. Re-run properly, two of three fired.

## On "four of five", because the number would flatter the state

**Moved means an analogue exists, not that nothing is left.** The count pin's own documentation now
carries a table of the residual in each of the four, so the figure cannot be read as completeness:
`decl_call_rows` left its actual-argument tag, `field_sets` its field accesses, and
`occurrence_rows` two shapes.

Those two are different in kind and the difference is the useful part. A `data` block identifier is
**representational** — the pipeline has no ident node there at all, so nothing is missing from the
wire. A `for` loop variable is **a wire gap**: the read reaches the forest but nothing binds its
slot to its name, because only `let` bindings emit a name record. The occurrence is dropped rather
than reported under a wrong name, which is the better of the two failures. Closing it is the same
shape of change as the record that already exists, and that one needed your ruling.

## What I would take up next

The last extraction, `expression_nodes_resolvable`; or the two-pass data resolution, which would
take the byte-identity corpus to twelve and is the largest single item I can see — **that one I
flagged as yours to call rather than start unilaterally.**
