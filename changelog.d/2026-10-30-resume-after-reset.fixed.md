- **A stream resume after `Reset` no longer pushes its input.** `Vm::resume` pushed the
  input to complete a yield even when there was none, so every iteration after the first ran
  one operand slot above the bound the verifier proved, and an arena sized by
  `auto_arena_capacity_for` ran out. The memory bound's invariant, that execution never
  exceeds the verified operand-stack peak, now holds across resets. The documentation of
  `auto_arena_capacity_for` states that composite host inputs are outside its figure. Pinned
  by `tests/auto_arena_after_reset.rs`. A pykeleusma finding.
