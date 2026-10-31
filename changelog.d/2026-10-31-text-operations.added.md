- **The bundled text operations return as compiler intrinsics over `Text<N>`.** Behind the new
  off-by-default `text` cargo feature, `text::len`, `text::capacity`, `text::concat`, `text::eq`
  and `text::slice` lower to existing opcodes with a static cost that is linear in the capacity.
  `text::concat` of a `Text<A>` and a `Text<B>` is a `Text<A + B>`, so it cannot overflow.
  Lengths and slice bounds count bytes, and `text::slice` clamps its range rather than faulting.
  No opcode, native or wire format change is involved. `keleusma-cli` enables the feature. A
  build without it refuses a call under `text::` at compile time, and `use text::..` is refused
  in every build, so a host native can no longer be imported under that path. Pinned by
  `tests/text_ops.rs` and `tests/text_ops_without_feature.rs`. The verifier soundness invariant
  preserved is that every admitted module has a static worst-case execution time and memory
  bound, which each operation contributes to as a fixed instruction sequence.
