- **A whole-array store into a data field is refused.** `d.a = [..]` on an array field
  wrote the whole array into the field's first slot and left the others unwritten, so the
  program compiled and verified and then read wrong values or faulted. A whole-array read
  was already refused for the same layout. Pinned by `tests/data_array_store.rs`. A
  pykeleusma finding.
