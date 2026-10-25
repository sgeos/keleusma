# WCET structural markers

> **Navigation** [Decisions](./README.md) | [Flat Text decode soundness](./FLAT_TEXT_DECODE_SOUNDNESS.md)

## Finding

On 2026-10-25 the Order 4 hosted runtime counted the instructions each corpus
program executed and compared the count with the entry chunk's verified time
bound. Every instruction costs at least one cycle in `NOMINAL_COST_MODEL`, so
the count can never exceed a sound bound. Seven runs exceeded it by one.

The time walk, `verify::wcet_region_at`, charged nothing for three structural
markers that the virtual machine dispatches and that the nominal table charges
one cycle each.

1. **`Else`.** The then-region stopped before it, though the then-path
   executes it as the jump to the `EndIf`.
2. **`EndIf`.** Both paths execute it, and the walk resumed after it.
3. **`EndLoop`.** Every completed pass executes it as the back-edge, and the
   body region stopped before it.

A fourth omission was found while reading the loop arm. A counted loop runs its
full passes and then one partial pass, the header check that takes the break.
The walk took the maximum of the full passes and the break path rather than
their sum, so that final header pass went uncharged.

For example, `if true { 1 } else { 2 }` costs 7 nominal cycles on its then-path
and was bounded at 5. `for i in 0..3 { }` with its tail costs 55 and was bounded
at 47. The shortfall grows with the number of `if` executions and loop passes.
The Keleusma mirror in `analyze.kel` carried the same omissions, so the two
analyses agreed on the understated value. Nothing in the specification states
that the markers are free.

## Ruling and repair

The operator ruled first-hand on 2026-10-25 that the bound be fixed in the Rust
walk and its Keleusma mirror, as a separate change before Order 4 continues.

- The then-path is charged its `Else`, and both paths are charged the `EndIf`.
- A bare `if` is charged its `EndIf`.
- A loop whose body falls through is charged its `EndLoop` on every pass.
- A counted loop adds its costliest break path to its full passes. A loop that
  runs a single pass, because its body never falls through or because the
  productive-yield clamp applies, keeps the maximum, since that pass ends inside
  the cost already counted.
- A marker met outside the structured arms is charged like any instruction.

`analyze.kel` makes the same changes, reading each marker's cost from the
per-instruction cost table the host already supplies.

## Effect

Every verified time bound that involves `if` or a loop rises. Memory bounds,
opcodes, `BYTECODE_VERSION` and the wire format are unchanged. An artifact that
declares an explicit time bound carries the producer's figure, which the safe
runtime accepts as declared, so an artifact built before this change keeps its
understated declared bound. That is the same stale-artifact hazard the Order 3
audit records.

## Evidence

`tests/wcet_marker_charging.rs` pins four bounds derived by hand from their
instruction listings, so the walk is checked against the cost model rather than
its own earlier output. `verify::tests::wcet_branching_takes_max` moved from 13
to 14 with its arithmetic restated. The fused-compile record count of
`analyze.kel` moved from 3229 to 3272, and three line citations of
`src/verify.rs` in the composite-region evidence index and its proof moved with
the inserted lines. The release workspace run on 2026-10-25 passed 3504 of 3506
before those two pins were updated, the two failures being exactly those pins,
and the affected binaries passed afterwards. Remote continuous integration
covers the remaining configurations.
