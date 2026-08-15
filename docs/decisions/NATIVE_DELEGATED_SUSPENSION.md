# Delegated suspension in the native lowering

**Status**: IMPLEMENTED behind an off-by-default flag, verified by execution on a
synthetic module. `codegen.kel` remains refused by default. See the closing section.
**Subject**: `codegen.kel`, the last module the native backend refuses.
**Date**: 2026-08-14.

## The recorded diagnosis was too pessimistic, and this document says why

The handoff, `rogue_ai_differential.rs`, and three earlier sections of
`NATIVE_LOWERING_INVENTORY.md` all describe this as a **design problem, not an
implementation one**, on the following grounds:

> `resume_after_enter` writes slot 0 of the ENTRY chunk whenever that entry is a
> `Stream`, regardless of which frame suspended, so a nested `yield fn` callee's
> suspension updates the entry's resume parameter in the virtual machine while
> natively the `kel_yield` return reaches only the callee's operand stack and the
> next iteration reads a stale value.

**Every clause of that is accurate. The conclusion drawn from it is not**, because
it describes the general case and `codegen.kel` does not present the general case.
The bytecode says so, and `probe_delegated_suspension.rs` prints it.

## What `codegen.kel` actually contains

The entry is five ops:

```text
chunk 12 `main`  Stream  params=1
   0  Stream
   1  GetLocal(0)
   2  Call(6, 1)      -> chunk 6 `emit_next`, Reentrant
   3  PopN(1)
   4  Reset
```

There is **exactly one yielding chunk in the module** and **exactly one call site
of it**, and that call site is in tail position. `emit_next` carries nine
`Yield`s, one per head of the multiheaded `yield emit_next`, and:

**Every one of the nine is immediately followed by `Return`.**

```text
chunk 6 `emit_next`  Reentrant
   ...  Call(75, 0) ; Yield ; Return ; EndIf ; ...   (x8 guarded heads)
   ...  Const(0)    ; Yield ; Return                 (the unguarded head)
   ...  Trap(1)                                      (NoMatchingHead)
```

The callees of those `Call`s — `seed_step`, `walk_step`, `drain_return` and the
rest — are declared `fn`, not `yield`. **They are pure total functions and cannot
suspend.** The suspension is exactly one frame deep, and it is in tail position on
both sides of the call edge.

## The semantics, executed rather than reasoned

On resume the virtual machine does two separate things, and only one of them is
live for this shape.

1. `resume_after_enter` writes the input into **slot 0 of `self.frames.first()`**,
   the entry frame, when the entry is a `Stream` with parameters.
2. It pushes the input onto the operand stack, so it becomes the value of the
   `yield` expression **in the frame that actually suspended**.

For this shape, (2) is dead. `emit_next`'s op after every `Yield` is `Return`, so
the resumed value is returned straight out; the entry then discards it with
`PopN(1)` and rewinds at `Reset`. The **only** live path for a resume value is (1),
the entry's slot 0, which the next iteration reads at `GetLocal(0)`.

`probe_delegated_suspension.rs::the_resume_value_reaches_the_entrys_slot_zero_and_is_dead_in_the_callee`
asserts this by running it, on a synthetic module of the same shape whose observable
depends on the resume value:

```text
first call arg 7, then replies 11, 22, 33, 44
yields = [107, 211, 322, 433]
```

Each iteration yields `st.n * 100 + resume`. The reply for iteration *k* appears in
iteration *k*'s output, which is only possible if it arrived through the entry's
slot 0 and was passed down as `emit`'s argument.

## The transform

The existing degenerate-stream transform turns

```text
Stream ; <body> ; Yield ; PopN(1) ; Reset
```

into a function that returns at the `Yield`, with the next call's argument
supplying the resume value. **The extension is that same transform spanning one
call edge**:

- In a qualifying callee, `Yield` lowers to `return <yielded value>`.
- In the entry, a tail-position `Call` to a qualifying callee is treated as the
  `Yield` is treated today: its result is returned directly rather than popped and
  followed by `Reset`.

No continuation, no suspended-frame record, no state machine, and no new opcode.
The callee's locals need not survive the suspension because the callee returns
through it.

## The admission predicate

`degenerate_stream_yield` must **not** simply be widened. The new rule is a
separate predicate over the call edge, and every clause is load-bearing:

1. The entry is a `Stream` whose ops are exactly a prologue-free
   `Stream ; <arg setup> ; Call(c, n) ; PopN(1) ; Reset`, with the `Call` in tail
   position. A prologue would run once in the virtual machine and on every native
   call.
2. The callee `c` is `Reentrant`, and **every** `Op::Yield` in it is immediately
   followed by `Return`. One yield that is not makes the resumed value live in the
   callee, and the transform loses it silently.
3. The callee contains **no** `Op::Call` to any chunk that is not `Func`, applied
   transitively. Suspension must be exactly one frame deep.
4. The callee's own body otherwise satisfies the existing per-op lowering rules.
5. The module contains **no other call site** of `c`. A second caller that is not
   the entry would reach a `Yield` lowered as a return and take it for an ordinary
   result.

Clause 2 is where the general case is refused, and refusing it is the point.

## What this does NOT solve, stated plainly

- **A yield that is not in tail position of its callee.** The resumed value is
  then live in the callee, and modelling it needs the suspended frame's locals to
  survive — the real design problem the record described. Nothing here addresses
  it, and clause 2 refuses it.
- **Suspension more than one frame deep.** Clause 3 refuses it.
- **A callee shared between the entry and an ordinary caller.** Clause 5 refuses it.

Each refusal is a fail-closed path, not a gap in a proof.

## The open question that gates landing this

**`codegen.kel` cannot be execution-differentiated by this line.** Its input is an
abstract-syntax-tree block (`ast.root`, `ast.kinds[]`, `ast.args[]`, ...) whose
layout belongs to the `src/selfhost/mod.rs` driver, a file this line may read but
must not edit. Seeding it means reproducing that format, and a seed the stage
silently rejects looks exactly like coverage — the precise failure Part A of this
increment was spent removing.

So admitting `codegen.kel` on this predicate would rest on `lower_module`
returning `Ok`, which **is a fact about the compiler and not about the program**,
and which has stood in for verification twice on this line and was wrong both
times.

**The synthetic reproducer dissolves this for the MECHANISM but not for the
MODULE.** A module of the identical shape is in
`probe_delegated_suspension.rs` and can be driven on both sides, so the transform
itself can be verified by execution. What that does not verify is `codegen.kel`
specifically.

Two ways forward, and the choice is worth making explicitly rather than by
default:

- **Land the transform, verified on the synthetic shape, and admit `codegen.kel`
  with its lowering unexecuted.** Honest only if the inventory and the handoff say
  so in those words, and if the must-not-fire control in
  `rogue_ai_differential.rs` is replaced by something that still asserts a
  boundary.
- **Land the transform, keep `codegen.kel` refused** until its input can be
  driven, and let the synthetic case carry the coverage. Costs the eleventh stage
  and the headline count; keeps every admitted module executed.

The second is the more conservative and matches this line's stated stance. The
first is defensible if the record is explicit. **This is not a decision to make
silently while implementing.**

## Reproduce

```sh
cd native_codegen
cargo test --test probe_delegated_suspension -- --nocapture
```

Four tests: the bytecode shape of `codegen.kel`, the entry's zero yields, the
synthetic reproducer's structure, and the executed resume-value semantics.

---

## OUTCOME 2026-08-14: implemented, flagged off, `codegen.kel` still refused

### The seam Part B needed does not exist, and that is why the flag is off

The plan was to seed the five vacuous stages from the real driver, which would
also have given `codegen.kel` a real input and dissolved the gate above.
**It cannot be done from this subproject**, and the reason is specific rather
than a matter of taste:

- the per-stage seeding is written **inline** in each `*_via_kel` function, not
  factored into anything that returns the buffer;
- the **78** slot constants (`SV_*`, `DV_*`, `TV_*`, `DL_*`, `BR_*`) are private;
- the two helpers a copy would need, `analyze_class` and
  `verify_depth_kel_module`, are private as well, so even a verbatim copy would
  not compile;
- **no function anywhere in `src/selfhost/mod.rs` returns or exposes a seeded
  shared buffer.**

Reproducing the formats by hand is the one thing that must not be done here: a
seed a stage silently rejects looks exactly like coverage, which is the defect
this whole arc exists to remove.

### What was implemented

`LowerOptions::delegated_suspension`, **off by default**, plus
`delegated_suspension_plan`, which implements the five clauses above and returns
`(entry, callee, call_op_index)`.

The lowering touches exactly two chunks. The callee's `Yield`s become returns
through the existing `degenerate_yield` mechanism. The entry is given an **empty**
yield list, which makes `Stream` and `Reset` lower to nothing while marking no
`Yield`, and its tail-position `Call` returns its result instead of pushing it.

### Verified by execution, and by mutation

`delegated_suspension.rs`, five tests:

- the synthetic shape **agrees with the virtual machine over 40 ticks**, on an
  observable that depends on both a persistent counter and the resume value, with
  a distinct-value floor so a run that stops working fails rather than passes;
- with the flag **off** the same shape is still refused;
- a callee whose `Yield` is **not** in tail position is refused even with the flag
  on — clause 2, the clause that refuses the general case;
- that control was checked to refuse **for the right reason**: the two sources
  differ only in what follows the yield, the tail one lowers, and the non-tail one
  reports `UnsupportedOp("Stream")`. A control that fired for an unrelated reason
  would leave clause 2 untested while looking tested;
- `codegen.kel` **qualifies under the predicate** and is **still refused by
  default**. Both halves are asserted, because the first shows the predicate is
  not merely fitted to the synthetic case and the second is the standing decision.

Mutation: removing the delegated return makes the native side yield forty zeros
against the virtual machine's real sequence. `src/lib.rs` restored byte-identical
under `cmp`.

### The decision that is now the operator's, with its cost

The transform exists and works. Turning the flag on for `codegen.kel` would take
the self-hosted stages from ten of eleven to **eleven of eleven** and remove the
last refusal in the corpus.

**The cost is that `codegen.kel`'s lowering would be unexecuted.** Nothing would
run it natively and compare, so its admission would rest on `lower_module`
returning `Ok` — a fact about the compiler and not about the program, and the
claim that has been wrong twice on this line.

The alternative that removes the cost entirely is a seam on the `v0.2.3` side:
any one of a `pub` on the slot constants, a function returning a seeded buffer,
or a `#[cfg(feature = "self-host")]` accessor would let this line drive the stages
on real input. That is a request, not a change to make here.
