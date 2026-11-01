- **A refutable `let` pattern is refused at compile time.** `let 3 = 5`
  compiled and silently matched nothing, because the compiler bound any
  pattern other than a name, a wildcard or a tuple as one hidden local. The
  type checker now refuses a literal pattern, a variant pattern of an enum
  with more than one variant, `Option::Some` and `Option::None`, and any
  tuple or struct pattern holding one of these, with a message that points to
  `match`. A variant pattern of a single-variant enum is irrefutable and now
  binds its payload, `let W::A(x, y) = w`. A program with a refutable `let`
  pattern no longer compiles. The self-hosted compiler already refused every
  refutable form, and it refuses the single-variant enum form as outside its
  subset. Pinned by `tests/let_pattern_irrefutable.rs`.
