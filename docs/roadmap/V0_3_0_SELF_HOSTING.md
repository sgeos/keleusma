# V0.3.0: Self-Hosted Compiler

> **Navigation**: [Roadmap](./README.md) | [Documentation Root](../README.md)

**Status**: Implementation under way in [`compiler/`](../../compiler/README.md); see [`compiler/MILESTONES.md`](../../compiler/MILESTONES.md) for the running log. **Five stages** now exist in Keleusma source and each **self-compiles byte-identically** to the Rust-hosted compiler: `lexer.kel`, `parse.kel`, `reconstruct.kel`, `codegen.kel`, and `analyze.kel`. The postorder-record-to-forest reconstruction that was host-side Rust (`reconstruct_into`) is now the Keleusma stage `reconstruct.kel`, so the compile path is Keleusma from lexing through code generation with the host only moving data between stages. The two remaining gaps recorded here earlier are also closed for the loop-free-and-call-shallow case: the emitted module no longer borrows its data layout, enum layouts, typed-verifier signatures, schema hash, chunk-table metadata, or WCET/WCMU header from the Rust reference; the driver assembles every one of these from the stage output, and the whole serialized module is proved byte-identical to the reference (the reference is used only as the comparison oracle).

The worst-case resource analysis and its validator are now self-hosted too. `analyze.kel` reformulates the Rust verifier's recursive WCET/WCMU control-flow traversal (`verify.rs::wcet_region`/`wcmu_region`) as an explicit region-frame stack, folds both analyses into one walk, and self-hosts the loop iteration-bound extraction including the C7 induction-advance soundness check. Given an arena capacity it emits a validation verdict — a provable finite bound whose stack-plus-heap budget fits — and it folds transitive-call WCMU (each callee's bound resolved in topological order and folded at every `Op::Call`, plus the composite-shared-read copy-out), so `validate_module_via_kel` is a **drop-in replacement for `verify_resource_bounds`** on a whole call-bearing module, proved to match at capacities below/at/above the budget for the four stage modules, synthetic call chains, and a composite-shared program. Two honest limitations remain: the one unmodelled WCMU term is the text-size string-allocation heap (zero for every text-free program, so the drop-in holds for the entire text-free self-hosted compiler; a text-allocating program would need that pass self-hosted too), and the analysis reimplements audited safety-critical logic in a second language and so warrants independent review against `verify.rs`. Separately, the self-hosted pipeline does not yet compile a user-written `break;` statement nor reconstruct a conditional used as a call argument, so stages stay within that subset. The research pass below remains the authoritative design; this note records that the schedule has advanced from "three stages self-compiling" to "five stages self-compiling, module scaffold self-assembled, resource analysis self-hosted, and the resource-bound validator a drop-in replacement for text-free modules."

**Subproject**: The work happens in [`compiler/`](../../compiler/README.md), a standalone package scaffolded against this strategy. The three stages live in `compiler/kel/` (`lexer.kel`, `parser.kel`, `codegen.kel`) with shared shapes in `compiler/kel/prelude.kel`; the Rust host driver and bootstrap harness live in `compiler/src/`. The release-by-release plan mapping the V0.2.x line to V0.3.0 is [`compiler/MILESTONES.md`](../../compiler/MILESTONES.md). This document remains the authoritative design; the subproject is where it is realized.

## Goal

A Keleusma compiler written in Keleusma source, compiled to Keleusma bytecode, running on the Keleusma virtual machine, producing Keleusma bytecode as output. The endpoint is a fixed point: the self-hosted compiler compiled by the Rust-hosted compiler produces bytecode identical (modulo non-essential ordering) to what the Rust-hosted compiler produces from the same source, and the self-hosted compiler compiled by itself reproduces its own bytecode.

This document is a strategy, not a milestone tracker. The architectural endpoint is the subject; the bootstrap mechanism and the schedule are not.

## Why self-hosting matters

Self-hosting a language is the most credible demonstration that the language is expressive enough to write its own toolchain. The signal is twofold. First, it validates the surface language and the type system against a concrete, complex program of substantial size. Second, it removes a dependency: a self-hosted Keleusma can evolve without forcing every change through the Rust-hosted compiler maintainers. Teams that value a short, auditable toolchain dependency graph benefit in particular; a self-hosted compiler with no external compiler dependency is materially closer to an auditable shape.

For Keleusma specifically, the self-hosted compiler is a precondition for V0.4.0 (native code generation; see [V0_4_0_NATIVE_CODEGEN.md](./V0_4_0_NATIVE_CODEGEN.md)). The V0.4.0 plan compiles the self-hosted compiler to native code via LLVM and links it as a static library against a Rust host, removing the VM from the compilation path for hosts that prefer ahead-of-time compilation. Without the V0.3.0 self-hosted compiler, V0.4.0 has nothing to compile to native code.

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

## Incremental migration ordering

> **Superseded by the backward strategy.** The forward, lexer-first ordering below is retained for its rationale, but the operator-decided migration strategy now runs the stages **last to first**, codegen then parser then lexer, so the emit and wire boundary is de-risked first. It uses one moving throwaway adapter at the Rust-to-Keleusma frontier, abandons byte-identity in favour of structural module equality since rkyv is only a means, and tracks intermediate deviations in a deferral ledger that is driven to empty at completion. The authoritative statement of the strategy is [`compiler/MILESTONES.md`](../../compiler/MILESTONES.md); this section documents the earlier reasoning and the per-stage validation idea that the strategy still uses.

Reaching the all-Keleusma pipeline is a non-atomic transition. The recommended strategy is to migrate the stages one at a time, validating the regression corpus at each intermediate state. The Bootstrap procedure documented in the next section applies to the final migration step, not to each step.

**Step 1. Replace the lexer.** Implement `compiler/lexer.kel`. Wire it into the existing Rust-hosted pipeline through a native interface: the Rust parser consumes tokens emitted by the Keleusma lexer. The token-value handoff crosses the native boundary either as values that match the existing Rust `Token` shape directly, or as a small wire format that the Rust parser deserialises. Validation: every program in the regression corpus compiles to byte-identical bytecode under the Keleusma-lexer-plus-Rust-parser-plus-Rust-compiler configuration as under the all-Rust baseline.

**Step 2. Replace the parser.** Implement `compiler/parser.kel`. The Rust compiler now consumes `Declaration` values produced by the Keleusma parser, again across the native boundary. Validation: byte-identical bytecode under Keleusma-lexer-plus-Keleusma-parser-plus-Rust-compiler as under the all-Rust baseline.

**Step 3. Replace the compiler.** Implement the compiler stage in Keleusma source (`compiler/codegen.kel` plus type-inference and monomorphization helpers as separate `fn` modules or sub-functions, as the architectural decomposition warrants). The full pipeline now exists in Keleusma. The Bootstrap procedure below applies: Phase A produces the initial bytecode under the Rust-hosted compiler, Phase B self-compiles, Phase C reaches the fixed point.

This ordering has three properties that recommend it.

First, it reduces risk per step. At each intermediate state, two of the three stages remain the proven Rust implementation. A bug in the migrated stage manifests against a known-good downstream consumer, isolating the failure.

Second, it matches the natural complexity gradient. The lexer is the simplest stage: a finite-state byte scanner with a keyword table. The parser is middle complexity: recursive-descent over a context-free grammar, with the recursive-data-structures constraint discussed above. The compiler stage is the most complex: type inference, monomorphization, code generation, and verification interaction. Tackling the simplest first builds confidence in the cross-language boundary and the inter-stage data shapes before attacking the hardest stage.

Third, it lets each migration step prove out a specific concern. Step 1 proves the byte-iteration story (the "missing or limited" surface-language feature identified above). Step 2 proves the recursive-data-walking discipline (explicit stacks or whatever resolution is chosen for the recursion question). Step 3 proves the Hindley-Milner inference scope and the monomorphization specialization-table bound. Concerns surface and resolve sequentially rather than all at once.

Alternative orderings are possible but less attractive. A "compiler first" ordering would let the Keleusma compiler validate the upstream Rust stages by re-compiling them with itself, but it requires hand-authoring large `Declaration` test fixtures since no Keleusma parser exists yet to produce them from source. A "parser first" ordering inverts Steps 1 and 2 and faces the byte-iteration concern in a less isolated form (the parser depends on lexer output, so a Rust lexer driving a Keleusma parser still exposes the cross-language data-shape boundary at the same place). Neither alternative offers a compelling advantage over Lexer → Parser → Compiler.

The user-facing CLI gains a per-step toggle, e.g. `--lexer keleusma`, `--parser keleusma`, `--compiler keleusma`, during the migration. After Step 3 ships, the toggles collapse into the single `--self-hosted` flag recorded in the success criteria.

## Bootstrap procedure

Three phases. The pattern is canonical across Wirth's *Project Oberon*, LLVM, Rust, and Go. The procedure applies to Step 3 of the Incremental migration ordering above, at the point when all three pipeline stages exist in Keleusma source.

**Phase A. Cross-compile.** The self-hosted compiler is written in Keleusma source under `compiler/lexer.kel`, `compiler/parser.kel`, `compiler/codegen.kel`, plus shared AST and bytecode-encoding helpers. The existing Rust-hosted compiler produces its bytecode. The output is a Keleusma bytecode artefact, call it `kelc.0.kel.bin`, that runs on the VM and accepts Keleusma source as input.

**Phase B. Self-compile.** `kelc.0.kel.bin` is loaded into a VM instance and invoked against its own source files as its input. The output is `kelc.1.kel.bin`. If `kelc.0` is correct, `kelc.1` is byte-identical to `kelc.0` modulo non-essential ordering (map iteration order, etc.). Any divergence is a bug in `kelc.0`.

**Phase C. Fixed point.** `kelc.1.kel.bin` is loaded into a VM instance and invoked against the same source files. The output is `kelc.2.kel.bin`. `kelc.2` must be byte-identical to `kelc.1`. Fixed-point reached.

Validation runs alongside Phases B and C: every test in the existing Rust-side regression corpus is recompiled under both the Rust-hosted compiler and `kelc.1`. The bytecode outputs must be byte-identical (modulo the same non-essential ordering). Divergence on the corpus is a bug in the self-hosted compiler.

The bootstrap procedure is mechanical. It does not require additional design work; it is included here so the next session does not need to derive it from prior art. The risk in the bootstrap is not the procedure but the surface-language gap: the self-hosted compiler may need features the V0.2.0 surface does not yet provide ergonomically (see "Required surface-language features" below).

## Inter-stage data shapes

The pipeline's value depends on the inter-stage data being expressible as Keleusma values. The following shapes are starting points; the implementation may refine them.

**Lexer output (one yield per token):**

```
struct Token {
    kind: TokenKind,
    span: Span,
}

enum TokenKind {
    Word,
    Identifier(Text),
    Integer(Word),
    Float(Float),
    StringLiteral(Text),
    Punctuation(Byte),       // '(', ')', '{', '}', etc.
    Keyword(KeywordKind),    // fn, yield, loop, let, ...
    Eof,
}

struct Span {
    start: Word,             // byte offset
    end: Word,
    line: Word,
    column: Word,
}
```

**Parser output (one yield per top-level declaration):**

A `Declaration` enum covering the same surface forms the existing parser accepts: `Function`, `Struct`, `Enum`, `Newtype`, `Trait`, `Impl`, `Use`, `Data`. Each variant carries the fully parsed sub-tree. The parser yields a fully formed declaration; expressions and statements inside are recursive data structures that the parser builds on its local stack and emits as a complete sub-tree.

The "expressions are recursive data structures" point is the load-bearing constraint. Keleusma forbids recursion in `fn` and `yield` categories. The parser must either (a) build expression trees using an explicit stack rather than recursive calls, (b) be granted a recursion exception, or (c) be written as a `loop` function where the "recursion" is encoded as iteration over the input stream with a manual stack of parser states. Brinch Hansen's compilers used option (a). Wirth's used recursive descent within the language's natural depth bound. The choice is recorded as an open question; option (a) is the safest default.

**Compiler output (one yield per chunk):**

```
struct CompiledChunk {
    name: Text,
    ops: Array<u8>,                  // opcode-stream bytes
    operand_pool: Array<u8>,         // operand-pool bytes
    constants: Array<ConstValue>,
    metadata: ChunkMetadata,         // WireChunk-shaped
}
```

The host-side driver collects compiled chunks, builds the auxiliary body's rkyv-archived form, and assembles the wire-format buffer. The compiler stage does not emit the wire format directly; it emits chunk-level outputs and the host serialises them. This split keeps the compiler stage's working set bounded by chunk size, not module size.

**Inter-stage buffering.** Keleusma's yield/resume protocol provides implicit back-pressure: a downstream stage that has not resumed blocks the upstream stage's next yield. No explicit bounded-queue machinery is required. The buffer size is effectively one item per stage.

## Required surface-language features

The compiler-in-Keleusma needs the features below. The first group exists today and is sufficient. The second group exists but with caveats. The third group is missing or limited and may need to land in V0.2.x before V0.3.0 begins.

**Sufficient as of V0.2.0:**

- Structs, enums, pattern matching, multiheaded functions, guards.
- For-in loops over arrays and bounded ranges.
- Static string literals for keyword tables and diagnostic messages.
- `Byte` type and explicit `WordToByte` / `ByteToWord` casts for bytewise lexer work.
- Information-flow labels (positive and negative; R43) for tracking provenance through the compiler if desired.
- `signed` modifier and the signing API for emitting signed compiler artefacts.

**Exists with caveats:**

- **Generics and traits.** Available, but the compiler itself probably does not need many. The compiler may be written largely without generics, relying on enum-based dispatch instead.
- **Hindley-Milner inference.** The compiler implements H-M; the compiler itself should be written with explicit type annotations so the compiler-checking-itself does not stress its own inference.
- **Text type.** Keleusma's `Text` covers static strings and arena-allocated dynamic strings. The lexer needs byte-level inspection of the source. The current convention is for the host to register a native that exposes bytes; whether the self-hosted compiler can do this internally depends on the surface admitting byte iteration over text.

**Resolved (no V0.2.x surface work required):**

- **Map / dictionary data structure.** Resolved by R3.2. A two-layered design: one string interner producing `Word` indices, plus sorted-array `WordMap<V>` for bulk tables (function table, type registry, specialisation table, use-table), plus linear `LocalScope` for per-scope locals. All structures implementable on the V0.2.0 surface directly with the work-stack pattern from R3.1 for any tree-shaped operations.

- **Byte iteration over `Text`.** Resolved by R3.3. The host passes source as `[Byte; N]`; the lexer uses array indexing directly. Three host-registered natives cover the residual `Text` work: `compiler::intern_bytes`, `compiler::text_from_bytes`, `compiler::text_concat`. No surface-language extension required.

- **Persistent compiler state.** Confirmed as a `data` block of the compiler-loop. The specialisation table, trait-impl registry, and similar cross-declaration state are bounded by the declared limits in R3.4 plus the per-module specialisation table reset at module boundaries (R5.3 separate compilation).

- **String building.** Provided by `compiler::text_concat` plus arena-resident scratch buffers. No new natives beyond the three R3.3 specifies.

## Success criteria

V0.3.0 is complete when:

1. The compiler pipeline exists in Keleusma source: `compiler/lexer.kel`, `compiler/parser.kel`, `compiler/codegen.kel`, plus shared AST and bytecode-encoding helpers.
2. **Step 1 intermediate validation**: every program in the regression corpus compiles to byte-identical bytecode under the Keleusma-lexer-plus-Rust-parser-plus-Rust-compiler configuration as under the all-Rust baseline.
3. **Step 2 intermediate validation**: every program in the regression corpus compiles to byte-identical bytecode under the Keleusma-lexer-plus-Keleusma-parser-plus-Rust-compiler configuration as under the all-Rust baseline.
4. The Rust-hosted compiler produces `kelc.0.kel.bin` from the full Keleusma source without error. The existing test suite continues to pass.
5. Phase B fixed-point: `kelc.0.kel.bin` recompiles its own source to produce `kelc.1.kel.bin`. `kelc.1` is byte-identical to `kelc.0` modulo non-essential ordering, formally documented.
6. Phase C fixed-point: `kelc.1.kel.bin` recompiles the same source to produce `kelc.2.kel.bin`, byte-identical to `kelc.1`.
7. Regression corpus equivalence: every script in `examples/scripts/` and the workspace tests compiles to byte-identical bytecode under both the Rust-hosted compiler and `kelc.1`.
8. The CLI gains a `--self-hosted` flag (or similar) that routes through `kelc.1` instead of the Rust-hosted compile path. Programs compile and run end-to-end. During the migration, per-stage toggles (`--lexer keleusma`, etc.) may exist; after Step 3, they collapse into the single `--self-hosted` flag.
9. Documentation: `docs/architecture/`, `docs/guide/`, and the README acknowledge the self-hosted compiler as an alternative path. The Rust-hosted compiler continues to ship; V0.3.0 does not retire it.

The dual-compiler period is intentional. The Rust-hosted compiler remains the reference implementation; the self-hosted compiler is the validation that the language admits its own toolchain.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Hindley-Milner inference cannot be streamed cleanly because the constraint graph spans an entire function body | Restrict the self-hosted compiler's surface input to programs whose bindings are explicitly annotated. The restriction is documented; the Rust-hosted compiler continues to accept un-annotated programs. The annotated form is a strict subset and is what the compiler-in-Keleusma is itself written in. |
| The specialization table grows unbounded across a large compilation | Adopt Modula-2-style separate compilation. Each module compiled independently with a per-module specialization table reset at module boundaries. Cross-module specializations land on the consumer side, bounded by the consumer's own program complexity. |
| Diagnostic quality regresses from the Rust-hosted compiler's level | Accept the regression in V0.3.0; document it. Invest in error-recovery in V0.3.1 once the streaming architecture is proven. Single-pass compilers historically have brittle error recovery (per the prior-art survey); some regression is expected. |
| The self-hosted compiler is materially slower than the Rust-hosted compiler | Profile. Single-pass compilers in the Wirth tradition are typically the fastest per unit of code; the streaming decomposition is theoretically equivalent. If measurement shows the inter-stage buffering cost dominates, fall back to the integrated single-pass alternative documented above. |
| The compiler's per-stage WCMU bound is too loose because of the fixup table or the symbol table | Bound the table sizes explicitly. Reject programs that exceed the declared bound at compile time. The bound is published as a documented limit; programs that need more compile through the Rust-hosted compiler. |
| A required surface-language feature (Map, byte iteration over Text, etc.) is not yet ergonomic | Land the feature in a V0.2.x release before starting V0.3.0 implementation. The "Required surface-language features" section above is the candidate list. |

## Resolved design questions

The strategy's open questions were addressed in a dedicated research loop (2026-05-21). Each recommendation below is summarised; the full record lives under `tmp/research/r3_*.md` and `docs/decisions/RESOLVED.md`.

### Recursion in the compiler (R3.1)

**Recommendation**. The self-hosted compiler walks recursive data with explicit work-stacks. No relaxation of the recursion-prohibition rule.

**Rationale**. The work-stack pattern translates any recursive walk into a `loop` function with a fixed-capacity stack in its `data` block. Three worked examples in R3.1 cover the recursion shapes the compiler needs: pre-order (free-variable collection), accumulating-fold (Robinson unification), and post-order (bytecode emitter). The pattern requires no language-surface change against V0.2.0.

**Confidence**. High for the design. Medium for ergonomics until a sample is compiled and measured.

### Hindley-Milner inference scope (R3.4)

**Recommendation**. Per-function-body inference. The constraint worklist lives in the compiler-loop's `data` block, bounded by explicit declared limits.

**Bounds**. `MAX_TYPE_VARS_PER_FUNCTION = 1024`. `MAX_CONSTRAINTS_PER_FUNCTION = 4096`. `MAX_FUNCTION_BODY_NODES = 16384`. Per-function transient memory approximately 130 KiB, plus persistent approximately 250 KiB.

**Compiler-in-Keleusma annotation discipline**. The compiler's own source is fully annotated so the self-compilation step exercises the easy inference path. This matches the strategy's recommendation.

**Confidence**. High. The bounds were derived from analysis of the Rust-hosted compiler's actual constraint-graph sizes.

### Module-scale compilation (R5.3 informs)

**Recommendation**. Modula-2-style separate compilation. Implementation files `foo.kel`, interface files `foo.def.kel`. Both files are Keleusma source. Per-module specialisation tables reset at module boundaries; cross-module specialisations land on the consumer side bounded by consumer complexity.

**Confidence**. High for the file-naming convention. The cross-module monomorphisation mechanism remains a known gap (see "Open questions" below).

### Diagnostic quality

**Status**. Not explicitly resolved by the research loop. The strategy's "as good as the Rust-hosted compiler's, where possible" target stands. Single-pass compilers historically have brittle error recovery; some regression is expected in V0.3.0 with investment in V0.3.x.

### Self-validation (R3.5)

**Recommendation**. Three-layered validation integrating into `cargo test`.

- **Layer 1**. Byte-identical comparison after canonicalisation (native-name table sorted lexicographically, constant pool sorted by encoded bytes, specialisation chunks sorted by mangled name, function-name to chunk-index map sorted by mangled name then specialisation key). SHA-256 over the canonical form provides a fast pass-or-fail signal.
- **Layer 2**. Logical equality with diagnostic when Layer 1 fails: pretty-prints the bytecode and runs a structural diff against the Rust-hosted output. Surfaces semantic-equivalence-but-not-byte-identity for investigation.
- **Layer 3**. Behavioural equivalence over the regression corpus: every program's runtime output matches between Rust-hosted and self-hosted compilation.

**Confidence**. High. The canonicalisation rules were derived from inspection of the wire format.

### Symbol-table substrate (R3.2)

**Recommendation**. String interner producing `Word` indices, plus sorted-array `WordMap<V>` for bulk tables (function table, type registry, specialisation table, use-table), plus linear `LocalScope` for per-scope locals.

**Surface**. No new language features required. Implementable on the V0.2.0 surface directly.

**Confidence**. High. Sorted-array binary search with `Word` keys is well-understood and matches the persistent-data discipline of the compiler-loop's `data` block.

### Byte iteration over `Text` (R3.3)

**Recommendation**. No surface change. The host passes source as `[Byte; N]`. The lexer uses array indexing. Three host-registered natives handle the residual `Text` work: `compiler::intern_bytes` returns a `Word` interner index, `compiler::text_from_bytes` constructs a `Text` from a byte range for diagnostic messages, `compiler::text_concat` builds composite messages.

**Confidence**. High. The pattern matches the existing host-native interface and adds no surface-language complexity.

## Open questions

These remain unresolved after the 2026-05-21 research pass. None are strategy blockers; each becomes a V0.3.x or implementation-time concern.

1. **Cross-module monomorphisation mechanism.** R5.3 settles the two-file shape but does not specify how generic functions specialise across module boundaries when modules are separately compiled. The shape of the per-module specialisation table and the cross-module instantiation protocol is open.

2. **Diagnostic quality regression bound.** How much diagnostic quality is acceptable to lose in V0.3.0 in exchange for the single-pass streaming architecture? The strategy's "as good as the Rust-hosted, where possible" target is qualitative.

3. **V0.2.0 surface adequacy audit.** R3.1's universal-expressibility argument is sound in prose but unverified in code. Before V0.3.0 implementation begins, a sample exercise (Robinson unification, a recursive AST walker, a monomorphisation pass) should be compiled and measured against the V0.2.0 surface. If a load-bearing affordance is missing, the work-stack pattern needs supplementation.

## Out of scope

The following are explicitly out of scope for this strategy document.

- **The implementation schedule.** Whether V0.3.0 lands in calendar quarter X or Y depends on capacity that this document cannot allocate. The strategy is correct regardless of when the implementation begins.

- **V0.4.0 and beyond.** Native-code generation, vintage-processor targets, and the Keleusma-host model are V0.4.0+ concerns. They are referenced here only because V0.3.0 is a precondition for them; their design lives in a separate strategy document.

- **Keleusma-host and Keleusma-VM self-hosting.** Compiling the runtime VM itself in Keleusma is a V0.5+ aspiration. It is mentioned here because V0.4.0's native-code-generation precondition opens the door, but the design is not within V0.3.0's scope.

## Lessons from a contemporary partial self-hosting (Brief)

The `brief-lang` project is a contemporary language that attempted self-hosting and reached, then stalled at, the frontier V0.3.0 approaches. A targeted review of it (fuller writeup retained outside the repository) yields several concrete lessons for this strategy. The observations reflect that project at a point in time and are cited for their engineering value, not as endorsement.

- **The frontend is the achievable part; codegen and host output are the wall.** Brief has a working compiler frontend written in Brief (lexer, parser, typechecker, contract engine) but is *not* bootstrapped, because the frontend runs inside a host interpreter and its backends are unfinished. Sequence the V0.3.0 work so that **codegen and the emit-to-host boundary are treated as the high-risk stages and de-risked first**, not the lexer and parser. This reinforces the incremental-migration ordering above.

- **The output capability must be a first-class host native from the start.** Brief's self-hosted compiler could read source but had no general facility to *write* its output, which is a hard blocker to a true bootstrap. Keleusma's host-native surface must expose a deliberate, bounded "emit compiled bytecode" capability to the self-hosted compiler as a designed feature, not an afterthought. See the bootstrap procedure and inter-stage data shapes.

- **Divergent execution models are a bootstrap hazard.** Brief maintained a tree-walking interpreter and a compiled backend that drifted into *different* runtime semantics, silently miscompiling programs. Keleusma's bootstrap fixed point (kelc.0 → kelc.1 → kelc.2) is the guard against this: it converges only if the self-hosted compiler's output is stable across stages, so the byte-identical fixed-point check in the bootstrap procedure is load-bearing, not ceremonial. Avoid keeping two independently evolving semantics for the surface subset.

- **The work-stack idiom is independently validated.** Brief's compiler used explicit work-stacks and threaded state (for example, iterative depth-first call-graph traversal) even though its language permits recursion. That a real compiler was written this way is external evidence that Keleusma's recursion-free, work-stack pipeline (R3.1) can express the compiler it needs to.

- **Only admit surface syntax you will actually compile.** Brief accumulated parsed-but-uncodegened constructs, each becoming a maintenance stub that generated defects. Grow the self-hosted compiler feature-complete per increment: parse, typecheck, and generate for a construct together, or not at all. This aligns with the constrained-surface-language discipline above.

- **Consume every analysis result on every path.** Brief repeatedly computed an analysis (liveness, convergence) and then failed to consume it in one of several codegen paths, losing the benefit and creating inconsistency. A staged self-hosted pipeline must ensure each stage consumes the descriptors the prior stage produced, everywhere they apply.

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
