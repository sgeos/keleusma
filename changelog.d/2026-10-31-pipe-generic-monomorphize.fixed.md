- **A pipe into a generic function is monomorphized as the plain call is.**
  `t |> id()` compiled the generic `id` as one chunk with `Opaque` parameter
  and return kinds, while `id(t)` produced the specialization `id__Word`. The
  reference compiler now rewrites a pipe whose target is a generic function
  into the call it stands for before type checking, so the two forms compile
  to the same module. A pipe into a non-generic function or a native is
  unchanged. The self-hosted compiler still refuses a pipe into a generic
  function, so the two compilers disagree on admitting it but not on any
  bytes. Pinned by `tests/pipe_generic_monomorphize.rs`.
