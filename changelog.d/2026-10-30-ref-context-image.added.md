- **`Vm::ref_context_for` and `Vm::bytecode_image`.** A host that decodes through
  `KeleusmaType::from_value_ctx` can take the context `Vm::decode` uses, including the
  bytecode image and the opaque registry. A context built field by field could not name the
  image, so a constant string in a returned struct could only be refused. Pinned by
  `tests/flat_ref_decode.rs`. A pykeleusma finding.
