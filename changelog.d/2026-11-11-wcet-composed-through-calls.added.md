- **A time bound composed through calls,
  `verify::module_wcet_composed_with_bounds` and `Vm::wcet_per_iteration_composed`.** The
  existing time bound is shallow. An `Op::Call` contributes its dispatch cost and not the
  callee's body, so the figure is below what executes for any program that calls a
  function. The proofs line measured an example whose header declares 20 cycles while
  872 operations execute in one iteration. The new function visits the call graph
  callees first and charges each call site its callee's composed figure, scaled by loop
  multiplicity and saturating. An entry bounds one whole call for a `fn` or `yield`
  chunk and one iteration for a `loop` chunk, and it is not a bound per resume slice.
  For a delegated `loop` the callee's own entry bounds every iteration after the first,
  and the entry chunk's bounds the first. **This is an addition.** The existing
  functions, `Vm::wcet_per_iteration` and the declared `wcet_cycles` header return what
  they returned before, and whether they adopt the composed figure is an open decision.
  No verifier acceptance rule changes, so no soundness invariant is relaxed. The
  relation to execution is tested on nine programs under a cost of one cycle per
  operation, in `tests/wcet_composed.rs`, and is not proven.
