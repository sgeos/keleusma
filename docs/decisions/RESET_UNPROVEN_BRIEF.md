# BRIEF — the last unproven opcode, and the hazard in the method that would "resolve" it

**Written**: 2026-08-27, seventeenth loop iteration. **For this line's own use.**

## Where the characterisation stands

`isa_lowering_census` partitions the 65 opcodes the corpus emits: **61 lower, 1 refused (`Len`), 3
UNPROVEN** — emitted, never visited, never named.

**Two of the three are already resolved elsewhere.** `backend_support_census` drives hand-built
probes and reports `IntToFloat` and `FloatToInt` as **REFUSED**, by a module-level float-signature
guard with a stated reason. **`Reset` appears in neither its supported nor its refused column.**

**So `Reset` is the single opcode in the instruction set whose backend status is unknown.**

## The trap: the obvious way to resolve it produces a FALSE SUPPORTED

`backend_support_census` decides by asking **"was there a refusal?"** — emission is checked first, so
a probe that does not emit the opcode is caught. **But a probe that emits an opcode the lowering
never VISITS produces no refusal either, and lands in the supported column having proved nothing.**

**`Reset` is exactly that shape.** `isa_lowering_census` records it as *"appearing ONLY in skipped
positions"*, and `a_degenerate_stream_visits_its_stream_op_and_never_its_reset` names the mechanism:
the degenerate-stream transform reaches `Stream` and steps over `Reset`.

> **So adding a `Reset` probe naively would move it from honestly-unproven to falsely-supported.**
> That is the flattering direction, and this exact failure has happened in this file before: the
> comment records `IntToFloat` and `FloatToInt` moving *"from refused to lowers while the backend had
> gained no float support whatever"*, because module-level refusals were not being matched.

## What the increment is

**Establish `Reset`'s status honestly, and if the method cannot, fix the method rather than the
verdict.** `isa_lowering_census` already distinguishes *visited* from *stepped over*; the probe
census does not. **The asymmetry is the finding.**

Three outcomes, all publishable:
- The probe census can be taught to see "emitted but never visited", and `Reset` is then reported as
  such rather than as supported.
- `Reset` turns out to be genuinely visited and lowered by a probe, resolving it outright.
- Neither, and the reason is recorded.

## Prior failures this is exposed to

1. **A false SUPPORTED in the flattering direction** — the trap above, with precedent in this very
   file.
2. **A vacuous probe.** Twelve guards or filters broke this session. The emission check is already
   there; the *visit* check is what is missing.
3. **Claiming a status the evidence does not support.** "No refusal" is not "lowers".
4. **Breaking the census's own instrument guard.** `isa_lowering_census` records that if `Reset`'s
   stepped-over count reaches zero, *"every LOWERS verdict has silently become the unsound
   chunk-level inference"*. **A probe adds no corpus module, so it cannot move that figure — but
   verify rather than assume it.**
5. **Reporting a figure without the command that produces it.**
6. **Running the two suites in parallel** — invalidates the perf canary. Sequential.

## Specific wrong turns to avoid

- **Do not edit `src/` or any read-only file.**
- **Do not add a `Reset` probe without a visit check.** That is the entire hazard.
- **Do not weaken `isa_lowering_census`'s UNPROVEN column to make the two censuses agree.** They read
  different populations and neither subsumes the other; the file says so already.
- **Do not report `Reset` as lowering because a module containing it compiled.** The module compiling
  says the OTHER opcodes lowered.
