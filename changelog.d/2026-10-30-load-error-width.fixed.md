- **Refusing a header width exponent of 32 or more no longer panics.** `LoadError`'s message for a
  word, address or float width mismatch computed `1 << exponent` from the untrusted header byte,
  which overflowed under overflow checks, so `Vm::load_bytes` and the signed loader panicked while
  formatting a correct refusal. Such an exponent is now written as a power of two. Found by the new
  fuzz targets. Pinned by `tests/fuzz_corpus_replay.rs`.
