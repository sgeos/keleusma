- **The self-hosted validator no longer accepts 41 corrupted modules the reference rejects.**
  The C5 review's mutation corpus of 10,703 corrupted modules found them, all in the typed
  stage `verify_typed.kel`. A loop forgot every local and a join forgot every local and
  every operand entry, where the reference keeps a shape every path agrees on, and a
  native's error flag was pushed as an unknown rather than as a `Word`. The stage now joins
  per entry at an `EndIf` and checks a loop body from its entry state. The invariant
  preserved is that the self-hosted validator accepts nothing the reference rejects, which
  now holds over that corpus and still fails for the two residuals the review names. The
  shipping reference gate is unchanged. Pinned by `tests/selfhost_validator.rs` and recorded
  in `docs/decisions/C5_SELFHOSTED_VALIDATOR_REVIEW.md`.
