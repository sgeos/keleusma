- **The data layout is one record per declared field on the wire and in memory.** The
  operator chose Encoding F on 2026-10-31. The `DATA_FIELDS` region (`0x0027`) holds one
  thirty-two-byte record per field, and replaces `DATA_SLOTS`, `SHARED_LAYOUT`,
  `PRIVATE_COMPOSITE` and `DATA_INIT` (`0x001A` to `0x001D`). The artifact size of a data
  layout no longer depends on any array's element count, for every element kind and
  visibility, where a private `Byte` array cost sixteen bytes and a private array of
  composites twenty-four bytes per element before. `BYTECODE_VERSION` stays `2` and the
  format fingerprint is unchanged. Pinned by `tests/wire_schema.rs`.
- **An artifact carrying a retired data-layout region is refused rather than read as a
  module with no data.** `SchemaError::RetiredRegion` names the kind, and the virtual
  machine, the owned decoder and the self-hosted decoders all refuse it.
- **`DataLayout` is field-based, a breaking change to a public type.** `DataLayout` holds
  `fields: Vec<DataField>`, each with its name, visibility, first slot, element count, kind,
  element body length, offset, stride and initialiser. `DataSlot` and
  `PrivateCompositeSlot` are removed. Slot-level views are methods, such as
  `slot_count`, `shared_slot`, `private_composite_offset` and `private_init_values`.
  Addressing is unchanged, each element still occupying one logical slot.
- **A private composite element above slot 65,535 now has a pool home.** The `u16` slot of
  the retired placement record is gone, so the compile-time refusal pull request 652 added
  for `[P; 65537]` is lifted, and an element above slot 65,535 survives RESET at its own
  offset. Pinned by `tests/private_composite_slot_cap.rs`. A private composite element whose
  flat body exceeds 65,535 bytes is now refused at compile time, as a shared one already
  was, since the field record carries one element's body length in sixteen bits.
- **The typed verifier checks the field invariants, and so does the self-hosted twin.** The
  fields must tile the slot space, shared fields must precede private ones, a private zero
  initialiser needs a kind with a zero, and pooled private fields must be ascending,
  disjoint and within the pool. The verifier soundness invariant preserved is that every
  data access the runtime resolves lands inside the buffer or pool its field names, now
  checked per field rather than per element.
