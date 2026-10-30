- **`keleusma compile` refuses a program whose worst-case memory bound cannot be computed.** Such a
  module, for example one that recurses, could be loaded by no host at any arena capacity, and
  `compile` used to write it anyway. It now exits with an error naming the verifier's reason and
  writes nothing. A module whose bound is computable but larger than the default arena is still
  emitted, for a host with a larger arena. On the operator's ruling of 2026-10-30. Pinned by
  `keleusma-cli/tests/print_memory_bound.rs`.
