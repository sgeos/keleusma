- **`Text<N>` values and the literal cast `"..." as Text<N>` compile.** A `Text<N>` with
  a positive literal capacity is admitted after monomorphization as a local, a parameter,
  a return value, a tuple, struct, array, `Option` or enum element, and a `private` data
  field. The cast lowers to one pooled constant holding the length word and the UTF-8
  bytes zero padded to `N`. A literal longer than `N` bytes is a compile error naming both
  lengths, as is `Text<0>` and a capacity left symbolic after monomorphization. Every
  other cast, arithmetic, comparison, destructuring, a `shared` data field and a native
  signature naming `Text<N>` stay refused, and the self-hosted compiler refuses `Text<N>`
  as unsupported. No opcode, wire format or `BYTECODE_VERSION` change is involved. The
  soundness invariant preserved is that no path produces a `Text<N>` whose length exceeds
  `N`, so each allocation holding one is bounded by `word_bytes + N`. Pinned by
  `tests/textn_literal.rs` and `tests/text_capacity_type.rs`.
