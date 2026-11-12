- **The memory bound refuses a slot width of zero and saturates its products.** No entry
  of `verify` that takes `value_slot_bytes` validated it. Zero divided by zero at a
  `Call`, and a large width overflowed the product of the slot count and the width, in
  `wcmu_stream_iteration_with_value_slot_bytes`, in
  `wcmu_whole_chunk_with_value_slot_bytes` and in the module analysis. Every public
  entry that takes the width now refuses zero with a `VerifyError`, the products
  saturate, the operand-stack offsets of the walk saturate, and a callee whose stack
  figure saturated saturates every caller, where dividing the saturated figure back into
  slots would have understated it. The soundness invariant restored is that a stack
  figure is never below the slot count times the width. A component of `u32::MAX` is
  saturated and is not a bound, and admission against an arena compares it as it stands.
  Found by the proofs line with Kani, as finding F2. Pinned by
  `tests/bound_arithmetic_saturation.rs`.
