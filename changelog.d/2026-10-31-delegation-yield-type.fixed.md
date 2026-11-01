- **A delegated callee must yield and resume at the entry's types.** A
  `yield` or `loop` function called from `loop main` suspends the whole call
  stack when it yields, so the host receives the callee's yielded value as
  the entry's output. The type checker held each `yield` only to its own
  function's return type, so `loop child() -> bool { yield true }` called
  under `loop main(t: Word) -> Byte` yielded a `bool` to a host that decodes
  a `Byte`. A call or pipe into a `yield` or `loop` function whose return
  type, instantiated at the call, differs from the entry's return type is now
  refused at compile time, as is a call into a `loop` function whose first
  parameter, its declared resume type, differs from the entry's first
  parameter. Such a program, which compiled before, no longer compiles. A
  pipe into a function is now held to the category-call discipline as the
  plain call is, so a `fn` can no longer reach a `yield` function through a
  pipe. The self-hosted pipeline refuses the same programs through its
  cross-check against the reference. Pinned by
  the `delegation` module of `tests/yield_resume_type.rs`.
