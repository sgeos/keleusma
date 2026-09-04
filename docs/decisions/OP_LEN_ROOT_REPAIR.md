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

## The sibling refusals, censused 2026-09-03

`Op::Len` is not the only opcode the virtual machine can refuse outright. Asking which others do
is the natural follow-up, since the trap exists precisely because the set the compiler can EMIT and
the set the machine will ACCEPT are allowed to disagree.

**Four opcode arms can return `InvalidBytecode`**, with comment text excluded so a comment
mentioning the error does not count as a refusal:

| Opcode | Sites | Classification |
|---|---|---|
| `Len` | 2 | The known hazard, on a flat array AND on a flat tuple |
| `IntToFloat` | 1 | Configuration: the `floats` feature is off |
| `FloatToInt` | 1 | Configuration: the `floats` feature is off |
| `Reset` | 1 | Structural: a `Reset` with no `Stream` in the chunk |

### Both questions are now closed, and the answers are DIFFERENT KINDS

**The flat-tuple refusal is not reachable through the for-in path.** Answered with four programs
rather than by re-reading the emission sites, because the array case's own comment asserted
unreachability and was wrong. One of the four compiles -- the one iterating an ARRAY of tuples,
whose length folds statically and emits no `Len`; the three that iterate a tuple directly do not
compile, because a tuple is not an iterable here. Pinned by
`no_tuple_shaped_iterable_reaches_op_len`, which also asserts at least one fixture reaches the
compiler, so the test cannot pass by rejecting everything. **This closes the PATH and not the
opcode**: the bounds-check emission site remains, restricted to boxed arrays, and widening it
reopens the question.

**`Reset` is a corrupt-module defence, not a compiler/machine disagreement.** The two
`Stream`/`Reset` presence checks live in `wcmu_stream_iteration_with_value_slot_bytes`, which
errors with "requires a Stream block" on any other chunk, so they apply to stream blocks only. The
compiler emits `Op::Reset` in exactly two places, both stream-loop epilogues. A stray `Reset` in a
non-stream chunk is therefore not something the compiler produces; reaching that refusal takes a
hand-built or corrupted module, which is what `InvalidBytecode` exists for.

**The distinction is the point.** `Op::Len` is a genuine disagreement -- the compiler emits what the
machine refuses, from ordinary source, with `verify()` accepting the module. `Reset` is a defence
against input no compiler produces. A census that reported both as "opcodes the machine refuses"
without separating them would have implied two hazards where there is one.

### What the census originally left open, retained because the reasoning is the record

**The flat-TUPLE refusal is unpinned.** `tests/len_flat_array_hazard.rs` covers the array case and
does not mention tuples. The tuple case looks unreachable, because `Op::Len` is emitted only from
the for-in dynamic path and from a bounds check already restricted to boxed arrays, and this
language does not iterate a tuple. **That is a judgement from reading, not a demonstration** — and
the array case's own comment asserted unreachability and was wrong. Treat it as probably-defensive
and unproven.

**`Reset` is refused in one direction and verified in the other.** The machine refuses a `Reset`
with no `Stream` in its chunk; `verify()` checks that a Stream block is not missing its `Reset`.
Those are different implications, and whether anything checks the machine's direction was not
established here.

### A note on how this census was taken, because the first attempt was reassuring and wrong

A scan matching opcode arms and looking for `InvalidBytecode` within a fixed twenty-five-line window
reported TWO arms. The true figure is four: the `Len` arm is ninety-nine lines long and its refusals
sit thirty-nine and eighty-eight lines in, outside the window.

**The failure direction is what matters.** An under-reporting census of refusals says the machine is
more permissive than it is, which reads as reassurance. The fix was to extract each arm by brace
matching rather than by a line budget, so the window is the arm's own extent instead of a guess
about it.
