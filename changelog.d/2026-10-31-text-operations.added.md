- **The bundled text operations return as compiler intrinsics over `Text<N>`, called as
  methods.** Behind the new off-by-default `text` cargo feature, `t.len()`, `t.capacity()`,
  `a.concat(b)`, `a.eq(b)` and `t.slice(start, end)` lower to existing opcodes with a static cost
  that is linear in the capacity. They are resolved by the receiver type `Text<N>` before any
  trait is consulted, so a method of the same name on any other type still resolves to its trait
  impl. `a.concat(b)` of a `Text<A>` and a `Text<B>` is a `Text<A + B>`, so it cannot overflow.
  Lengths and slice bounds count bytes, and `t.slice` clamps its range rather than faulting.
  Every operation gives a defined result for every operand body, including one a host supplies
  with a negative length, a length above `N`, or nonzero bytes past the length, by reading the
  effective length `min(max(len, 0), N)` and treating every byte at or past it as zero, and every
  result has a length in `[0, N]` and zero padding. No opcode, native or wire format change is
  involved. `keleusma-cli` enables the feature. A build without it refuses one of these methods on
  a `Text<N>` at compile time naming the feature. The free-function spelling under `text::` that
  briefly preceded the methods during this release cycle is gone, and the `text` path is not
  reserved, so a host native may still be imported under it. Pinned by `tests/text_ops.rs` and
  `tests/text_ops_without_feature.rs`. The verifier soundness invariant preserved is that every
  admitted module has a static worst-case execution time and memory bound, which each operation
  contributes to as a fixed instruction sequence.
