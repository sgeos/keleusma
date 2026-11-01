- **A `loop` called from a `loop` receives the resume value in its parameter.**
  `Vm::resume` wrote the host's input into the parameter slot of the base frame,
  which belongs to the caller and is never resumed once a `loop` delegates to
  another `loop`. The callee's `Reset` had cleared its own parameter, so every
  iteration after the first read `Unit` there. A tuple built from it faulted with
  `flat scalar codec: OutOfBounds` or stored zeros into `private data`. The input
  now goes to the innermost running `loop` and is checked against that loop's own
  parameter type, so a mismatched callee fails with a `TypeError` at the resume
  boundary. A program without delegation is unaffected. The fix restores the
  soundness premise of the definite-initialization verifier, that each resume of
  a stream with a parameter writes slot 0 of that stream. A delegated `loop` now
  also sees each mid-iteration resume value in its parameter, as the entry `loop`
  does, which is a reading of the language design that awaits operator
  confirmation. Pinned by `tests/delegated_loop_reset.rs`.
