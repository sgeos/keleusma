- **A handled indexed write binds a composite element's `old` value as it was
  before the write.** A private composite element's body lives at a fixed
  offset in the persistent pool, and reading the slot yields a handle to that
  body, so `ok(old, new)` on a struct, tuple or enum element observed the new
  value once the write overwrote the body in place. The reference compiler now
  copies the element into a fresh arena body before the write, through a
  one-element array that the worst-case memory bound already charges. No
  opcode, wire format or `BYTECODE_VERSION` changes. The self-hosted compiler
  refuses a handled write on a composite element, so it has no twin to change.
  Pinned by `tests/indexed_write_handlers.rs`.
