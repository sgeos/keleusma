# Self-hosting milestones: the road from V0.2.x to V0.3.0

Completing the self-hosted compiler is the V0.3.0 release. The V0.2.x line proceeds
toward it, each release landing a stage increment or a prerequisite. This document is
the release-by-release plan and the migration strategy. The architecture design is in
[`docs/roadmap/V0_3_0_SELF_HOSTING.md`](../docs/roadmap/V0_3_0_SELF_HOSTING.md), which
stays authoritative for the shape of the compiler. The migration strategy below is an
operator decision that revises the earlier forward, byte-identical migration ordering
described there. Version numbers past V0.2.2 are a plan, not a promise; the strategy is
the load-bearing part.

## Migration strategy

This section is the Keleusma instantiation of a general, project-agnostic method,
[Incremental Self-Hosting by Backward Migration](../docs/reference/INCREMENTAL_SELF_HOSTING.md),
which states the same strategy without Keleusma nouns for any language author to reuse.
The subsections below apply it here.

The self-hosted compiler has three streaming stages, lexer then parser then codegen,
and it is ported from the last stage to the first. Codegen goes first, then the parser,
then the lexer. The reason is risk. The emit and wire boundary is the wall that stalls
self-hosting efforts, so it is proven first, and the lexer, which is the easy part,
comes last. This reverses the convenience-first ordering the design document originally
described.

### The moving adapter seam

At any moment exactly one throwaway adapter sits at the frontier between the Rust
upstream and the Keleusma downstream. The Rust stages above the frontier run as a batch,
and the adapter maps their batch output into the nominal stream that the first Keleusma
stage consumes. The Keleusma stages below the frontier are already written and chain
directly to one another. Each time an earlier stage is ported, it supersedes the adapter
that stood in for it, and a new adapter appears one position upstream.

### Adapters are throwaway prototypes

An adapter's output is real, permanent work. It is the Keleusma inter-stage data shape
that the pipeline will carry forever. An adapter's implementation, the Rust to Keleusma
conversion and the batch to stream mapping, is a throwaway prototype of the output
behavior that the not-yet-written upstream Keleusma stage will eventually produce.
Writing that upstream stage is what retires the adapter.

Adapters need to be good enough to port the chain, not perfect, not general, and not
efficient. There is a real tension between fixing an adapter and writing the upstream
stage that will replace it. When writing the upstream stage is the higher-value move,
take it. The goal is stages, not adapters.

### The correctness gate and the deferral ledger

rkyv is not a constraint. It was a means to serialize bytecode, and the self-hosted
compiler does not need to reproduce its byte layout. Serialization is delegated to a
host emit native, or replaced outright when that serves the end better.

The primary equivalence check is structural equality of the logical module, comparing
the module the Keleusma chain produces against the module the Rust compiler produces at
the level of chunks, opcodes, layouts, and the auxiliary tables, rather than at the
level of serialized bytes. Behavioral equivalence over the corpus, compiling both and
running and comparing observable yields and results, is the secondary check.

Correctness is a judgment call backed by a ledger. Each corpus program is either
processed correctly by the current chain, or its deviation is recorded in a deferral
ledger that names the upstream stage which will correct it. When that stage lands, the
deferred cases are re-run and confirmed resolved. A deviation that its responsible
upstream stage does not fix was never an adapter limitation, and it is a real bug in the
Keleusma stage that produced it. Without the ledger and the recheck, the judgment call
becomes a place for stage bugs to hide.

### Two engineering modes

During migration, engineering defers. It prefers building the next stage over
perfecting a throwaway adapter, and it records what it defers in the ledger. Once all
three self-hosted stages exist and the adapters are gone, engineering switches from
deferred work to bug fixing. The deferral ledger is driven to empty, and the
all-Keleusma chain must process the corpus correctly with no adapter left to defer to.
An empty ledger and a passing corpus under the all-Keleusma chain is the V0.3.0
acceptance gate. Every intermediate deferral is a debt against it.

### Stage decomposition

Each of the three stages is many increments. The Rust compiler was not written in three
iterations, and the translation will not be either. Codegen is the largest, because in
the streaming design it fuses typecheck, monomorphization, compile, and emit for one
declaration at a time. It is migrated inside-out, mirroring the same backward discipline
one level down. The codegen input adapter first delivers already-typechecked and
monomorphized declarations, so the first increment is the emit-to-host native plus a
logical-module round-trip, then compile, then monomorphization, then typecheck, with the
adapter delivering progressively-less-processed declarations at each increment.

## The stages, ported last to first

| Order | Stage | Deliverable | Its input adapter |
|-------|-------|-------------|-------------------|
| done | Scaffolding | The three-stage structure, the shared inter-stage data shapes (`kel/prelude.kel`), and the Rust host driver. The three `compiler::` Text natives (`intern_bytes`, `text_from_bytes`, `text_concat`) are named in the design but not yet registered, since they are a lexer prerequisite and the lexer is now ported last. | not applicable |
| 1 | **Codegen** (`kel/codegen.kel`) | The largest stage, migrated inside-out over several increments: the emit-to-host boundary plus a logical-module round-trip first, then compile, then monomorphization, then per-declaration typecheck. Yields the ops the host assembles into the module. **Increment 0 landed**: the emit spike. A hardcoded emitter yields the op stream for `fn main() -> Word { 1 }`; the host lowers it to real opcodes, and the stream matches the Rust compiler's output (logical-artifact equivalence) and builds a runnable module that returns 1 (`tests/selfhost_codegen.rs`). No input yet; the module frame and constant pool are host-supplied. | Rust lex, parse, and initially typecheck and monomorphization, mapped into the Keleusma declaration stream. The adapter delivers progressively-less-processed declarations as the inner increments advance. |
| 2 | **Parser** (`kel/parser.kel`) | Recursive descent over the grammar using the work-stack discipline (R3.1), yielding one `Declaration` per top-level declaration into the finished Keleusma codegen. | Rust lexer token output mapped into the Keleusma token stream. |
| 3 | **Lexer** (`kel/lexer.kel`) | A streaming byte tokenizer feeding the finished Keleusma parser. Increment 1 already exists, built as an early spike before this backward strategy was adopted, and is kept. | none; the source bytes are the real host input, not a throwaway adapter. |
| V0.3.0 | **Bootstrap** | The adapters are gone. Cross-compile the Keleusma source to `kelc.0` with the Rust-hosted compiler, self-compile to `kelc.1`, and reach the fixed point `kelc.2` equal to `kelc.1`. The per-stage toggles collapse into a single `--self-hosted` flag. | none. |

Note on the lexer. Increment 1 of the lexer already compiles, verifies, and runs
(`keleusma-selfhost lex <file>` and `tests/selfhost_lexer.rs`). It was built while the
plan was still forward-first. Under the backward order the lexer is the last stage to
complete, so the spike simply sits ahead of its place, and the lexer is finished last.

## Surface-language and runtime prerequisites

Per the design's "Required surface-language features", most of the surface is already
sufficient as of V0.2.0. The items that still need *work*, not surface changes, are tied
to the stage that needs them rather than to a release number:

- **For the lexer.** The `compiler::intern_bytes` / `text_from_bytes` / `text_concat`
  natives, and the byte-array source-input path (already exercised by increment 1). No
  surface-language extension is required (R3.3).
- **For the parser.** The work-stack idiom (R3.1), so recursive grammar is walked
  without `fn`/`yield` recursion. Whether to relax the recursion rule for the compiler
  instead of using explicit stacks is the one open surface question; the default is
  explicit stacks.
- **For codegen.** The compiler-in-Keleusma is written in the **explicitly-annotated
  subset** so that the compiler checking itself does not stress its own Hindley-Milner
  inference. Inference is bounded to per-declaration scope, and the monomorphization
  specialization table is bounded persistent `data`-block state (R3.4, R5.3). None of
  this requires a surface change; it is a discipline on how the compiler's own source is
  written.

## What V0.3.0 does not do

V0.3.0 does not retire the Rust-hosted compiler. The dual-compiler period is
intentional. The Rust-hosted compiler stays the reference implementation and the
equivalence oracle, and the self-hosted compiler is the proof that the language admits
its own toolchain. Native code generation (Keleusma to LLVM to native) is the separate
V0.4.0 effort and depends on this milestone landing first; see
[`docs/roadmap/V0_4_0_NATIVE_CODEGEN.md`](../docs/roadmap/V0_4_0_NATIVE_CODEGEN.md).
