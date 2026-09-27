# Print-Memory Bound

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: defect found and repaired 2026-09-27, session 67. One related question is
**measured and left open** for the operator. No language, instruction-set, or wire-format
change.

---

## The defect

`keleusma run <file> --print-memory` exists, in its own documentation, to report "the
program's worst-case arena footprint ... for provisioning a host". It computed the transient
region as

```
keleusma::vm::auto_arena_capacity_for(module, &[]).unwrap_or(DEFAULT_ARENA_CAPACITY)
```

That call returns `Result<usize, VmError>`, and it fails for a program whose worst-case
memory usage cannot be statically bounded. The discarded error was the only record of why.
So the flag printed the **default arena constant** as though it were the program's computed
bound, and exited zero.

Measured on the recursive example from `book/src/19_why_rejected.md`:

| invocation | result |
|---|---|
| `run rec.kel` | refused, `count_down: recursive call detected during WCMU topological sort` |
| `run rec.kel --print-memory` | **`arena: 65536 bytes total (persistent 0, transient 65536)`, exit 0** |

An operator sizing a host from that figure would be provisioning for a program that cannot
run. The transient figure equalling the whole default arena is the tell, and nothing in the
output distinguished it from a computed bound.

**Why this one matters more than its size.** The crate's stated value proposition is
definitive worst-case execution time and memory usage, and `CLAUDE.md` requires never
implying completeness where verification is incomplete. The single tool whose entire output
is a worst-case bound is the sharpest possible place to report a placeholder instead.

## The repair

The sizing function now returns the verifier's error instead of swallowing it. The reporting
path propagates it, printing no figure and exiting non-zero with the cause named. The
allocation path keeps the default substitution, in a separately named function whose comment
says it is a fallback and says what catches the unbounded case instead.

**The two paths need opposite treatments, which is why the fallback was not simply removed.**
Verification compares the arena's capacity against the program's bound, so an arena must
exist before the check can run. Refusing to allocate would replace a precise verification
error with an allocation error. On that path the unbounded case is caught immediately
afterwards by `Vm::new`; on the reporting path nothing ran afterwards at all.

`keleusma-cli/tests/print_memory_bound.rs` pins both directions on both input forms, source
and compiled bytecode, each refusal paired with a program that must still report. Restoring
the swallowing call reproduces the original output exactly and fails the two refusal tests
while leaving the control passing, so the tests discriminate rather than merely fail.

**No expected byte count is asserted anywhere in that file.** The same session removed two
stale byte-count claims from the guide; an arena figure moves with the compiler, and pinning
one would manufacture the next stale claim.

## Measured and left open: `compile` emits an artifact no host can load

`keleusma compile rec.kel -o rec.bin` **succeeds** and writes the file. `keleusma run
rec.bin` then refuses it for the same unbounded-memory reason. So the compiler emits a
module that cannot load anywhere.

**This is not obviously a defect, and the distinction matters.**

- A module whose bound is **computable but larger than the default arena** is legitimately
  emittable: a host with a bigger arena loads it, which is the cross-compilation case
  `compile --target` exists for.
- A module whose bound is **not computable at all**, as here, can be loaded by no host at any
  capacity.

`auto_arena_capacity_for` already distinguishes them: it fails only in the second case. So
refusing at compile time is implementable and cheap.

**Not done, because it changes the contract of a published subcommand.** Whether `compile`
should refuse a module no host can load, or continue to emit it and leave the refusal to
load time, is a decision about that subcommand's purpose rather than a consequence of this
measurement. Recorded for the operator with the distinguishing test already available.
