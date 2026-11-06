- **The self-hosted compiler accepts `Byte` and `Word` literal suffixes.** Its lexer read
  `2Byte` as the integer 2 followed by the name `Byte`, so the parser reported an undefined
  identifier and the type stage rejected a program the reference accepts, such as
  `let t = (1, 2Byte)`. A `Byte`-suffixed literal now compiles to the reference's byte
  constant, and a `Word` suffix changes nothing. Pinned by the `suffix` cases of the
  construct-support boundary in `tests/selfhost_codegen.rs`.
