- **The typed verifier refuses a `Text` read from a field built as another kind.** It knew a
  flat body only by kind and size, so a hand-built module could read a `Text` field at an offset
  holding a `Word`, and the VM-internal read would rebuild a pointer from the integer. The pass
  now tracks the `Text` field offsets of every body the program builds and refuses such a read
  with `TextFieldMismatch`. This preserves the soundness invariant that a VM-internal `Text` read
  of a program-built body reads a pointer the program wrote as text. Bodies from outside the
  program are not yet covered, and the self-hosted twin is not yet built. On the operator's
  ruling of 2026-10-28. Pinned by `tests/flat_text_layout_proof.rs`.
