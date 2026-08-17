# Sizing: reaching a non-literal operand in self-hosted type rejection

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**A sizing result, measured 2026-08-16. Not a plan and not an implementation.**

> **OUTCOME, 2026-08-16.** The slice this sized has landed and the estimate held. Local resolution
> is in `verify_types.kel` as a binding table plus a lookup, with ONE alias hop for a `let` bound to
> a call. No unification, no fixpoint, no new input channel. The four cases below moved from
> disagreements to ordinary corpus members.
>
> **One thing the estimate did not anticipate.** `let a = g()` needs the let rule and the
> declared-return rule COMPOSED, and composing them host-side would have been the very join the
> stage exists to perform. It is resolved in the stage by an alias row and a bounded hop instead.
> The prototype below did the composition in the host and so did not surface it -- **a throwaway
> prototype measures reachability, not where the reasoning belongs.**

## The question

The self-hosted type-rejection stage rejects all fifteen enumerated shapes, but every rule fires
only where the offending operands are **literals**. `expr_tag` maps a literal to its kind and
everything else to `0`, UNKNOWN, which the stage must not reject. Measured consequence, pinned in
`the_rules_reach_only_literal_direct_occurrences`:

| case | reference | stage |
|---|---|---|
| `1 + true` | rejects | rejects |
| `let b = true; 1 + b` | rejects | **accepts** |
| `g() + true` | rejects | **accepts** |

So: what would the pipeline have to compute to do better, and how large is that?

## The answer: two local rules, and no unification

Measured by `sizing_how_far_local_propagation_reaches` with a throwaway host-side prototype that
adds exactly two rules, both lookups over information the source states outright:

1. a `let` whose initialiser has a known tag binds that tag;
2. a call takes the callee's **declared** return type, and a parameter its **declared** type.

**Result: 5 of 5.** Every constructed case is reached, including the composed one
(`let a = g(); a + true`) which needs both rules together.

**Nothing unifies.** There is no substitution, no occurs check, no type variable. That is the
sizing result that matters, and it is a consequence of the subset rather than a lucky corpus: the
self-hosting subset is monomorphic `Word`/`Byte` code in which **every function declares its
parameter and return types and every `let` has an initialiser**. There is no position where a type
is determined by use rather than by declaration.

> **What this does NOT establish.** Five constructed cases are a case list, and this project has
> been bitten by exactly that four times. The claim is bounded: it says the two rules reach the
> cases measured, and it gives a structural reason to expect that to generalise over the subset. It
> is not a proof, and the first real implementation should keep the well-typed controls, because
> **rejecting a valid program is a language change** rather than a conservative choice.
>
> It also says nothing about the full language. Generics, traits and bounds are Order 6, and there
> unification is unavoidable.

## The channel: no new encoding is required

`parse.kel` already carries everything the two rules need.

- **Declared types are already on the record.** `ParsedFn` carries `param_types` and `return_type`,
  produced by the pipeline's own parser.
- **Let initialisers are in the body records**, which `reconstruct.kel` already walks into an AST
  node array.

So the tags are a **computation over records the pipeline already emits**, not a new input surface.
That matters because the wire-format plan and the type-checker plan were both warned not to invent
a second encoding, and this session has twice found a reconstruction that drifted from its original.

## Sizing, stated as a range

- **Small** if the tagger is a pass over existing records producing a per-operand tag: two rules, no
  fixpoint, no unification.
- **Larger** by whatever it costs to express that pass in a total language with static loop bounds
  and no sum types — the same cost the other `.kel` stages paid, and the reason the range is not
  tighter here. `verify_typed.kel`'s `(tag, size, kind)` lattice is the nearest precedent, though it
  was built for flat layout rather than source types.

**What it is NOT** is a Hindley-Milner port. `src/typecheck.rs` is 8601 lines of which a large share
serves traits and bounds; none of that is required for obligation 1.

## What would change the answer

A subset widening that admits any of: a `let` without an initialiser, a function without a declared
return type, or generics. Each introduces a position where a type is determined by use, and each
turns the two rules into a fixpoint.
