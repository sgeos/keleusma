- **A `yield` expression has the type the host resumes with.** Inside
  `loop main` and every `yield` function, the value of `yield v` now has the
  type of the first parameter of `loop main`, which `Vm::resume` already
  checks the resume value against. So `let r = yield v; r.y` compiles with `r`
  of the parameter's type instead of faulting when the field is read. When
  `loop main` declares no parameter, or `main` is not a loop, nothing checks
  the resume value, and a field read or destructuring of a resume value that
  no use or annotation types is refused at compile time with a request for an
  annotation. A program whose annotation or use disagrees with the parameter
  type, such as `let r: Word = yield v` under `loop main(p: P)`, or which
  yields the resumed value as another type, no longer compiles. A loop's
  discarded tail value is admitted at the parameter type as well as at the
  return type, so a tail `yield` still compiles. Pinned by
  `tests/yield_resume_type.rs`.
