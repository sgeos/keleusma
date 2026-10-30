- **`verify()` refuses a float opcode on a build without the `floats` feature.** A module compiled
  on a floats build and carrying `IntToFloat` or `FloatToInt` used to verify and load on a build
  without the feature, then fail at the call with `InvalidBytecode`. It is now refused at
  verification, so `Vm::new` refuses it at load. On the operator's ruling of 2026-10-30. Pinned by
  `tests/float_opcode_without_floats.rs`, which runs with
  `--no-default-features --features verify,compile`.
