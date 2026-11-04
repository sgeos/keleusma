- **The self-hosted compiler lowers `Text<N>` and its literal cast byte-identically.** The
  lexer folds `Text<N>` into one token, the parser lays the type out as the tuple of a
  length word and `N` bytes, and `"..." as Text<N>` loads one pooled constant, as the
  reference does. A signature, a `let`, a tuple, an array, a struct field and an enum
  payload are covered. A data field, an `Option` payload, a capacity that is not a literal
  or exceeds 65527, a trait, an impl, a newtype and the five text operations are refused as
  unsupported before any stage runs. The lexer intern table grew from 1280 to 1536 names.
  Pinned by `capacity_bounded_text_has_its_pinned_outcomes` in
  `tests/selfhost_full_language.rs` and the `text_n` cases of the construct-support
  boundary.
