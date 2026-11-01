- **A struct pattern destructures in a `let`.** `let P { x, y } = p` was
  refused with `undefined variable: x`. It now binds each named field, with
  field renaming, wildcard fields, a subset of the fields and nested patterns,
  the struct pattern a `match` arm accepts. The self-hosted compiler compiles
  a flat struct pattern of names and wildcards byte-identically, by the
  destructuring filter that already lowers a flat tuple pattern, and refuses a
  nested pattern as outside its subset. Pinned by
  `tests/let_pattern_irrefutable.rs` and
  `let_struct_patterns_compile_and_refutable_lets_are_rejected` in
  `tests/selfhost_full_language.rs`.
