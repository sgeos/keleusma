# Brief: the oracle's type-surface census

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: brief, self-directed. Not an operator ruling.

## The measurement that motivates it

The lexical census established that the twelve-stage byte-identity corpus contains zero string
literals. Asking the obvious next question -- what ELSE is at zero -- produced a much stronger
result, measured over all twelve stage sources with comments stripped:

| construct | occurrences in the corpus |
|---|---|
| declared return types | **861, and every one is `Word`** |
| declared parameter types | **every one is `Word`** |
| string literal | 0 |
| `Text` | 0 |
| `Float` | 0 |
| `Fixed` | 0 |
| `bool` at a function boundary | 0 |
| tuple at a function boundary | 0 |
| composite (struct or enum) at a function boundary | 0 |
| `impl` block | 0 |
| `while` | 0 |

**The byte-identity oracle exercises exactly one type at every function boundary.** Composites,
arrays and `Byte` appear inside bodies, through `private data` and indexing, but never cross a
signature.

That is the general form of the string-literal gap. The oracle is the project's primary correctness
signal for the self-hosted compiler, and its reach is a property of what the corpus happens to
contain rather than of the pipeline.

**A first draft of this brief said the boundary table is the ONLY coverage for non-`Word` types.
That was too strong, and testing the claim refuted it.** Fourteen test files drive the self-hosted
compiler. Counting lines that declare a non-`Word` parameter or return in a source snippet:
`selfhost_codegen.rs` has 299 and `selfhost_pool_tags.rs` has 22; the other twelve have between zero
and two. So non-`Word` coverage is real and not confined to the boundary table.

The accurate statement is narrower and still worth having:

> The byte-identity oracle over REAL programs is `Word`-only. Every non-`Word` signature the
> self-hosted pipeline is tested against is a hand-written snippet, typically one to three lines.

The distinction that matters is not synthetic-versus-absent, it is synthetic-versus-scale. A
200-kilobyte stage exercises interactions between constructs that a three-line snippet cannot
reach, and it is exactly those interactions the byte-identity oracle was built to catch. For
`Word` the oracle has both; for every other type it has only the snippets.

## And the boundary table's own distribution is lopsided

Counted by family over its 101 cases: **43 equality cases**, then 11 scalar, 10 bool, 8 op, 8 comp,
5 prec, 5 ctrl, 3 nested, 3 cast, 2 scope, and **exactly one each for `literal`, `tuple`, and
`removed`**.

The single `literal` case is `let s = "hi"`. That is the degenerate case which let both string
defects through: the family had a case, so it looked covered. The single `tuple` case is one nested
element access.

**A distribution like this measures where attention has been, not where risk is.** It is not
evidence of anything being wrong; it is evidence about what the table can find.

## The goal

Make the oracle's type surface a measured, checked figure rather than an unexamined property of the
corpus, and state plainly which families are covered only by a single synthetic case.

## Prior failures to avoid

**Do not "fix" this by adding types to the stage sources.** The stages are a real program with a
purpose; contorting them to broaden coverage would corrupt the oracle's subject to flatter its
reach. The remedy is to MEASURE and record, and to widen the synthetic case families where that is
cheap.

**A zero is a strong claim and my instruments were wrong four times today.** Every zero here was
confirmed by two independent routes, and one first attempt was wrong in each direction: a
`grep -F '\['` searched for a literal backslash, and a character-scanning guard was fooled by quotes
inside a comment. **Derive from the real reader.** The lexer and parser are available; a regular
expression over source text is a choice to have an instrument that can be wrong.

**A count that is not asserted goes stale silently.** The figures above must live in a test that
recomputes them, not in prose. `tests/corpus_pattern_coverage.rs` is the precedent: it measures the
example corpus and pins the shape, and its header states plainly what the measurement does not
establish.

**Do not overclaim what a broad type surface would buy.** A corpus exercising more types would not
prove the compiler correct; it would remove one specific blind spot. Say that.

## What "done" is not

It is not a longer probe list. It is a figure the tree recomputes, a record of which families rest
on a single case, and no claim that coverage has been achieved where it has only been measured.
