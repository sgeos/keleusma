# BRIEF — relax one guard route, and let the differential decide

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The state

Slice one built the operand-kind channel, a float constant, both conversions, and float
`Add`/`Sub`/`Mul`, verified against the reference through `lower_chunk`. **The module-level guard is
untouched**, so nothing float-carrying reaches `lower_module` and the censuses have not moved.

The guard closes four routes, and **only one of them now has an implementation behind it**:

| route | implemented? | disposition |
|---|---|---|
| chunk **constant** | **yes** — constant, conversions, float arithmetic | **relax** |
| chunk signature | no — the entry ABI is not built | **keep** |
| native return shape | no — no float native ABI | **keep** |
| data slot | no — float slots are an open ABI question | **keep** |

## Why this is the right next slice

`float_witness.kel` is blocked by the **constant** route. Relaxing it makes the module lower, which
puts it into the **corpus differential** — and that is a materially stronger check than slice one's,
because it runs the whole module against the virtual machine rather than a hand-built chunk.

**The coverage gain is real and small**: 66 → 67 modules, and the two conversions stop being UNPROVEN
because the census would finally *visit* them.

## What protects the relaxation

The operand **whitelist**: an opcode consuming a float that was not written for one refuses. So a
float constant flowing into division, a comparison, a composite, or a native still fails closed at the
use. The guard route is being replaced by a finer check, not removed.

## Wrong turns to avoid

- **Do not relax more than one route.** Three have nothing behind them, and a module accepted through
  them would be lowered wrong rather than refused.
- **Let the differential decide, and revert if it disagrees.** Lowering the module is not the same as
  computing the same answer. If `float_witness.kel` disagrees or traps, the relaxation comes out and
  the finding is reported — a wrong float is a plausible number, not a fault.
- **Expect the censuses to MOVE, and say which and why.** Modules lowering, and the UNPROVEN column,
  should both change. **A relaxation that moves nothing means the module is still refused for another
  reason**, which would need explaining rather than celebrating.
- **Do not assume the module gets a resource bound.** It was refused by the backend, not the runtime,
  but that is an assumption until the differential actually runs it.
- **Do not weaken the whitelist to make the module lower.** If it refuses at an operand, that is the
  guard working and the slice is incomplete, not the guard wrong.
- **Check `float_guard_routes.rs` still passes.** It asserts each route refuses; one of its assertions
  is now expected to change, and it must be updated to state the new truth rather than deleted.

## What good looks like

`float_witness.kel` lowers, runs in the corpus differential, and **agrees**. The three unimplemented
routes still refuse. The census movement is stated with its cause.
