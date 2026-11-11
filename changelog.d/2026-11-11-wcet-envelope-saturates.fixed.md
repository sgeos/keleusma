- **The time bound of a `Stream` iteration saturates where it wrapped.**
  `verify::wcet_stream_iteration_with_cost_model` added the costs of `Stream` and `Reset`
  with a plain `+`. Under a host cost model with large costs the sum panicked in a build
  with overflow checks and wrapped in one without, so the function returned a figure
  below the cost of the instructions it had charged. The sum now saturates, as every
  other sum and product of the time analysis already did. The soundness invariant
  restored is that a figure is never below the true cost. The documentation now states
  the contract, which is that a result of `u32::MAX` is saturated and is not a bound. The
  compiler treats it so. It writes the overflow sentinel to the header, which `Vm::new`
  refuses, and it no longer records a `wcet-per-iteration-bound-proven` witness or a
  marker for a chunk whose figure saturated. Found by the proofs line with Kani, as
  finding F1. Pinned by `tests/bound_arithmetic_saturation.rs`.
