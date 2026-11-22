- **The ring buffer type `Ring<T, N>` parses and lays out, increment R1.** A field of a
  `private data` or `shared data` block may be declared as `Ring<T, N>` with `T` one of
  `Word`, `Byte`, `bool`, `Fixed` and `Float` and `N` a positive constant up to 16,777,213.
  The field lowers to three hidden data fields, a length word `r.len`, a head word `r.head`
  and the elements `r.buf: [T; N]`, so it occupies `2 + N` data slots and loads empty. A
  ring anywhere else, an inadmissible element or capacity, an initializer, and every
  expression that reads or writes a ring field are refused at compile time with a diagnostic
  naming the rule, because the ring operations are not yet built. The self-hosted compiler
  produces the same module byte for byte on ring declarations. No opcode, wire format or
  `BYTECODE_VERSION` change. The verifier is not changed and no soundness invariant is
  relaxed, since a ring field reaches it as three ordinary data fields. Pinned by
  `tests/ring_buffer.rs` and by `ring_fields_have_their_pinned_self_hosted_outcomes` in
  `tests/selfhost_full_language.rs`.
