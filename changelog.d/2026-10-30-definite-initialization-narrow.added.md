- **The verifier refuses a read of a local that no path writes.** A forward may-analysis over each
  chunk computes the locals some path writes before every instruction, and a `GetLocal` of any
  other local is refused, including a stream local written only before `Stream` and read after it,
  which every pass after the first would read as the empty-slot `Unit`. The rule is narrowed to
  what the compiler already guarantees, so a local written on one path only and composite private
  data are not checked. On the operator's rulings of 2026-10-28 and 2026-10-30. Pinned by
  `tests/definite_initialization.rs`.
