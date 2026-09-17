# BRIEF — re-run the optimiser sweep, because I changed the emitter six times

## The gap my own work opened

The corpus-wide `default<O2>` execution differential was run once, green, at
`d99ed3de`. **Six emitter changes have landed since**: width propagation through
`Div`/`Mod`, the bitwise trio, the four shifts, `Op::Not`, `FixedMul`/`FixedDiv`,
and `FixedToWord`.

Those changes alter what the emitter produces. **`-O0` is a codegen setting, not a
pass pipeline**, so every gate run since has exercised the new lowerings without
the middle end ever seeing them. **Undefined behaviour in emitted IR is invisible
at `-O0` and appears at `-O2`** — which is the entire reason that sweep exists.

**A green sweep from before the changes is evidence about the code before the
changes.** That is the same class as a guard that ran before the last edit, which
is the FIRST row in this project's catalogue of how a green run under-reports.

## And the generated subjects have never been optimised at all

`KEL_OPTIMIZE` turns the hook on everywhere, so the several thousand generated
scalar, byte, composite and nesting programs would all go through the middle end.
**They have only ever run at `-O0`.** They are the densest differential in the
package and the cheapest to re-run.

## Prior failures to avoid

- **Reading a flat runtime as proof the optimiser did not run.** That nearly became
  a false negative last time: the corpus sweep cost the same with O2 as without,
  because LLVM is a small share of a VM-dominated workload. **The probe settles it,
  not the clock** — with the variable set, a panic inside the hook fires.
- **Leaving the variable set** for later runs, which would silently change what
  every subsequent measurement means.
- **Reading a killed run as a pass.** The corpus phase must be detached; it is
  over the foreground ceiling and slower under the middle end.
- **Capturing only the tail.** Leak lines, slow lines and failure lines all scroll.

## What either outcome means

- **Green**: the corpus and the generated subjects agree under `default<O2>` at
  this commit, on this machine — **not** that the emitted IR is free of undefined
  behaviour, which is input- and version-dependent.
- **Red**: a divergence that appears only under optimisation is the most serious
  class this line can find, because it means the emitted IR permits something the
  optimiser is entitled to exploit. It would take priority over everything queued,
  and would establish that the two implementations disagree — **not which is
  wrong**.
