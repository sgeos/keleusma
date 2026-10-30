- **A `let` annotated with a generic instance type compiles.** `let b: Box<Word> = ...`
  and the same with a generic enum failed after monomorphization, which renamed the value's
  type and not the annotation. Pinned by `tests/generic_instance_annotations.rs`. A
  pykeleusma finding, whose host-built layout part remains open.
