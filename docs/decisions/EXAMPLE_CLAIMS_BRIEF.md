# Brief — the example index makes a claim its file contradicts

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: brief for the increment. Written 2026-08-28, session 56.

## THE DEFECT, AND HOW THE OP-TAG CENSUS LED TO IT

`examples/scripts/README.md` describes `10_multbyte.kel` as **"Byte-typed arithmetic —
Multiplication on `Byte` operands"**.

**The file contains no `Byte` at all.** Its own header says: *Multi-word ("Multbyte") arithmetic ...
A Multbyte value is a fixed-length array of `Word` digits in little-endian order.* The name means
multi-WORD; the index read it as the `Byte` TYPE.

**Zero `Byte` across all fifteen shipped examples**, verified two independent ways — a word-boundary
pattern and a fixed-string count, because a `\b` escape had silently failed elsewhere in this
session and the whole finding rests on this count.

**THE ROUTE IN IS WORTH RECORDING.** The op-tag census reported `addop`, `subop` and `mulop`
unreached by any corpus. A reader consulting the index would find that surprising, because the index
says byte multiplication is demonstrated. **The census was measuring the tree; the index was
describing something else.** A coverage number disagreeing with the documentation is a signal about
one of them, and here it was the documentation.

## PROTOTYPED BEFORE COMMITTING, AND IT IS THREE VIOLATIONS ACROSS TWO ROWS, NOT ONE

The guard was written outside the tree first and run against the current index. **Twelve checkable
claims, three violations, nine correct rows passing** — so it is neither too loose nor too tight,
and both directions were checked rather than one.

| row | claims | the file contains |
|---|---|---|
| `10_multbyte.kel` | "Byte-typed arithmetic", "Multiplication on `Byte` operands" | **no `Byte` at all**; its own header says multi-WORD digits |
| `01_arithmetic.kel` | `Word`, **`Float`**, **`bool`**, arithmetic, comparison, casts | sixteen lines using only `Word`: multiply, add, subtract |

**A CHECKABLE CLAIM IS A BACKTICKED TYPE OR KEYWORD**, not prose. Extending from types alone to
keywords took the check count from four to twelve and found no further violations, which is worth
recording: the other nine claims — `for`, `match`, `signed`, `private data`, `loop main`,
`newtype` — are all honest.

**THE TWO ROWS ARE NOT THE SAME KIND OF WRONG, AND THE FIX DIFFERS.** For `10_multbyte.kel` the
file's own header CONTRADICTS the index, so the index is simply wrong. For `01_arithmetic.kel` the
index's Topic agrees with the file's header and only its FEATURE column overstates — which raises a
second reading: the example may be under-delivering on its own stated topic rather than the index
overstating it.

**Correcting the feature column is the defect fix and is in scope. Enriching the example is not**,
for the same reason the `Byte` example is not: it is a design call about a curated progression.

## WHAT TO DO, AND WHAT TO LEAVE TO THE OPERATOR

**IN SCOPE: correct the false row, and add the guard that would have caught it.** A row naming a
type keyword should have that keyword in the file it names. That is derivable from the index and the
files, non-vacuous, and it fails today on exactly one row.

**NOT IN SCOPE WITHOUT THE OPERATOR: adding a sixteenth example that teaches `Byte`.** The set is
curated and numbered `01`–`15` as a progression, so where `Byte` belongs in it is a design question
about the project's documentation rather than a defect fix. **Put it to the operator with the
evidence.** It would also close three of the four residual op tags, and that is precisely why it
must not be decided on those grounds — closing a coverage number is not a reason to add user-facing
documentation.

## PRIOR FAILURES THIS INCREMENT MUST NOT REPEAT

**A GUARD MUST BE ABLE TO FIRE, AND ITS CONCLUSION MUST NOT FOLLOW FROM ITS OWN PRECONDITION.** A
subset assertion earlier this session was unfalsifiable four lines below the check that guaranteed
it. Construct the failing input first: this guard has one, today, in the row being corrected.

**BEFORE PINNING A VALUE, ASK WHAT ITS WIDEST INPUT IS AND WHETHER THAT INPUT IS PINNED TOO.** The
`v0.3.0` line's rule. This guard reads BOTH the index and the example files, so both are its input —
it must not pin a count that a new example would move, and it must fail loudly if a row names a file
that is absent.

**DO NOT LET THE GUARD ENCODE THE ANSWER.** Checking "the row for `10_multbyte.kel` says multi-word"
would confirm the fix and nothing else. The guard must be over ALL rows, derived, so a future row
with the same defect is caught without anyone thinking of it.

**A MUTATION HAS THREE FAILURE POINTS.** Confirm it APPLIED, then that it COMPILED, then believe the
result — and print what changed. This session was bitten at all three, most recently by BSD `sed`
having no `\b`.

**ACCOUNT FOR EVERY TEST BINARY** rather than sampling, and read cargo's own exit status rather than
a pipeline's.

## THE WRONG TURNS SPECIFICALLY

- **Do not rewrite the example's own header.** It is correct; the index is wrong.
- **Do not add the sixteenth example unilaterally.**
- **Do not make the guard demand a keyword for every row** — most rows name features, not types, and
  a guard that insists every row mention a literal keyword would fail on correct rows. Scope it to
  rows that name a TYPE the language has, and assert the scoping is non-vacuous.
- **Do not gate a source-text guard behind an off-by-default feature.**
