# The corpus differential's exemptions: which are real?

**Status**: two findings established by reading; execution confirmation pending.
**Date**: 2026-08-15.

`corpus_differential` exempts 23 sources with a stated reason each. The reasons
were believed legitimate. Two groups are not: **their reasons describe the
HARNESS and read as properties of the module.**

## Finding 1: `rogue_dungen.kel` — two recorded facts, both true

The tree contains what looks like a contradiction:

- `corpus_differential` exempts it: *"the VM refuses to run it:
  `IndexOutOfBounds(630, 8)`"*.
- `rogue_dungen_differential.rs` runs the same module against the same virtual
  machine on two floors and **passes**.

**Both are correct, and they are not in conflict.** The difference is the native
stub, and it matters because the module's control flow depends on what its
natives return.

| harness | `host::rng_range(lo, hi)` returns |
|---|---|
| `corpus_differential` | `stub_value(...)`, an arbitrary `acc % 1024` |
| `rogue_dungen_differential` | `lo + ((lo*7 + hi*13 + 5) % span).abs()`, **inside `[lo, hi)`** |

`rogue_dungen.kel` is a dungeon generator that uses `rng_range` results as map
indices. The generic stub does not respect the range implied by the arguments, so
an index lands far outside its array and the virtual machine faults —
`IndexOutOfBounds(630, 8)`, an index of 630 into eight elements, which is exactly
the signature of an out-of-range random value rather than of a broken module.

**The module is fine. The stub violates a contract the harness cannot see.** The
declaration `use host::rng_range(Word, Word) -> Word` carries types and not
ranges, so nothing in the bytecode tells the harness what the contract is.

### Why this is worth more than one exemption line

The generic stub can drive **any** module into a state a contract-respecting host
would never produce. So "the VM refuses to run it" is not, on its own, evidence
about a module. It is one of the three exemption reasons that describes the
harness, and it was the only one of its kind, but the reasoning generalises.

## Finding 2: five rtos scripts are rejected only because they are compiled alone

`event_listener.kel`, `faulty.kel`, `heartbeat.kel`, `led.kel` and `sensor.kel`
are exempted as *"rejected by the REFERENCE compiler"*. That reads as a statement
about the scripts. It is a statement about how the harness compiles them.

`led.kel` says so in its own header:

> `Status` and `StatusErrorCode` are declared in the prelude
> (`scripts/prelude.kel`), **prepended at compile time by `setup::build_module`**.

And `examples/rtos/src/setup.rs:429` does exactly that:

```rust
fn build_module(src: &str) -> Result<Module, String> {
    let combined = format!("{}\n{}", PRELUDE, src);
    ...
}
```

The corpus harness compiles each `.kel` standalone, so every rtos script that
references a prelude declaration fails to compile, and is recorded as a
reference-compiler rejection.

### Why prepending here is NOT the thing Part B refused to do

Part B of this increment declined to reproduce the self-hosted stages' input
formats by hand, because a seed a stage silently rejects looks exactly like
coverage. **This is a different act.** The composition is four lines, it is the
host's own, it is quoted above from the shipping source, and it is documented in
the scripts themselves. Reproducing it invents nothing, and a wrong composition
fails loudly at compile time rather than silently producing a plausible run.

### A prediction, recorded before it is measured

`faulty.kel` compiles fine. Its header says it *"deliberately triggers
`VmError::DivisionByZero` every fifth iteration so the kernel's supervised-restart
policy can be observed end to end"*, which is a RUNTIME fault in a valid module.

So the predicted outcome of prepending the prelude is:

- **four** scripts — `event_listener`, `heartbeat`, `led`, `sensor` — compile and
  become candidates for execution;
- **`faulty.kel`** compiles and then faults in the virtual machine, moving from a
  FALSE exemption reason ("rejected by the REFERENCE compiler") to a TRUE one
  ("the VM refuses to run it: `DivisionByZero`"), which is a legitimate exemption
  to keep.

Writing the prediction down first is the point. Measuring and then describing
whatever happened as expected is how a check stops being one.

## What this does NOT claim

- **Nothing here is confirmed by execution yet.** Both findings come from reading
  the sources and the host. The prepend may expose further reasons these scripts
  do not lower, and the corrected `rogue_dungen` exemption may reveal that the
  module fails for an additional reason once its stub is well behaved.
- **The remaining exemptions are still believed legitimate**: ten `piano_roll`
  string arguments blocked on the ABI decision, two preludes with no entry point,
  three rogue AI modules with a composite entry parameter covered by hand-written
  differentials, one module requiring a signature, and `codegen.kel`.
- **This is not a coverage claim.** Admitting five modules that then agree
  vacuously would be no gain at all, which is the lesson of the stage finding
  earlier in this increment. Each admitted module must clear the vacuity bar.
