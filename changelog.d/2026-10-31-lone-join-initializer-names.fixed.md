- **The module-input joins see a composite private initializer.** `wire_names_via_kel`,
  `wire_regions_via_kel`, `wire_chunks_via_kel` and `wire_windowed_via_kel` built their
  input from the chunk constants alone, so for a module such as
  `examples/scripts/14_frame_log.kel` they wrote zeros in `NAMES` and `STRING_POOL` where
  the reference holds the names of the private `Frame` initializer. Their input now carries
  the reference's constant roots. A module whose constant names the joins would intern in
  a different order from the reference is refused rather than emitted. The shipping
  serializer, `wire_auxiliary_from_module_via_kel`, was not affected. Pinned by
  `tests/selfhost_wire.rs`.
