- **The hot-swap schema hash covers each data slot's declared type.** It covered slot names and
  visibility only, so a swap between modules whose data fields shared names but not types, a
  reordered struct or a `Word` field become `Byte`, passed `Vm::replace_module` and then read
  the carried values by the wrong layout. Every module with data now carries a different
  `schema_hash`, so a swap between a module compiled before this change and one compiled after
  is refused. The wire format is unchanged. On the operator's ruling of 2026-10-28. Pinned by
  `tests/typed_schema_hash.rs`.
