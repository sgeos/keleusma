# Nested Loop Arenas

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: Open. Design record awaiting operator answers to the questions at the end. No
runtime change is made by this record.
**Date**: 2026-11-04.
**Line**: V0.2.x, branch `docs/v02-nested-loop-arenas` cut from `v0.2.3` at `ff48e35c`.

## The ruling this record answers

The operator ruled on 2026-11-04, verbatim.

> A. Admit, but my understanding that nested loops need their own arena so that inner and outer
> loops reset independently. This also has implications for code hot reloading, so this is a
> non-trivial language feature.

The question ruled on was whether a `loop` may call a `loop`. The admission half matches the
implementation. `category_can_call` in `src/typecheck.rs` answers true for a `loop` caller and any
callee, section 6 of [the composite-region evidence](./COMPOSITE_REGION_EVIDENCE.md) records the
arrangement as constructible, and `tests/stream_never_returns.rs` pins it. Section 6.3 of
[the grammar](../spec/GRAMMAR.md) and its productive divergent function summary still say that one
`loop` exists per script. Amending the grammar is another task and is not done here.

This record takes the second half of the ruling as a design question. It measures what happens
today, states the problem that the measurements leave, and sets out the candidate designs with
their bound, hot reload, opcode and wire costs.

## How to read the evidence

Each claim carries one of three markers.

- **Measured** means a test in this tree executes the behaviour and asserts it. The tests named
  below are in `tests/delegated_loop_reset.rs` unless another file is named.
- **Read** means the claim was taken from the source and not executed in this pass.
- **Inferred** means the claim follows from measured or read facts by an argument stated here,
  and nothing executes it.

The measured figures are for the default feature set, a 64-bit word and the 32-byte `Value` slot.
The byte counts change under a narrow word, and the relations between them are what the record
relies on.

## Measured current behaviour

### What an inner Reset does to the arena

The probe program is `OUTER_HOLDS_ARRAY` in the test file. The outer `loop main` builds a
sixteen-element array, whose flat body is 128 bytes in the top region, and passes one element to
`loop child`, which builds a four-element array of 32 bytes in every iteration.

| Observation | Figure | Marker and test |
|---|---|---|
| Top region in use at the first yield | 160 bytes, the outer 128 and the inner 32 | Measured, `an_inner_reset_reclaims_the_outer_frames_top_region_bodies` |
| Top region in use after the first inner Reset | 0 bytes | Measured, same test |
| Top region in use at later yields | 32 bytes, the inner array only | Measured, same test |
| Top region peak over the run | 160 bytes | Measured, same test |
| Bottom region across Resets | Unchanged at every suspension | Measured, same test |
| Bottom region of the nested program against the inner body run alone | Larger, 656 against 344 bytes in the measured configuration | Measured, `the_inner_loop_alone_has_a_smaller_resident_bottom_region` |
| Epoch | Advances by one at each inner Reset | Measured, `an_inner_reset_reclaims_the_outer_frames_top_region_bodies` |
| Private data written by the outer loop before it delegates | Survives every inner Reset | Measured, `private_data_written_by_the_outer_prologue_survives_inner_resets` |

The `Op::Reset` arm of `src/vm.rs` explains the figures. It clears the locals of the resetting
frame only, truncates the operand stack to that frame's locals, resets the top region through
`reset_top_unchecked`, which rewinds the top pointer to the capacity and advances the single
epoch, and clears the ephemeral opaque registry. It does not touch the persistent region, and it
does not touch any frame beneath the resetting one. This was read, and the table above measures
its consequences.

So after the first inner Reset the outer frame still holds, in its local slot, a handle to an
array body that the arena has reclaimed and that the inner loop's next allocation reuses. The
outer frame is stale, not zeroed. Its locals keep their old values and the memory they name now
belongs to the inner loop.

### Why the stale outer frame is not a defect today

A `loop` chunk emits no `Op::Return`. Its only exits are `Op::Reset`, which restarts the same
frame, and `Op::Trap`. So once the outer loop calls the inner loop, no instruction of the outer
chunk runs again, and nothing reads the stale locals. `no_compiled_stream_chunk_emits_return` in
`tests/stream_never_returns.rs` pins the emission fact over five shapes. Section 8 of the
composite-region evidence reads from dispatch that a stale handle read would surface as
`InvalidBytecode` through the epoch check rather than as a wrong value, which is a second line of
defence that this pass did not execute.

The routes by which an outer value could reach code that runs after an inner Reset were probed one
by one.

| Route | Result | Marker and test |
|---|---|---|
| Outer local read after the call | Unreachable, the call never returns | Read, and pinned by `tests/stream_never_returns.rs` |
| Outer value passed as the inner loop's first parameter | Rebound to the resume value at each inner Reset since pull request 704 | Measured, `a_nested_delegation_chain_rebinds_the_innermost_loop` and the earlier cases in the file |
| Outer value passed as a second or later loop parameter | Refused at compile time by the definite-initialization rule, for the entry loop, a delegated loop and a three-level chain | Measured, `a_second_loop_parameter_is_refused_because_reset_clears_it` |
| Outer composite written into private data | Copied into the persistent region and survives | Measured, `private_data_written_by_the_outer_prologue_survives_inner_resets` |
| A `yield` function calling a `loop` | Refused by the type checker | Read, `category_can_call` in `src/typecheck.rs` |
| A composite yielded to the host during the first iteration | Stale after the next Reset exactly as for a single loop, and `Vm::decode` refuses it | Read, `ref_context_for` in `src/vm.rs` |

The second-parameter refusal supersedes an observation in
[the stream spill layout record](./STREAM_SPILL_LAYOUT.md), which reported that a stream with more
than one parameter compiled, verified and faulted. Measured today, it does not compile.

**Defect status.** No soundness defect was found in admitted programs. Every admitted nested
program probed in this pass produced defined results, ran within its auto-sized arena, and could
not observe the reclaimed outer bodies. This is a finding about the routes listed above and the
compiler's present emission, not a proof that no other route exists. No interim refusal is
proposed, because there is nothing unsound to refuse.

### The memory bound

`module_wcmu` in `src/verify.rs` charges each call site its callee's figure, and a call to a
stream chunk is charged that chunk's Stream to Reset figure. Measured, the outer chunk's heap term
is 160 bytes and the inner chunk's own is 32, and the outer figure equals the measured top peak,
`the_composed_outer_wcmu_covers_the_measured_top_peak`. The load-time check and
`auto_arena_capacity_for` take the maximum over stream chunks, so the composed outer figure governs,
and the program runs in its auto-sized arena, `the_nested_program_runs_in_its_auto_sized_arena`.

The declared header figures `wcet_cycles` and `wcmu_bytes` are computed per chunk without call
composition, as `src/compiler.rs` and the documentation of `wcet_stream_iteration` in
`src/verify.rs` state. That is a property of every call, not of nesting, and it is recorded here
only because the first slice of a nested program spans two chunks.

### The time bound

Read, and inferred. The per-iteration time bound is computed per stream chunk and charges a call its
dispatch cost only. For a nested program the host sees three kinds of slice. The first runs the
outer prologue and the inner loop up to its first suspension. Later slices run within the inner
loop alone, from a suspension to the next suspension or Reset. The inner chunk's figure bounds the
later slices. No per-chunk figure bounds the first slice, which is the general shallow-call
limitation and not something nesting introduces. A composed time bound would charge the outer
chunk the inner chunk's figure at the call, which bounds the first slice and, because the inner
figure is part of it, every later slice as well.

### Hot reload

`Vm::replace_module` in `src/vm.rs` verifies the new module, clears every frame and both arena
ends, and leaves the next `call` to start the new module's entry. Read.

| Observation | Marker and test |
|---|---|
| The only Reset a nested program ever reaches is the innermost active loop's. The outer loop's own Reset is unreachable once it delegates | Read from the absence of `Op::Return`, and measured by the Reset sequence in `a_swap_at_an_inner_reset_restarts_the_outer_prologue` |
| A swap at that Reset is memory safe, because it discards the outer frames with their stale handles | Read |
| The next `call` reruns the outer prologue, so a yield the outer loop makes before delegating is seen by the host again | Measured, `a_swap_at_an_inner_reset_restarts_the_outer_prologue` |
| An argument transformation in the outer prologue applies again to the first post-swap iteration. In a chain where `mid` calls `leaf(b + 100)`, the first iteration sees the offset and later ones do not | Inferred from `a_nested_delegation_chain_rebinds_the_innermost_loop`, which measures the offset on the first iteration only |
| The execution model's statement that the stack is quiescent at RESET by construction is false for a nested program, since the outer frames remain | Measured by the unchanged bottom region above |
| The virtual machine does not refuse a swap requested after a yield rather than at a Reset. The rule that a swap happens only at RESET is a host contract | Measured in a scratch probe on a single-loop program during this pass and not pinned, read from the documentation of `replace_module` |

So nesting turns the innermost Reset into a restart point for the whole program rather than a
continuation point. What a swap loses is the position in the delegation chain and every effect of
the outer prologues, which it replays. What it must preserve is what it already preserves, the
private data the host supplies and the host's own shared buffer.

The native route mirrors the virtual machine. `rebind_delegate_parameter` in
`native_codegen/src/coroutine.rs` rebinds the innermost loop's parameter, and a nested stream
clears its own locals on Reset. Read. Any change to the semantics of `Op::Reset` must be made on
both routes and differentially tested.

## The problem

The operator's understanding is that inner and outer loops reset independently. Measured today,
they do not, and the reason is not the arena. Once the outer loop delegates, it never resets
again, because the inner loop never returns. The inner Reset reclaims the outer loop's top region
bodies too, which is harmless only because the outer loop never runs again.

Independent resets therefore need one of two things that the language does not have.

1. **An inner loop that returns.** Some construct ends the inner loop and resumes the outer
   iteration, which then reaches its own Reset. At that point the outer frame's arena handles must
   still be valid, and today they are not, so this is where a per-loop arena becomes a soundness
   requirement rather than a convenience. Productivity must also be restated, since the outer
   iteration would yield through its callee on some slices and directly on others.
2. **Loops that run side by side.** The outer loop keeps running and resumes the inner one. This
   is the sub-coroutine model of [the sub-coroutine design](../architecture/SUB_COROUTINES.md),
   which gives each sub-coroutine its own arena slot and which requires new opcodes.

Without either, a per-loop arena changes nothing a program can observe. It would retain outer
bodies that nothing reads. The one observable gain short of either feature would be admitting
second and later loop parameters that survive the inner Reset, which a per-loop arena makes sound
and which the definite-initialization rule refuses today.

## Candidate designs

The designs assume the hard limits of this line. No opcode is added, the opcode count stays 66,
and `BYTECODE_VERSION` stays 2.

### D0. Terminal delegation, the present semantics, specified

A `loop` that calls a `loop` hands over control permanently. The callee's Reset is the only
reachable Reset and the only swap point, and a swap restarts the program at its entry.

- **Bounds.** Unchanged. The memory bound already composes, as measured. The time bound has the
  general shallow-call limitation and nothing more.
- **Hot reload.** A swap replays the outer prologues. This is documented behaviour rather than a
  hazard once it is written down, and a host that needs a transparent swap writes the outer loop
  as a pure delegation, `loop main(t: Word) -> Word { child(t) }`.
- **Opcode and wire.** None.
- **Cost.** Documentation, plus the optional hardening below.

### D1. An arena mark at each loop entry

When a call enters a stream chunk, the virtual machine records the current top pointer as that
loop's mark and pushes it on a mark stack beside the frames. The loop's Reset rewinds the top
region to its own mark, not to the capacity, so bodies allocated by enclosing loops survive. A
return from an inner loop, if one is ever admitted, pops the mark and reclaims everything above it.

- **Liveness.** The arena's single epoch cannot distinguish an outer handle from an inner one,
  since both predate the inner Reset. The check must become region-aware, a handle is reclaimed
  when its address lies below the mark of the loop that reset and its epoch is older than that
  loop's last Reset. This needs a new operation on `keleusma-arena`, a rewind to a mark with a
  per-level epoch, which is an additive change to a separately published crate.
- **Memory bound.** Unchanged in value, inferred. The composed outer figure already counts the
  outer prologue and the inner iteration together, and D1 keeps exactly that much live. Steady
  state usage rises from the inner figure to the composed figure, which is the bound the arena is
  already sized to. The mark stack holds one entry per active loop. The call graph is acyclic, so
  the depth is at most the number of `loop` functions in the module, which is a static bound.
- **Time bound.** A Reset gains one mark lookup, a constant cost per Reset.
- **Hot reload.** Unchanged from D0 for a whole-module swap, which clears every mark with the
  frames. D1 is the mechanism that would later allow a swap of the inner chunk alone with the
  outer frames retained, as the sub-coroutine design contemplates, because the outer bodies would
  then still be valid.
- **Opcode and wire.** None. It changes the runtime meaning of `Op::Reset` for nested programs
  only. For a single loop the mark is the capacity and the behaviour is identical.
- **Cost.** A virtual machine and native route change, an arena change, and differential tests.
  It buys nothing observable until a returning loop or surviving extra parameters are admitted.

### D2. A separate arena region per nesting level

The load-time sizing partitions the top region into one slice per static nesting level, each sized
by that level's own heap figure, and each loop's Reset clears only its own slice.

- **Memory bound.** The sum of the per-level figures, which for a strict chain equals the composed
  figure D1 keeps live, inferred. For loops whose lifetimes nest strictly, a partition and a mark
  stack hold the same bytes, and the partition only pays for itself when lifetimes do not nest,
  which is the sibling case of the sub-coroutine design.
- **Time bound.** Constant per Reset.
- **Hot reload.** As D1, with the extra cost that a swap which changes the nesting structure must
  recompute the partition.
- **Opcode and wire.** None, but the sizing routes and `auto_arena_capacity_for` change, and the
  slices fragment the arena.
- **Cost.** Higher than D1 for the same behaviour on the only shape the language admits today.

### D3. Refuse arena-backed values live across an inner loop call

The compiler refuses a program in which an arena-backed value is live in the outer frame across a
call to a loop chunk. The inner Reset may then reclaim everything.

- **Today.** Vacuous, since nothing after such a call is reachable. It costs a liveness analysis
  and refuses nothing.
- **With a returning loop.** A cheap alternative to D1. The outer frame keeps only scalars and
  private data across the inner loop, so the single epoch suffices.
- **Bounds.** Unchanged.
- **Hot reload.** No effect.
- **Opcode and wire.** None.
- **Cost.** A compile-time liveness pass, and expressiveness, since configuration built by the outer
  loop must go through private data.

### D4. A static bound on nesting depth

The depth is already bounded by the acyclic call graph, at most the number of `loop` functions.
An explicit constant cap would give the mark stack of D1 or the slices of D2 a fixed reservation
and would give the native route a fixed frame count.

- **Bounds.** A fixed bottom-region reservation of one mark per level.
- **Cost.** Small. It is a complement to D1 or D2, not an alternative.

### D5. Refuse a loop calling a loop

Reverting to the grammar's one-loop rule. Listed for completeness only, since the operator ruled
to admit.

### Hardening that applies to every design

`tests/stream_never_returns.rs` records that the absence of `Op::Return` in a stream chunk is an
emission fact, not a verified property. A hand-written module with a `Return` in a stream chunk
would resume the outer frame with stale handles. Read from dispatch, the epoch check would turn
such a read into an error, so the outcome is defined, but the premise that makes D0 sound is not
checked by the verifier. Adding the check to pass 2 of `verify` in `src/verify.rs`, and its twin
in the self-hosted validator, costs no opcode and no wire change.

## Comparison

| Design | Memory bound | Time bound | Hot reload | Opcode or wire change | Observable gain now |
|---|---|---|---|---|---|
| D0 | Unchanged | Unchanged | Restart at entry, prologues replay | None | None, it specifies today |
| D1 | Unchanged in value | Constant per Reset | As D0, enables per-chunk swap later | None, `Op::Reset` semantics for nested programs | Only with returning loops or extra parameters |
| D2 | Sum of levels, equal to D1 for chains | Constant per Reset | As D1, partition recomputed on swap | None, sizing routes change | As D1 |
| D3 | Unchanged | Unchanged | No effect | None | None until loops return |
| D4 | One mark per level | None | No effect | None | None alone |
| D5 | Unchanged | Unchanged | Unchanged | None | Contradicts the ruling |

## Recommendation

Adopt D0 for V0.2.x and specify it. A `loop` calling a `loop` is a terminal delegation. Its
innermost Reset is the only reachable Reset and the only swap point, and a swap restarts the
program at its entry and replays the outer prologues. Add the hardening that the verifier refuses
`Op::Return` in a stream chunk, mirrored in the self-hosted validator. Correct the execution
model's quiescence statement for nested programs.

Defer independent resets to the feature that makes them observable. If that is a returning inner
loop on this line, pair it with D1 and D4, and keep the definite-initialization refusal of extra
parameters until D1 lands. If it is the sub-coroutine model of V0.5.0, its arena slots already are
per-loop arenas and D1 is the strict-nesting special case of them.

The reasoning is that the measured semantics are sound, that the operator's concern about
independent resets is real but arises only with a construct the language does not yet have, and
that every arena design costs a runtime change on two routes while buying nothing a present
program can observe.

## Questions for the operator

Each question lists the recommended option first.

**Q1. What a `loop` calling a `loop` means in V0.2.x.**
- A. Terminal delegation as measured, specified in the grammar and the execution model.
- B. A returning inner loop on this line, with a new exit construct and D1.
- C. Side-by-side loops, which is the V0.5.0 sub-coroutine design and needs opcodes.

**Q2. The arena mechanism once independent resets exist.**
- A. A mark per loop entry, D1, with the static depth bound of D4.
- B. A static region per nesting level, D2.
- C. Refuse arena-backed values live across an inner loop call, D3, with the single epoch.
- D. D3 now and D1 later.

**Q3. Hot reload at an inner Reset.**
- A. Whole-module restart at the entry, documented, with the outer prologues replayed.
- B. Refuse at compile time an outer loop that does anything but pass its parameter to the inner
  loop, so that a restart is indistinguishable from a continuation.
- C. A per-chunk swap that keeps the outer frames, which needs D1 and belongs with the V0.5.0
  live update model.

**Q4. The emission fact that a stream chunk has no `Return`.**
- A. Make it a verifier check, mirrored in the self-hosted validator.
- B. Keep it an emission fact pinned by `tests/stream_never_returns.rs`.

**Q5. The nesting depth bound.**
- A. The acyclic call graph bound, at most the number of `loop` functions, stated in the
  specification.
- B. An explicit constant cap.

**Q6. Second and later loop parameters.**
- A. Keep the definite-initialization refusal.
- B. Admit them as values that survive Reset, which requires D1.

**Q7. A swap requested away from a Reset.**
- A. Have `replace_module` refuse unless the machine is at a Reset, has not started, or has been
  recovered by `reset_after_error`.
- B. Keep the Reset-only rule a host contract, as documented today.

## Not established

- The native route was read, not executed, for any case in this record.
- The epoch check on a stale outer handle was read from dispatch and not executed, because no
  compiled program reaches one.
- No composed time bound exists, so the first-slice argument above is an inference.
- The probes cover the shapes listed. Other shapes, such as a delegated loop reached through a
  `match` arm or an inner loop that calls natives allocating in the arena, were not probed.

## References

- `tests/delegated_loop_reset.rs`, the measurements of this record and the pull request 704 cases.
- `tests/stream_never_returns.rs`, the emission fact.
- [The composite-region evidence](./COMPOSITE_REGION_EVIDENCE.md), sections 6 and 8.
- [The sub-coroutine design](../architecture/SUB_COROUTINES.md), arena slots and hot replacement
  quiescence.
- [The execution model](../architecture/EXECUTION_MODEL.md), the reset domain and hot code swapping.
- [The arena sizing record](./ARENA_SIZING_ROUTES.md).
