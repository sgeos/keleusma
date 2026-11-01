- **A generic call on a resume value whose type a later use fixes is
  specialized.** Under a `loop main` with no parameter,
  `let r = yield 1; let s = id(r); let u: Word = r;` compiled the generic
  chunk `id` while `let r: Word = yield 1` produced `id__Word`, since the
  type checker inferred `r` from the later use and monomorphization saw only
  `loop main`'s parameter. The type checker now returns the type it resolved
  for each such `yield` at the end of its function, and monomorphization
  types the `yield` by that entry, so both forms compile to the annotated
  form's module. The pipeline order is unchanged. The self-hosted compiler
  now agrees byte-identically where the later use fixes `Word`. Where it
  fixes another type, the self-hosted compiler still specializes at `Word`,
  and the shipping cross-check refuses that divergence. Pinned
  by `tests/pipe_generic_monomorphize.rs` and
  `pipes_into_generic_functions_compile_byte_identically` in
  `tests/selfhost_full_language.rs`.
