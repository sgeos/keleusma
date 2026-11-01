# BRIEF — what happens to the obligation on the day `Stream` lands

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Measure which corpus module the yield-escape refusal takes over on the day `Stream` lowers | yes |
| 2 | Consolidate the obligation's state so the design decision is actionable | yes |
| 3 | Absorption 30 | yes |
| 4 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |

## Rationale

The composite slot-reuse obligation is **required for soundness and not discharged**. Its analysis is
now spread across a dozen increments: the refusal exists, it is shadowed by the `Stream` refusal, its
fireability is proven by bytecode mutation, the interprocedural residual is measured and empty, and a
tail yield turns out to be lowered as a return rather than a suspension.

**Measurement is close to exhausted; what remains is a design decision** — whether the planner may
consume a confinement verdict, which trades the immunity that a verdict-free planner currently
enjoys. That decision is the operator's, and it cannot be made from twelve scattered entries.

**One measurable thing remains, and it is the blast radius.** On the day `Stream` lowers, the
yield-escape refusal stops being a precaution. **Which corpus modules does it then refuse, and does
coverage fall?** That is the number an operator needs to weigh the options, and it is not recorded
anywhere.

**The expected answer is one module** — `13_telemetry_stream.kel` carries the only instance of the
shape — but expected is not measured, and the last dozen increments have repeatedly shown the
difference.

## Prior failures to avoid repeating

1. **Nineteen recorded premises found false or overstated**, several this line's own predictions.
   Write this one down: exactly one module, and coverage does not fall because it is already refused.
2. **A figure without its population.**
3. **Assert on every substitution** — that slip has now happened twice in this session.
4. **A guard's false positive is repaired at the subject**, not by widening the guard.
5. **The first explanation offered has twice been wrong.** Offer it, then test it.

## Specific wrong turns to avoid

- **Do not weaken or remove the `Stream` refusal to measure this.** The measurement is a mutation of
  compiled bytecode in a test, not a change to the backend.
- **Do not present the consolidation as new analysis.** It is a synthesis of measurements already
  made, and anything in it that has not been re-derived should say so.
- **Do not recommend a disposition the numbers do not support.** The options have real costs — one
  region per iteration is unbounded in the iteration count — and the brief's job is to lay them out,
  not to choose for the operator.
- **Do not describe the obligation as discharged.** It is not, whatever the blast radius turns out to
  be.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
