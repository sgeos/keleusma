- **A private composite slot beyond 65,535 is refused at compile time.** Each private composite
  slot's pool placement is recorded with a 16-bit slot index while the data slot space is 24 bits,
  and the compiler skipped any slot above the 16-bit range with no diagnostic, so a large private
  array of composites lost the placement of its later elements. The module is now refused, naming
  the field. Pinned by `tests/private_composite_slot_cap.rs`.
