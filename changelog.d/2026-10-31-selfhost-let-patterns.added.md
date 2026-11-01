- **The self-hosted compiler compiles nested and variant `let` patterns.**
  A tuple, struct or single-variant enum pattern nested in any other, such
  as `let (a, (b, c)) = t`, `let Q { p: P { x, y }, z } = q`,
  `let (P { x, y }, c) = v`, `let S { t: (a, b), z } = s` and
  `let W::A(x, y) = w`, was refused as outside the subset. The destructuring
  filter now gives each nested pattern a hidden binding of its own, as the
  reference gives it a fresh local, and the parser reads a variant's payload
  as the reference does, so each compiles byte-identically. A grouping
  parenthesis is transparent as in the reference. A binding of a whole
  tuple-typed or enum-typed struct field now keeps its type. A typed pattern,
  more than six open patterns, more than twenty-four pattern items and a
  variant whose payload holds an enum are still refused. Pinned by
  `let_patterns_nest_and_bind_variants_byte_identically` in
  `tests/selfhost_full_language.rs` and the `let/` rows of the
  construct-support boundary in `tests/selfhost_codegen.rs`.
