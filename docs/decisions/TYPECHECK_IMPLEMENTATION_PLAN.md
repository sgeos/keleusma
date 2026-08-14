# Self-Hosting Type Rejection — Implementation Plan

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

The sliced implementation plan the scoping document
([`TYPECHECK_SELFHOST_PLAN.md`](./TYPECHECK_SELFHOST_PLAN.md)) deferred until its two spikes had
run. Both have. Written 2026-08-14.

Status: **PLANNED, NOT BUILT.**

## What this is not

**It is not a port of `src/typecheck.rs`.** That file is 8,601 lines, of which a large share serves
traits and bounds the Order-1 subset does not use. Two measured results shrink the obligation to
something much smaller, and neither is a guess:

- **Monomorphization is an identity transform** on all ten stage sources, pinned by
  `tests/selfhost_monomorphize_identity.rs`. Nothing to port.
- **Clearing `program.fn_expr_types` leaves every stage module byte-identical**, so the emitter's
  structural fallback covers everything the subset reaches. **Inference for emission is not
  required.**

What is left is **rejection alone**: deciding that an ill-typed program is ill-typed. The checker
needs exactly as much type propagation as that decision takes, and no more.

## The oracle, and the one thing it is not

**Verdict agreement. Accept versus reject.** Not message agreement. This is the oracle the
`verify_*.kel` family already uses, and it avoids committing a `.kel` checker to reproducing
English diagnostics that the reference is free to reword.

**The direction matters and is not symmetric with the verifier's.** `verify_*.kel` is allowed a
sound over-approximation, deferring to a runtime guard when it cannot decide. A type checker has no
such latitude in the accepting direction: **rejecting a valid program is a language change**, not a
conservative choice. So the two directions carry different obligations:

| direction | obligation |
|---|---|
| the reference accepts | the stage **must** accept. A false rejection narrows the language. |
| the reference rejects | the stage must reject, for the enumerated shapes. |

The second is bounded by the corpus; the first is not, which is why the well-typed side of the
corpus grows with every slice rather than staying at the five controls the spike used.

## The trap that governs the corpus

**A corpus of rejections alone cannot detect a checker that rejects everything.** It would score
perfectly. Every slice therefore adds well-typed controls alongside its rejections, and a slice
whose well-typed count does not grow is a slice that has not been checked.

The scoping document records the converse mistake, made while sizing this: a case labelled
ill-typed that was in fact well-typed, reported as "accepted but should not be". It did not mislead
**only because explicit well-typed controls existed to check it against**.

## The fifteen shapes, grouped into slices

Measured by execution, not by counting `TypeError` sites: eighteen ill-typed subset programs, of
which seventeen were rejected, plus five well-typed controls, all accepted.

### Slice 0 — the harness, before any rule

A driver that runs a `.kel` checker over `parse.kel` records and compares accept-versus-reject
against the reference, over the whole corpus at once.

**The `.kel` checker accepts everything at this slice.** That is the point: the corpus must then
show every well-typed case passing and every ill-typed case FAILING. A harness that reports success
here is broken, and finding that out costs nothing before any rule exists. This is the must-fire
for the harness itself.

### Slice 1 — scalar operand agreement

`1 + true`; array elements of differing types. The smallest real rule, and the one that establishes
how a type is represented in the stage at all.

### Slice 2 — function signatures

Argument count, argument types, `Byte` against a `Word` parameter, and body type against declared
return type. Four shapes, one mechanism: a signature table and a comparison.

### Slice 3 — identifiers and callability

Undefined function, undefined identifier, and **calling a local**.

> **The odd one out, and it must not be handled by looking for a type error.** "Calling a local" is
> a V0.2.0 surface restriction rather than a type error, and it is the one rejection of the fifteen
> that does not carry the `type error:` prefix. A stage that located rejections by that prefix would
> miss it, and a stage that reproduced the reference's routing would be reproducing English.

### Slice 4 — control flow

`if` branches of differing types; a non-bool condition.

### Slice 5 — composites

Unknown field; wrong field count in a struct literal; indexing a scalar; field access on a scalar.

## Sizing, honestly

**The shapes are enumerated and the harness pattern exists**, so there is no discovery left in
*what* to check. The unknowns are two, and both are about representation rather than about types:

1. **How a type is represented in a total language with no sum types in the stage's own subset.** The
   `verify_typed.kel` shape lattice `(tag, size, kind)` is the closest precedent and is probably the
   answer, but it was built for flat layout rather than for source types.
2. **How much of the AST has to reach the stage.** The wire-format work has just established a
   module-input encoding for names; a type checker needs expression structure, which is a different
   and larger input surface.

The second is the one that could turn out large. It is the same class of work as the wire format's
"wire the driver to a module, not to a model", and the same rule applies: **doing the checker before
its input encoding is wasted**.

## Ordering against the wire-format line

The wire-format wiring and this share an input-encoding problem. **Neither should invent a second
encoding.** If the module-input blob grows an expression section, that section serves both, and this
plan should follow the wire-format line rather than run beside it.

## Caveats

- The fifteen shapes come from **one execution over eighteen programs**. A shape reachable in the
  subset that no program in that set reached would be missed, and the corpus is the only guard.
- Nothing here is built. Every size above is a projection from a measured shape count, not from a
  written stage.
