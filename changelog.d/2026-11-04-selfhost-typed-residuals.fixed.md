- **The self-hosted typed stage closes the two open soundness residuals of the C5 review.**
  After a loop, `verify_typed.kel` now runs the reference's loop-head fixpoint, with the same
  cap of the local count plus two passes, and sets the locals the body writes and the operand
  entries above the loop's entry height to the join of the last pass's break states, where it
  forgot them before. `Text` layouts are now variable-length records in a 65,536-word layout
  heap instead of four offsets per entry. An element read from an array of flat bodies now
  takes the layout its elements share, as the reference's `element_text_layout` gives it. A
  layout heap past its capacity is unavailable analysis and never a verdict. The invariant
  preserved is that the self-hosted validator accepts nothing the reference rejects. On the
  enlarged mutation corpus, 20 mutants were accepted before the repair and none after it. That
  is a measured result for that corpus and not a proof for arbitrary bytecode. Pinned by
  `tests/selfhost_validator.rs` and recorded in
  `docs/decisions/C5_SELFHOSTED_VALIDATOR_REVIEW.md`.
