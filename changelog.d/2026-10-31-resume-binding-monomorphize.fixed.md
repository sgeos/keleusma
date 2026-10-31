- **A generic call on an unannotated resume binding is specialized.**
  `let r = yield v; yield r |> id()` failed with `undefined function `id``
  after monomorphization once another instance of `id` existed, and kept the
  generic chunk when none did, while `let r: Word = yield v` produced
  `id__Word`. Monomorphization now types a `yield` in a `yield` or `loop`
  function by `loop main`'s first parameter, the type the type checker
  already infers for it, so both forms compile to the annotated form's
  module. When no `loop main` parameter declares the resume type and no use
  fixes it, a generic call on that value is refused with the type checker's
  request for an annotation. Such a program, which compiled before with an
  unspecialized generic chunk, no longer compiles. The self-hosted compiler
  compiles the inferred forms byte-identically. Pinned by
  `tests/pipe_generic_monomorphize.rs`.
