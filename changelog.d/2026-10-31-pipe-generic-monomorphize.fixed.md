- **A pipe into a generic function is monomorphized as the plain call is.**
  `t |> id()` compiled the generic `id` as one chunk with `Opaque` parameter
  and return kinds, while `id(t)` produced the specialization `id__Word`. The
  reference compiler now rewrites a pipe whose target is a generic function
  into the call it stands for before type checking, so the two forms compile
  to the same module. A pipe into a non-generic function or a native is
  unchanged. A generic call whose argument is a pipe, `x |> d() |> id()`,
  is typed by the piped function's return and specializes as `id(d(x))`
  does. The self-hosted compiler instantiates a pipe into a generic function
  the same way, byte-identically, including the placeholder form. Pinned by
  `tests/pipe_generic_monomorphize.rs` and
  `pipes_into_generic_functions_compile_byte_identically` in
  `tests/selfhost_full_language.rs`.
