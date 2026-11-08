# The backend lowers modules the virtual machine would refuse to load

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: documented and pinned, not enforced.** This is a **precondition gap, not a live corpus
defect** — every module the backend lowers today is bounded, measured at **66 lowering, 0 unbounded**.

## What was measured

Mutating `04_for_in.kel` by a single `CheckedAdd` → `CheckedSub`:

| check | result |
|---|---|
| `verify()` | **accepts** |
| `auto_arena_capacity_for` | rejects — *"loop at instruction 12 has no statically extractable iteration bound"* |
| `module_wcmu` | rejects, same reason |
| `Vm::new` | **rejects** |
| `module_refusals` (this backend) | **`[]` — accepts** |
| the lowered code | **SIGBUS** |

The mutant is well-formed bytecode: same arity, same types, one opcode different.

## Why it matters, stated no more strongly than the evidence supports

`lower_module` documented no admissibility precondition and checked none. **Verified is not enough** —
`Vm::new` additionally requires a statically extractable resource bound, and that bound is the
guarantee this project sells. An ahead-of-time path that runs what the bound analysis refuses is a
hole in the value proposition, not merely a crash.

**Nobody is running such a module today.** The gap is that nothing said the caller must not.

## How it was found, which is the part worth reusing

Not by reading the source. It fell out of an unrelated measurement — a sweep asking which differential
subjects would notice a wrong backend, which built mutants and ran them. **The sweep died with SIGBUS
and the crash was the finding**, larger than the census that produced it.

The census then needed two filters, each of which is a correctness point rather than a convenience:

- **An inadmissible mutant is not a wrong backend.** It is a program the runtime would refuse, so
  comparing against it measures nothing. This filter is what removed the SIGBUS.
- **A mutant that FAULTS is not a wrong backend either.** Checked arithmetic is supposed to trap, and
  both sides do; on the native side that arrives as a process-killing signal. This filter removed a
  subsequent SIGTRAP.

## The disposition, and why enforcement was not chosen

| option | cost |
|---|---|
| **document the precondition and pin the corpus** (taken) | the hazard remains for a careless caller |
| enforce it inside `lower_module` | couples a pure lowering function to the resource analysis, and pays that cost on every call |
| a debug assertion | free in release, but the bound analysis is not cheap in debug |

**Enforcement is a real option and is not ruled out here.** What made documenting sufficient for now
is the measured zero: no shipped module violates the precondition, and
`no_lowerable_corpus_module_is_unbounded` fails the day one does.

## What this does NOT establish

- **Not that the backend is unsafe on admissible input.** Every module measured as admissible lowered
  and ran without incident.
- **Not that `verify()` is wrong.** It accepts the mutant correctly; bound extraction is a separate
  analysis that `Vm::new` runs and this backend does not.
