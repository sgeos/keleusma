# The corpus differential's exemptions: which are real?

**Status**: CONFIRMED by execution. 34 -> 37 executed, 23 -> 20 exempt.
One prediction was wrong and the miss is recorded below.
**Date**: 2026-08-15.

> **2026-08-16: 44 executed, 2 vacuous, 18 exempt.** `led.kel` is CLOSED — the
> `gpio_set` composite return this document predicted was closeable is now
> honoured on both sides, from one shared byte builder. `rogue_dungen.kel`
> remains un-closeable for the reason stated below, which has not changed: the
> range lives only in the host's head. **Closing `led.kel` cost a `Trap` subject
> rather than gaining one** — a faithful stub returns a valid variant, so the
> module no longer faults at all. See `NATIVE_MUTATION_CENSUS.md` Part E.
>
> **Later the same day: 38 executed, 5 vacuous, 20 exempt.** The extra module is
> `lexer.kel`, which left `KNOWN_VACUOUS` rather than the exemption list, so the
> figures in this document are unchanged where it speaks about EXEMPTIONS. The
> two remaining exemptions analysed below — `rogue_dungen.kel` and `led.kel` —
> are unaffected, and what the bytecode records for each is now measured rather
> than assumed: see `what_return_contract_does_the_bytecode_record`.

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

---

## MEASURED 2026-08-15: three of four, and the miss was informative

**Executed and agreeing: 34 -> 37. Exempt: 23 -> 20.**

### The prediction against the outcome

| predicted | actual |
|---|---|
| `event_listener`, `heartbeat`, `led`, `sensor` compile and become candidates | **three did.** `led.kel` compiles and then FAULTS |
| `faulty.kel` compiles, then faults with `DivisionByZero` | **exactly so**: *"the VM refuses to resume it: `DivisionByZero`"* |

So the prediction was right about `faulty.kel`, right that the rejections were a
harness artefact, and **wrong that all four would execute**.

### The miss confirms Finding 1's generalisation

`led.kel` now reports *"the VM refuses to run it: `NoMatchingArm`"*. It matches on
the `Status` enum returned by the `gpio_set` native; the generic stub returns
`stub_value(...)`, an arbitrary integer corresponding to no variant, so no arm
matches.

**That is the `rogue_dungen` mechanism again, in a module found by fixing a
different problem.** Finding 1 said the generic stub *"can drive any module into a
state a contract-respecting host would never produce"* and that it *"was the only
one of its kind, but the reasoning generalises"*. It generalised within the hour,
and the second instance was not predicted.

Two exemptions therefore remain that describe the HARNESS rather than the module:
`rogue_dungen.kel` and `led.kel`. Both are honestly stated now, and both would be
closed by a stub that respects its native's contract — which the harness cannot
derive from the bytecode, since a declaration carries types and not ranges or
enum domains.

### Not a coverage claim without the vacuity bar

The three new modules are not in `KNOWN_VACUOUS`, so each is non-trivial on at
least one observable. That was checked rather than assumed, because admitting
modules that then agree vacuously would be no gain at all.
