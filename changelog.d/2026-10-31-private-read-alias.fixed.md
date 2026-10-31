- **A private composite value read before a write to its slot keeps the value
  it was read with.** A private data slot holding a struct, tuple, enum or
  array element keeps its body at a fixed offset in the persistent pool, and
  reading the slot yields a handle to that body, which a later write to the
  slot overwrote in place. So `let o = d.s; d.s = P { x: 1, y: 2 }; o.x * 10 +
  o.y` returned 12 where 34 is correct, and `let o = d.x[1]` followed by a
  write to `d.x[1]` likewise. Under the operator ruling of 2026-10-31 the
  compiler now copies such a read right after it, and only where a write to an
  overlapping slot, a call or a yield can follow the read while its value is
  live. A read reduced to a scalar at once, a value whose last use precedes
  every such write, and a shared slot are not copied. The rule is decided over
  each chunk's copy-free ops and is documented in `src/compiler/alias_copy.rs`.
  One copy costs fifteen nominal cycles and adds twice the element's byte size
  to the worst-case heap bound, and the self-hosted compiler emits the same
  copies byte for byte or refuses the program. A function needing more than 64
  copies, or a program with more than 64 private struct slots, is refused by
  the self-hosted compiler. No opcode, wire format or `BYTECODE_VERSION`
  changes. Pinned by `tests/private_read_alias.rs` and
  `tests/selfhost_private_read_alias.rs`.
- **Writing a private composite slot's own value back to it is defined.** The
  runtime copied the body into the pool with `copy_nonoverlapping`, which is
  undefined behaviour when the value is a handle to that same body, as verified
  bytecode `GetData(s); SetData(s)` makes it, and a debug build aborted. The
  copy now permits overlap.
