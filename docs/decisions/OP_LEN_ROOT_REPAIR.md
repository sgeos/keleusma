# The `Op::Len` trap is a small class, and the obvious repair does not close it

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: Analysis, measured against the tree. Not implemented. Written 2026-09-03.

## What is recorded, and what it gets right

`tests/len_flat_array_hazard.rs` pins a latent trap. The compiler emits `Op::Len` on a flat array,
the virtual machine refuses that opcode, and `verify()` accepts the module anyway. What holds the
trap shut is the resource-bound refusal, which this project's own taxonomy calls liftable. On the
day the bound extractor learns to see through an `if`, the program stops being rejected and starts
loading and trapping.

That record is accurate and the ratchet is well built. Its stated root repair is an `Expr::If` arm
in `static_for_in_length` so the length folds.

## The class, measured rather than asserted

`parse_iterable` calls the full expression parser, so **every** expression form is syntactically
admissible after `in`. The population is therefore the whole `Expr` enum, not a designated subset.

`Expr` has **27** variants. `static_for_in_length` folds **6** of them: an array literal, a call, a
field access, an identifier, an array index, and a match. **21 are unfolded.**

Most of the 21 cannot carry a fixed-array type and so cannot reach the hazard. The ones that
realistically can are about seven:

| Form | Can inference type it? |
|---|---|
| `TupleIndex` | yes |
| `If` | no |
| `MethodCall` | no |
| `Pipeline` | no |
| `Yield` | no |
| `Classify` | no |
| `Declassify` | no |

`Closure` and `ClosureRef` are retired surface and are not real members.

**An earlier draft of this document said the class had roughly twenty-nine members. That was
wrong twice over**: the variant count came from a pattern that also matched other structs' fields,
and it ignored that most variants cannot be array-typed. The measured class is small, and the
correction matters because a small enumerable class is worth closing exhaustively, whereas a large
vague one invites a general mechanism nobody validates.

## Why the obvious repair is weaker than it looks

Four of the six existing arms do one thing: infer the expression's type, then read the array length
off it. So the natural fix is to make that the fallback instead of returning nothing.

**That closes only the forms inference can already type, and inference does not handle the ones
that matter.** `infer_expr_type` has no arm for `If`, `MethodCall`, `Pipeline`, `Yield`,
`Classify` or `Declassify`. Of the seven realistic members, the generic fallback fixes exactly
one, `TupleIndex`.

So the gap is jointly in the fold and in inference, and the leverage is in inference: adding `If`
and `MethodCall` there fixes both call sites at once, because the fold can then delegate.
`Classify` and `Declassify` are label wrappers that preserve their operand's type, so each is a
one-line delegation to the inner expression in both functions.

## The floor, which matters more than it first appeared

Inference will always have unhandled forms, so a widened fold still returns nothing sometimes.
Today that emits an `Op::Len` the virtual machine refuses at run time.

**The emission site should refuse instead.** If the iterable's type is a fixed-size array and the
length could not be folded, that is a compiler defect, and a `CompileError` naming it is the honest
outcome. Emitting an opcode the runtime is documented to reject is not. This costs no opcode, which
the rad-hard minimal-instruction-set constraint requires, and it is the property the `v0.3.0` line's
native backend already enforces by refusing `Op::Len` deliberately.

Because the floor is what makes the remaining gaps safe, **it is the part to build first**, and it
is worth having even if none of the fold widening is ever done.

## What implementing this must not do

**Do not delete the ratchet.** It pins a sequence, and after the repair the sequence changes rather
than disappearing: the program should compile and run, and `Op::Len` should not be emitted.

**Do not trust a folded length that inference produced without checking it.** A wrong bound is
worse than a trap: a trap is loud, a wrong iteration count is silent, and the worst-case execution
time analysis consumes this number.

**Do not take the seven-member table above as verified reachability.** It is a judgement about which
forms can hold an array type, not a demonstration that each compiles to the hazard. The cheap
confirmation is one program per row, and that is the first thing the implementing increment should
write, because a row that turns out unreachable should be struck rather than defended.
