- **The hot-swap schema hash is one canonical line per data field.** The operator chose
  this form, H2, on 2026-10-31. An array field's type token is `[leaf;count]`, so resizing,
  retyping or reordering an array changes the hash, and the hash costs work in proportion to
  the field count rather than the element count. Every schema hash of a module with an array
  field changes, so a hot swap between a module compiled before this change and one compiled
  after it is refused as a schema mismatch whenever either declares an array. Pinned by
  `tests/typed_schema_hash.rs`.
