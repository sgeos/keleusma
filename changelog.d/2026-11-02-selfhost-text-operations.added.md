- **The self-hosted compiler lowers the five `Text<N>` operations byte-identically.**
  `t.len()`, `t.capacity()`, `a.concat(b)`, `a.eq(b)` and `t.slice(s, e)` compile to the
  reference's instruction sequences, with the defined result for every operand body, on a
  local, a parameter, a call result, a field, an array element, a match binding and a
  parenthesized literal cast. An operation whose instruction count would exceed 7000, and a
  text value passed to a generic function, are refused as unsupported. Pinned by
  `the_self_hosted_compiler_lowers_the_operations_byte_identically` in `tests/text_ops.rs`.
