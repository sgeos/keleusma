- **A host composite input of the wrong size is refused.** A tuple, struct or array passed to the
  entry function or as a resume value was packed in the host's field order and read by the
  script's layout with no check, so a missing, extra or wider field shifted every later field. The
  VM now compares the packed body's kind and byte length with the chunk signature and refuses a
  mismatch with a `TypeError` before any instruction runs. Two fields of equal width in the
  opposite order are still not detected, which the documented host contract covers. On the
  operator's ruling of 2026-10-30. Pinned by `tests/host_composite_shape.rs`.
