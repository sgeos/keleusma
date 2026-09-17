# BRIEF — the corpus has never been EXECUTED under the optimiser

## The gap, stated precisely

Three facts, each checkable:

1. **Every execution differential on this line runs at `OptimizationLevel::None`.**
   That is a codegen setting; the middle end is a pass pipeline and does not run
   from it. **Undefined behaviour in emitted IR is invisible at `-O0`.**
2. `corpus_differential` can run the whole corpus through `default<O2>` — but only
   when `KEL_OPTIMIZE` is set in the environment. **Nothing sets it.** Not
   `tools/backend-gate.sh`, and continuous integration does not build this package
   at all.
3. The unconditional guard `corpus_modules_still_verify_after_the_middle_end`
   checks that corpus IR **still verifies** after O2. **That is not the same
   question as whether it still computes the same values**, and undefined
   behaviour characteristically manifests as a wrong result rather than as invalid
   IR.

**So the corpus has been optimised and validated, and never optimised and RUN.**

## Why this is worth the wall clock

This is the one remaining place a **genuine backend defect** could be hiding in
bulk. Everything else this session found was a record overstating what the tree
supports; the backend itself survived every instrument. **An O2 execution
differential is a different instrument, not a rephrasing of an existing one**, and
region aliasing — which this backend does a great deal of, handing every call site
a disjoint block of a caller's buffer — is exactly the sort of thing an optimiser
reasons about aggressively.

The one genuine codegen defect this line has ever found was a composite-return
aliasing bug. That is the family this run probes.

## Method

Run the existing corpus differential with `KEL_OPTIMIZE` set. **Nothing new is
written first** — the capability exists and is documented; what has never happened
is someone running it and reading the result.

## Prior failures to avoid

- **Reading a killed run as a pass.** At `-O0` this phase takes 380 seconds; with
  the middle end on every module it will take longer and **must be detached**, not
  run in the foreground against a ten-minute ceiling. Three background launches
  were killed mid-flight earlier in this line's history and one was read as a
  result.
- **Capturing only the tail.** A `LEAK` line, a `SLOW` line and a failure line all
  scroll past. **Capture the whole log**; this session already lost one leak
  observation to `tail` and recorded it as an omission.
- **Declaring victory from a green run without asking what it covered.** If the
  run passes, the honest claim is *"the corpus agrees under `default<O2>` on this
  machine, at this commit"* — not that the emitted IR is free of undefined
  behaviour. An optimiser exploiting UB is input- and version-dependent.
- **Assuming a failure is the optimiser's fault.** If results diverge under O2,
  the divergence is evidence that the two implementations disagree, **not which
  one is wrong** — the same rule this line applies to the reference compiler.
- **Leaving the environment variable set** for subsequent runs, which would
  silently change what every later measurement means.

## What to do with either outcome

- **Green**: record that the corpus executes identically under the middle end,
  with the cost, and decide separately whether it belongs in the routine gate —
  that is a per-run cost and the operator has said such costs are their call.
- **Red**: that is a backend finding and it takes priority over everything else
  queued. Reduce it to the smallest module that diverges before reporting.
