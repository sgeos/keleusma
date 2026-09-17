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


---

## OUTCOME

**The sweep was run and it is green. The more useful result is what the sweep's
absence had hidden.**

### The corpus-wide run

`KEL_OPTIMIZE=1` over `corpus_differential`: **10 tests, 0 failed, frozen tree,
382.5 seconds.** Six unoptimised runs of that same phase the same day took
379-433s, so **the middle end costs approximately nothing here** — the reason the
sweep stayed opt-in does not hold.

**The variable's reach was proven before the result was believed.** The flat
timing was suspicious: if O2 had run on every module, why no cost? A probe inside
the hook settles it — with the variable set the probe panics, without it the same
test passes. So the variable does reach the test process, and the flat timing
simply means LLVM is a small share of a VM-dominated workload.

**Scope of the claim**: the corpus agrees under `default<O2>` at this commit on
this machine. **Not** that the emitted IR is free of undefined behaviour; an
optimiser exploiting UB is input- and version-dependent.

### ⚠ AND THE COVERAGE WOULD HAVE SURVIVED A DEAD OPTIMISER

Adding permanent optimised-execution coverage, four subjects driven through the
middle end on every run, it passed first time. **Then the instrument was
perturbed, and the result was worse than expected**: stubbing the shared
`force_optimize` helper to return immediately left **all three** tests in
`optimised_lowering.rs` passing — including the one named *"the O2 pipeline
measurably transforms a real module"*.

**That test ran `run_passes` inline**, so it guarded a pipeline nothing else used.
The file's own header warns against exactly this: *"An unguarded 'it passes under
O2' would be the same mistake in a new place."* It was already the same mistake,
one helper away from where the header was looking.

Routed through the shared helper now. A dead optimiser fails the suite.

### A fifth probe implicating itself

One subject was written with `let mut`, and **Keleusma has no mutable local**. The
failure was a `ParseError`, not a divergence. Replaced with nested conditionals,
which is what an optimiser folds anyway.

### What this does NOT settle

Whether the sweep should join the routine gate. It costs one extra corpus phase of
roughly six and a half minutes, which is a gate-time decision rather than a
correctness one.
