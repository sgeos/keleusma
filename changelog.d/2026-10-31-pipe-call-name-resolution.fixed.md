- **A pipe and a plain call resolve their target name identically.** A
  local binding shadows a top-level function or native of the same name, and
  no local is callable. The plain call `inc(3)` under `let inc = 5` was
  refused while the pipe `3 |> inc()` called the function, and for a generic
  `id` both `id(3)` and `3 |> id()` called the function, because
  monomorphization renamed the call before the compiler saw the local. The
  type checker now refuses both forms with the same message wherever a local
  of the target's name is in scope, whether a `let`, a parameter, a `match`
  binding or a loop variable, and both forms call the function wherever none
  is. A program that piped into or called a shadowed generic function, which
  compiled before, no longer compiles. The self-hosted type stage already
  refused these forms. Pinned by the `name_resolution` module of
  `tests/pipe_generic_monomorphize.rs` and
  `delegation_types_and_shadowed_pipe_targets_have_their_pinned_outcomes` in
  `tests/selfhost_full_language.rs`.
