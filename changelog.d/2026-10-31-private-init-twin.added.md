- **The self-hosted validator decides rule 2 of definite initialization.** A read of a
  private data slot whose load-time initializer is `Unit` is refused unless every path
  writes the slot first, as `verify_init` refuses it. `verify_depth.kel` computes each
  chunk's sets, its callee must-write summary and its verdict, and the driver runs the
  chunks callees first. `verify_module_via_kel` reports such a refusal as the
  initialization stage. The soundness invariant preserved is that no read observes the
  empty-slot `Unit`. A module with no `Unit` private initializer costs no extra stage run.
  Pinned by `tests/selfhost_definite_initialization.rs`.
