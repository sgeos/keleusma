- **A trait impl for `Text<N>`, `Fixed` or `Multiword<n>` is callable.** The
  type checker registered such an impl under a key carrying the dimension, such
  as `Text<4>`, while method lookup, trait-bound checking and the compiler keyed
  it by the family name, such as `Text`. The impl was admitted and could never be
  called, either as a method or through a bound, and a call was refused with
  "type `Text` has no method". An impl now belongs to the family, as the lookup
  already assumed. A call on a receiver of another capacity than the impl's is
  refused as a type mismatch. A second impl of one trait within one of these
  families is now refused at compile time, because both would mangle to the same
  name and the later one would silently replace the earlier one. Pinned by
  `tests/generic_methods.rs`, module `dimensioned_families`.
