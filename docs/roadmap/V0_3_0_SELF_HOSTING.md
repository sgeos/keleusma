# V0.3.0: Self-Hosted Compiler

> **Navigation**: [Roadmap](./README.md) | [Documentation Root](../README.md)

**Status**: Strategy (research pass complete; implementation not started)

## Goal

A Keleusma compiler written in Keleusma source, compiled to Keleusma bytecode, running on the Keleusma virtual machine, producing Keleusma bytecode as output. The endpoint is a fixed point: the self-hosted compiler compiled by the Rust-hosted compiler produces bytecode identical (modulo non-essential ordering) to what the Rust-hosted compiler produces from the same source, and the self-hosted compiler compiled by itself reproduces its own bytecode.

This document is a strategy, not a milestone tracker. The architectural endpoint is the subject; the bootstrap mechanism and the schedule are not.

## Why self-hosting matters

Self-hosting a language is the most credible demonstration that the language is expressive enough to write its own toolchain. The signal is twofold. First, it validates the surface language and the type system against a concrete, complex program of substantial size. Second, it removes a dependency: a self-hosted Keleusma can evolve without forcing every change through the Rust-hosted compiler maintainers. Defense and aerospace customers in particular value a toolchain whose dependency graph is short and auditable; a self-hosted compiler with no external compiler dependency is materially closer to a certifiable shape.

For Keleusma specifically, the self-hosted compiler is a precondition for V0.4.0 (machine-code generation). The V0.4.0 plan compiles the self-hosted compiler to native code via LLVM and links it as a static library against a Rust host, removing the VM from the compilation path for hosts that prefer ahead-of-time compilation. Without the V0.3.0 self-hosted compiler, V0.4.0 has nothing to compile to native code.

## Prior art

A research pass surveyed the single-pass and stream-processor compiler traditions. The most relevant precedents:

- **Per Brinch Hansen's pipeline-of-processes compilers.** Brinch Hansen advocated decomposing compilers into concurrent processes (lexer, parser, semantic analyzer, code generator) communicating through bounded queues. The architecture produces compilers whose working memory is bounded by the buffer sizes between stages, not by the program size. His *Brinch Hansen on Pascal Compilers* (Prentice-Hall, 1985) is the canonical reference; his SuperPascal compiler was itself written in this style and demonstrated the pattern of "stream-processor compiler in a stream-processor language." This is the architectural precedent closest to Keleusma's intended design.

- **Niklaus Wirth's single-pass compilers.** PL/0 (1976), the original Pascal compiler (1970), Modula-2 (late 1970s), Oberon (1987-1989). Wirth designed each successive language to be single-pass compilable, with declare-before-use rules and explicit `forward` declarations for mutual recursion. The Oberon compiler is approximately 4000 lines of Oberon and is published in full source in *Project Oberon* (Addison-Wesley 1992; revised 2013 edition available as a PDF from Wirth's ETH archive). Wirth's *Compiler Construction* (Addison-Wesley, 1996) is the canonical pedagogy. Wirth's tradition demonstrates that single-pass discipline survives language evolution; it is not only viable for tiny pedagogical languages.

- **Turbo Pascal 1.0 through 3.0 (1983-1986).** Anders Hejlsberg's compiler, written in 8086 assembly. Compiled to memory and ran the code from memory; no traditional link step for the default in-memory build. Single-pass in the sense that no AST was constructed and no separate semantic-analysis pass ran. The headline productivity feature was compile-link-run cycle that fit in 64 KB of RAM. The internals were never released as open source; primary documentation is sparse. The Computer History Museum oral history of Hejlsberg is the most reliable secondary source. Turbo Pascal is the commercial proof that single-pass compilation produces compilers fast enough to change developer workflow, with the explicit trade-off that whole-program optimization is unavailable.

- **The C-family tradition is explicitly not relevant prior art.** GCC, Clang, lcc, and the PCC line are multi-pass and AST-based. They optimize for the opposite trade-off (heavy optimization at the cost of compilation speed and memory). The lcc compiler (Fraser and Hanson, *A Retargetable C Compiler*, Addison-Wesley 1995) is a useful counter-example to study for what multi-pass design looks like at small scale, but it is not the model V0.3.0 should follow.

See `docs/reference/RELATED_WORK.md` for citations integrated into the broader knowledge graph (pending; this document captures the new citations until that integration lands).

## Recommended architecture: decomposed stream-processor compiler

V0.3.0 implements the compiler as three coordinated stages, each a Keleusma `loop` function:

```
source bytes
   │
   ▼
┌────────┐       tokens        ┌────────┐        tokens-plus-      ┌──────────┐    bytecode
│ lexer  │ ──────yield/resume──▶│ parser │ ──────context─yield────▶│ compiler │ ────yield────▶
└────────┘                     └────────┘                          └──────────┘
```

Each stage is a Keleusma `loop` function in source. The lexer consumes source bytes and yields tokens. The parser consumes tokens and yields parsed declarations (the unit of work is a single top-level declaration, not the whole program). The compiler consumes parsed declarations and yields bytecode chunks plus the auxiliary body that the wire format expects.

The decomposition matches Brinch Hansen's pipeline model and matches Keleusma's coroutine semantics directly. Each stage's working memory is bounded by its local state plus the inter-stage buffer, both of which fit inside the per-Stream-to-Reset arena budget. The whole-program AST is never constructed: the parser yields each declaration as it completes; the compiler emits bytecode immediately and forgets the declaration. Symbol tables are per-scope and popped on scope exit.

The host application that drives the pipeline is a Rust program (or a Keleusma program once V0.4.0 lands) that resumes each stage as needed and handles the inter-stage flow control. The host's responsibilities are minimal: collect tokens emitted by the lexer, hand them to the parser, collect declarations emitted by the parser, hand them to the compiler, collect bytecode emitted by the compiler, and assemble the wire-format buffer.

### Why this is the recommended shape

Three reasons.

First, it composes cleanly with Keleusma's existing model. Each stage is a `loop` function, which Keleusma already admits. The yield/resume protocol is the inter-stage communication channel. No new language primitives are required. The bounded-WCMU guarantee falls out for each stage independently.

Second, it matches the demonstrated prior-art model. Brinch Hansen's compilers were written exactly this way and worked. The pattern is not speculative.

Third, it provides natural test points. Each stage can be tested in isolation by driving it with a synthesized input stream and inspecting the output stream. The Rust-hosted compiler already has a per-stage test surface (lexer, parser, type-checker, monomorphizer, emitter) that the self-hosted version can reuse with minor adaptation.

### Constraints on the surface language

The self-hosted compiler must be expressible in the V0.2.0 (or V0.3.0-adjusted) Keleusma surface. Three surface-language tensions surface immediately:

1. **Recursion.** The self-hosted compiler will want to walk recursive data structures (parsed declarations contain expressions that contain sub-expressions; types contain sub-types). Keleusma forbids recursion in `fn` and `yield` categories; only top-level `loop` admits cyclic execution through productive yield. The classical resolution is to walk recursive data using explicit stacks rather than recursive function calls. Brinch Hansen's compilers used this technique. The Wirth tradition handled the same constraint with recursive-descent parsers that exploited the fact that the recursion depth was bounded by the language's nesting depth, not the input size; Keleusma's recursion prohibition is stricter and requires explicit stacks. Whether to relax the recursion rule for the compiler, or implement explicit stacks, is an open question (see "Open questions" below).

2. **Hindley-Milner type inference.** The Rust-hosted compiler runs Robinson unification over a constraint graph that spans an entire function. This is a multi-pass procedure within a single function. A pure single-pass compiler in the Wirth tradition does not perform this kind of inference; the surface language typically requires explicit type annotations. The realistic V0.3.0 answer is one of: (a) the self-hosted compiler accepts a restricted surface language where every binding is annotated and Hindley-Milner is unnecessary; (b) the self-hosted compiler bounds inference to per-declaration scope, with the constraint graph held in arena memory bounded by the declaration's complexity; (c) a separate inference stage runs as a non-streaming `fn` function over a per-declaration constraint graph, sandwiched between the parser stage and the emitter stage. Option (b) or (c) is the most likely path; option (a) would require a parallel V0.3.0 surface language definition.

3. **Generics and monomorphization.** Monomorphization requires the compiler to see every call site of a generic function before it can know which specializations to emit. This is fundamentally a whole-program operation. The realistic V0.3.0 answer is to keep a small specialization table in the compiler's persistent state (across the entire compilation, not per-declaration), and emit specialized chunks lazily as new call sites are discovered. The specialization table grows with the number of distinct specializations, not the program size; in practice this is a small bound.

These three tensions are real but not blocking. Each has a known resolution in the prior-art literature.

### Surface-level features that V0.3.0 will likely need

The research pass identified several language-design levers from Wirth and Brinch Hansen's tradition that map cleanly to Keleusma:

- **Declare before use.** Already the Keleusma surface convention. Pre-declaration of identifiers eliminates most forward-reference machinery.
- **Explicit forward declarations for mutual recursion.** Keleusma does not currently admit recursion in `fn` and `yield` categories. If recursion is permitted for the compiler, an explicit `forward` declaration analogue is the cleanest path.
- **Bounded fixup tables.** A fixed-capacity buffer in the arena that holds (location, target-placeholder) pairs for forward jumps. Patched when the target address becomes known. This is universal across single-pass compilers.
- **Separate compilation with precomputed interfaces.** Modula-2's definition/implementation module split. Keleusma already has a notion of compiled bytecode artifacts; the V0.3.0 self-hosted compiler can consume a module's interface as a separate input stream and bound its working set independently of the imported module's size.

The research pass also flagged what V0.3.0 will likely *not* need: full-program AST construction, multi-pass constraint solving across function boundaries, source-level transformations (macros), and whole-program optimization. Keleusma's design already excludes most of these by construction.

## Documented alternative: integrated single-pass compiler

The Wirth tradition produced compilers that did not decompose into stages at all. The Turbo Pascal compiler, the Oberon compiler, and the various Modula-2 compilers ran the entire compile pipeline as a single recursive-descent parser that emitted bytecode (or machine code) directly during parsing. There was no token stream materialized between the lexer and the parser; the lexer was just a method on the parser that returned the next token on demand. There was no AST: each syntactic construct emitted its corresponding bytecode at the point in parsing where the construct was recognized.

This is the integrated single-pass alternative. Its appeal is speed: the Turbo Pascal benchmark of 10,000-30,000 lines per second on a 4.77 MHz 8088 in 1984, and the Oberon compiler at millions of lines per second on modern hardware, are the gold standards. The architecture has no inter-stage buffering and no stage-coordination overhead.

V0.3.0 documents this alternative but does not recommend it. The reason is that Keleusma's coroutine model rewards the decomposed pipeline shape: each `loop` function is a natural stage, and the bounded-WCMU guarantee falls out per-stage. An integrated single-pass compiler in Keleusma would either be a single very-long `loop` function (which the verifier might admit but which is awkward to test in isolation) or a function-call chain that cannot use recursion (which forces the explicit-stack discipline anyway). Neither shape is obviously better than the pipeline. Brinch Hansen's pipeline-of-processes maps directly to Keleusma's coroutine pipeline; the integrated single-pass maps to Keleusma awkwardly.

If V0.3.0 implementation surfaces a real reason to prefer the integrated form (for example, the inter-stage buffering cost dominates the per-stage compilation cost, or the testability advantage of the pipeline turns out to be illusory), the design is on the shelf and the migration is straightforward: collapse the three `loop` functions into one, drop the inter-stage `yield` boundaries, and inline the staging.

## Open questions

The following questions are deferred to implementation. They are not blockers for the strategy.

1. **Recursion in the compiler.** Does the self-hosted compiler relax the recursion-prohibition rule for itself (treating compiler-internal recursion as a controlled exception), or does it walk recursive data with explicit stacks throughout? The Brinch Hansen tradition used explicit stacks; the Wirth tradition used recursive-descent and bounded recursion. Keleusma's existing prohibition is stricter than either.

2. **Hindley-Milner inference scope.** Per-declaration, per-function-body, or per-expression? The Rust-hosted compiler uses per-function. A streaming compiler may need a tighter bound.

3. **Module-scale compilation.** Does V0.3.0 target single-file programs (every module compiled from scratch each time), or does it adopt a Modula-2-style separate compilation with precomputed module interfaces? The first is simpler; the second is closer to a production toolchain.

4. **Diagnostic quality.** Single-pass compilers historically have brittle error recovery. The V0.3.0 compiler's diagnostic quality target is "as good as the Rust-hosted compiler's, where possible." Whether the streaming architecture forces a quality regression is an open question.

5. **Self-validation.** The V0.3.0 compiler should be validated against the Rust-hosted compiler on a regression corpus: every test program in the existing test suite should produce equivalent bytecode under both compilers (modulo non-essential ordering). The mechanism is an integration test that runs both compilers and compares outputs.

## Out of scope

The following are explicitly out of scope for this strategy document.

- **The bootstrap mechanism.** How the self-hosted compiler is bootstrapped (cross-compile from the Rust-hosted compiler, then verify the bootstrap by recompiling under the bootstrapped compiler) is an implementation question, not an architecture question. The mechanism is well-understood in the prior art and does not need design work here. See *Project Oberon* (2013 edition) for a worked bootstrap example.

- **The implementation schedule.** Whether V0.3.0 lands in calendar quarter X or Y depends on capacity that this document cannot allocate. The strategy is correct regardless of when the implementation begins.

- **V0.4.0 and beyond.** Native-code generation, vintage-processor targets, and the Keleusma-host model are V0.4.0+ concerns. They are referenced here only because V0.3.0 is a precondition for them; their design lives in a separate strategy document.

- **Keleusma-host and Keleusma-VM self-hosting.** Compiling the runtime VM itself in Keleusma is a V0.5+ aspiration. It is mentioned here because V0.4.0's native-code-generation precondition opens the door, but the design is not within V0.3.0's scope.

## References

The strategy draws on the following published works. Citations include ISBN and ACM/IEEE catalog identifiers where applicable so readers can independently verify.

- Per Brinch Hansen, *Brinch Hansen on Pascal Compilers*, Prentice-Hall, 1985, ISBN 0-13-083098-4. The canonical reference for the pipeline-of-processes compiler architecture.
- Per Brinch Hansen, "SuperPascal: A Publication Language for Parallel Scientific Computing", *Concurrency: Practice and Experience*, 6(5), 1994, pp. 461-483. Stream-processor compiler in a stream-processor language.
- Per Brinch Hansen, "The Programming Language Concurrent Pascal", *IEEE Transactions on Software Engineering*, SE-1(2), 1975, pp. 199-207.
- Niklaus Wirth, *Compiler Construction*, Addison-Wesley, 1996, ISBN 0-201-40353-6. The PL/0 pedagogy and the principles of single-pass compilation. PDF available from Wirth's ETH home page.
- Niklaus Wirth and Jürg Gutknecht, *Project Oberon: The Design of an Operating System and Compiler*, Addison-Wesley, 1992; revised edition 2013. The full Oberon compiler source is published in the revised edition.
- Niklaus Wirth, *Programming in Modula-2*, Springer-Verlag, 1982. The separate-compilation model.
- Niklaus Wirth, "The Programming Language Pascal", *Acta Informatica*, 1(1), 1971, pp. 35-63. The original Pascal report.
- Kathleen Jensen and Niklaus Wirth, *PASCAL User Manual and Report*, Springer-Verlag, 1974. The widely-distributed Pascal description.
- Steven Pemberton and Martin Daniels, *Pascal Implementation: The P4 Compiler*, Ellis Horwood, 1982. The portable Pascal compiler internals.
- Christopher W. Fraser and David R. Hanson, *A Retargetable C Compiler: Design and Implementation*, Addison-Wesley, 1995, ISBN 0-8053-1670-1. The lcc compiler; a multi-pass small C compiler studied for comparison.
- Donald E. Knuth, *Literate Programming*, CSLI Lecture Notes 27, Stanford University, 1992, ISBN 0-937073-80-6.
- Guy L. Steele Jr. and Richard P. Gabriel, "The Evolution of Lisp", *ACM SIGPLAN Notices*, 28(3), 1993, pp. 231-270. The incremental-compilation tradition.
- Computer History Museum, oral history of Anders Hejlsberg. Available at computerhistory.org. The most reliable secondary source for Turbo Pascal internals; primary sources are not publicly available.

The Turbo Pascal architectural claims in this document carry an uncertainty flag because the source code was never released and primary-source documentation is sparse. The strategy does not depend on Turbo Pascal specifics; the citation establishes commercial viability of the single-pass discipline without depending on internal details.

The Oberon compiler line-count claim (approximately 4000 lines of Oberon source) is drawn from general circulation in the literature and should be verified against the published source listing in *Project Oberon* (2013 edition) before being cited as a target in implementation documentation.
