- **The definite-initialization rule proves that no read observes the empty-slot `Unit`.**
  The compiler now writes every local on every path before any read. The handled indexed write
  writes each bound `old` and `new` slot with the element's zero before its flag test, which
  costs two instructions per bound `Word` slot and three per `Byte` slot. Every private data slot
  now starts as its type's zero, a composite its canonical zero and `Text` the empty static
  string, where it formerly started as `Unit`. A hot swap that seeds a private slot with `Unit`
  gives it the new module's initializer. The verifier's rule for locals becomes a must-analysis,
  so a local that some path leaves unwritten is refused at its read, and a private slot whose
  initializer is still `Unit`, a slot of type `()` or one whose zero would violate a refinement,
  must be written on every path before its read. The soundness invariant strengthened is that a
  verified module never reads the empty-slot marker from a local or from private data, which
  the narrowed rule held only for a local that no path writes. The self-hosted compiler emits
  the same bytes within its subset, and its validator twin decides the strengthened rule for
  locals. A private array of structs or a private tuple field now differs from the reference's
  bytes, so the cross-check refuses it. The rule for private data has no self-hosted twin yet. On the
  operator's ruling of 2026-10-30. Pinned by `tests/definite_initialization.rs` and
  `tests/selfhost_definite_initialization.rs`.
