- **The self-hosted validator decides the definite-initialization rule.** `verify_depth.kel`
  computes, in the run that checks operand-stack depth, the same least fixpoint of written locals
  as `keleusma::verify_init`, and then checks every reached read against it.
  `verify_module_via_kel` reports a refused read as the `initialization` stage. A chunk with more
  than 256 locals, or one whose fixpoint is not confirmed within the sweep budget, is unavailable
  analysis rather than a verdict. The soundness invariant preserved is the reference's, that a read
  of a local that no path writes is refused. Pinned by `tests/selfhost_definite_initialization.rs`,
  and by `tests/definite_initialization.rs` for a local written on either arm of an if.
