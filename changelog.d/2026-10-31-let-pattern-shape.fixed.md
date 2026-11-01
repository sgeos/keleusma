- **A `let` destructuring pattern is checked against the value's type.** The
  type checker refuses a tuple pattern over a value that is not a tuple, a
  tuple pattern of another arity, and a struct, enum or literal pattern that
  does not match the value, with the message a `match` arm already gives.
  Before, the check ran only for a value carrying a `Text<N>`, so
  `let (a, b) = 5` compiled and faulted at run time with
  `cannot tuple-index Int`. A program with such a pattern no longer compiles.
  Pinned by `tests/let_pattern_shape.rs`.
