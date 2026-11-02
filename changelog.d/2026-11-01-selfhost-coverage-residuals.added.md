- **The self-hosted compiler closes five coverage residuals, each byte-identical to the
  reference.** A generic call on a resume value specializes at the reference's resolved
  resume type, the first parameter of `loop main` when there is one and otherwise the kind a
  later annotated `let` fixes, which the parser reports in a probe round. An enum-typed payload
  field of an enum declared earlier is laid out at that enum's size and read whole, in a
  `match` binding and in a destructuring `let`. A typed destructuring pattern annotates the
  binding that stores the value. A range induction variable is a `Word` value, so a tuple
  holding one has a layout. A payload whose type is declared after its enum, an equality of
  enums whose payload is an enum, and a resume value of a type the parser does not model are
  refused as unsupported rather than emitted divergently. `Text<N>` remains refused. Pinned by
  `coverage_residuals_compile_byte_identically_or_are_refused` and
  `pipes_into_generic_functions_compile_byte_identically` in
  `tests/selfhost_full_language.rs` and by `self_hosted_construct_support_boundary` in
  `tests/selfhost_codegen.rs`, now 138 cases at 130 SOk, 4 Refuses, 2 Diverges and 2
  RefRejects.
