- **`stage_executor::StageExecutor` and `selfhost::with_stage_factory`.** Every stage of
  the self-hosted toolchain, and the cryptographic framing stage, is now driven through a
  trait holding exactly the operations a host performs on a stage program, with the virtual
  machine as the reference implementation. A host may install a `StageFactory` for a scope
  to run stages by other means, such as native builds, and the frame natives reach a
  returned executor through `register_native`. The full self-hosted tier passes unchanged.
  Pinned by `tests/stage_executor.rs`. Requested by the V0.3.X line.
