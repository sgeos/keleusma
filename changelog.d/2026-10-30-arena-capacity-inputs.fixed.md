- **`auto_arena_capacity_for` now counts the composite host inputs of the entry
  chunk.** The VM copies each composite entry argument and each composite resume
  value into the arena, where it stays until the next `Reset`, and the figure
  omitted those copies, so an arena of the reported size failed with `OutOfArena`
  at the first composite input. The figure now adds the entry arguments, one resume
  value for each `Yield` an iteration can execute, counted through bounded loops
  and called `yield` functions by the new `verify::max_resumes_before_reset`, and
  one transient body for packing a nested value. A module whose entry takes and
  resumes with scalars gets the same figure as before. `Text` inside a composite
  input has a length the module cannot bound, so the new
  `auto_arena_capacity_with_input_text` takes the host's bound on it, and the new
  `host_input_arena_bytes_for` returns the input term alone. Packing a nested host
  composite now reclaims the dead bodies of its children, so a value holds its own
  flat size whatever its nesting depth. The worst-case memory invariant this
  preserves is that an arena of the reported size admits the call and every resume
  up to each `Reset` for inputs of the declared types without `Text`. Pinned by
  `tests/auto_arena_host_inputs.rs`.
