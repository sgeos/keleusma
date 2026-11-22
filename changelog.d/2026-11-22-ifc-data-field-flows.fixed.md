- **Information-flow labels are enforced at data-field writes and annotated `let`
  bindings.** Both checks passed the flow rule its two types in reverse order, so a
  labelled value could be written into an unlabelled data field or bound to an
  unlabelled annotation, while a plain value could not be bound to a labelled
  annotation. A write under an `if` condition, a `match` scrutinee or a match guard
  that carries labels is now charged with those labels, so an implicit flow into a
  less labelled field is refused. The invariant restored is the one
  `docs/spec/GRAMMAR.md` states, `source.labels ⊆ target.labels` at every data-field
  write, including the labels of every governing condition. Not covered: a write
  inside a called function is checked in that function, without the caller's
  governing labels. Reported by a downstream host crate. Pinned by
  `tests/ifc_shared_field_labels.rs`.
