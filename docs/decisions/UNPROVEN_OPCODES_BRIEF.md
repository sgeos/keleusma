# BRIEF — the opcodes whose lowering has never run

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Absorption 23 | yes |
| 2 | Determine which unproven opcodes are REACHABLE at all today | yes |
| 3 | Give a reachable one an execution witness against the reference | yes, conditionally |
| 4 | Keep the gate green, running the gate rather than `cargo test` | yes |
| 5 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |
| 6 | Lower `Stream` | not in one increment |

## Rationale

`isa_lowering_census` reports **61 of 66 opcodes lowered**, of which **three are "emitted, never
visited, never named"** — `FloatToInt`, `IntToFloat`, `Reset` — and one, `IsStruct`, has **no corpus
witness**.

**Lowering code that has never run is where a miscompile hides.** "The backend lowers it" is a claim
about the existence of an arm, not about its behaviour. Every other opcode's arm has been exercised
against the reference; these four have not, and one of them may be reachable today.

The prior increment closed the coverage frontier and found the census counting it wrongly. This is
the same question asked on the other axis: not *how much* is lowered, but *how much of what is
lowered has ever executed*.

**The expected split, to be tested rather than assumed**: `Reset` belongs to the `Stream` family and
is likely unreachable while `Stream` is refused. `FloatToInt`/`IntToFloat` likely require a float to
reach the module, which the float guard refuses — but the guard is about a float CONSTANT, and a
conversion is not obviously a constant, so this needs measuring rather than reasoning. `IsStruct` has
no obvious blocker and is the most likely to be reachable.

## Prior failures to avoid repeating

1. **A census counted chunks of a module it could not lower at all.** An instrument's answer can be
   well-formed and mean something else. **Cross-check instruments against each other.**
2. **`stack_growth`/`stack_shrink` are the peak model, not pop and push counts.** Use
   `verify::op_depth_effect`.
3. **A gate run killed by a signal reported a plausible small number.** Read the exit status.
4. **A negative test asserted something false** about a subject that never had the property.
5. **Coverage is not correctness.** Execution agreement is the evidence.
6. **Five recorded premises have been found false in consecutive increments.** Re-derive.

## Specific wrong turns to avoid

- **Do not conclude "unreachable" from a failed attempt to write a program.** That is evidence about
  the attempt. Say what was tried and what the compiler or backend said, and mark the difference
  between "the surface cannot express it" and "I could not find the form".
- **Do not add an opcode witness that only compiles.** A witness that reaches the arm without
  executing it proves the same thing the census already reports. The deliverable is agreement with
  the reference on a program that actually runs the instruction.
- **Do not weaken the float guard to reach a float conversion.** The float entry ABI is
  operator-held, and admitting floats to reach an unproven arm would be widening on the strength of
  wanting a test.
- **Do not report the census figure as changed unless it is.** A witness that executes does not
  necessarily change "61 of 66"; say what moved and what did not.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
