- **A private array lies at element width, and every admitted program's declared private
  size and persistent figure shrink.** This is packed private storage, S1 of the array
  wire-slot design, ruled on 2026-10-31. A private array of more than one element whose
  elements do not start as `Unit` stores a fixed-width scalar element at its own width,
  eight bytes for a `Word` on the default host target and one for a `Byte` or `bool`,
  where every element took a 32-byte `Value` slot before, and a private array of flat
  composites keeps its pool bodies and drops its per-element handles. A private scalar and
  every other private field keep a `Value` slot per element. The module header's
  `private_data_bytes` and `required_persistent_capacity_for` follow, by
  `DataLayout::private_data_bytes` and `DataLayout::private_storage_extent`. Measured on the
  fifteen stage sources, the declared private data falls from 13,969,728 bytes to 3,515,784,
  74.8 percent less. `auto_arena_capacity_for` is unchanged for every stage and stays tight,
  pinned by `tests/persistent_data.rs` and `tests/private_composite_slot_cap.rs`. No wire
  byte changes, since the packing is derived from the field records alone.
- **A packed element refuses a value of another kind.** Writing, for example, a `Byte`
  into a private `[Word; N]` array now fails with `VmError::TypeError`, where a `Value`
  slot stored it before. Compiled programs never do this. A hot swap seed that a packed
  element cannot hold is refused before the swap commits, and the running module is kept.
- **`Module::declares_private_data` replaces the `private_data_bytes != 0` test** in the
  ephemerality rule, since a module whose only private field is a composite array now
  declares zero private data bytes and still holds persistent state.
- **The native route keeps its private image.** The native backend lays its private image
  out against `native_region::native_private_base_bytes`, the element-slot figure that
  `required_persistent_capacity_for` returned before packing, so every native memory image
  and offset is unchanged and `native_route_buffers_for` reports the same private figure.
