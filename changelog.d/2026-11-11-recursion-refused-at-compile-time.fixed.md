- **A program with a call cycle is refused at compile time, and `verify` refuses a module
  with one.** The grammar and the language design state that the compiler rejects any cycle
  in the call graph, and until now nothing before `Vm::new` did. A recursive program
  compiled, verified and carried a finite declared worst-case execution time, so a host
  that loaded it with `Vm::new_unchecked` received a module that need not terminate. The
  type checker now refuses a cycle among the calls it resolves, which are plain calls,
  pipes, trait methods and refinement predicates, with a diagnostic that begins
  `recursive call detected`, names the functions of the cycle in call order and carries
  the span of the call that closes it. `verify::verify` refuses a cycle in the bytecode
  call graph, and the new `verify::find_call_cycle` reports it. The self-hosted
  `verify_module_via_kel` refuses it at the same point, as stage `call cycle`. The
  soundness invariant restored is that every module `verify` accepts has an acyclic call
  graph, which the time and memory analyses assume. The memory analysis keeps its own
  refusal, and its message now names the cycle in place of
  `recursive call detected during WCMU topological sort`. The call graph walk is
  iterative, so a long call chain in bytecode from another producer no longer costs one
  native stack frame per chunk. Found by the proofs line's differential test. Pinned by
  `tests/recursion_prohibition.rs`.
