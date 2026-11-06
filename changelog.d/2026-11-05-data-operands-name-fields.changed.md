- **A data operand names a field, one logical slot per field.** This is option B of the
  array wire-slot design, ruled on 2026-10-31. `GetData(f)` and `SetData(f)` address
  element zero of field `f`, and `GetDataIndexed(f, len)` and `SetDataIndexed(f, len)`
  address element `i` of field `f` for an index below `len`, which is at most the field's
  element count. The runtime resolves an operand by one record read rather than by a
  binary search. The opcode stream of every module whose arrays precede another field
  changes, and its size does not, since the operands keep their twenty-four-bit encoding.
  No opcode is added, `BYTECODE_VERSION` stays `2` and the format fingerprint is unchanged.
  An artifact compiled before this change and loaded after it is not refused by either, so
  it must be recompiled, which the operator's ruling of 2026-10-31 on the within-cycle
  hazard accepts until the release rolls the fingerprint.
- **The host keeps its element indices, so no embedder changes.** The operator ruled on
  2026-11-04 that `Vm::get_shared`, `Vm::set_shared` and a hot swap's seeds keep addressing
  elements, and the virtual machine translates. A host written against element indices
  before this change reads and writes the same values, pinned by
  `tests/wide_shared_segment.rs`. `DataLayout::field_count`, `operand_slot`,
  `operand_span`, `element_addressed` and `field_index_of` expose the translation, and
  `DataLayout::slot_count` now documents itself as the host's element count rather than
  the bound of an operand.
- **A data span may not cross a field.** The structural verifier, in both the reference
  and the self-hosted twin, refuses a nonempty `GetDataIndexed` or `SetDataIndexed` span
  longer than the field its first operand names, where it previously refused only a span
  past the end of the data segment. An empty span stays valid up to and including the field
  count. The verifier soundness invariant preserved is that every data access the runtime
  resolves lands inside the field its operand names.
