- **A native's declarations must agree.** Two contradictory `use` declarations of one
  native compiled, and so did a declaration of a bundled library native that differs from
  the library's own, so the result was decoded by the wrong layout. Both are now type errors,
  and prepending a library's `SIGNATURES` now catches a mismatch as documented. Pinned by
  `tests/native_declaration_agreement.rs`. A pykeleusma finding.
