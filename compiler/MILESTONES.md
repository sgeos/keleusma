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
| 1 | **Codegen** (`kel/codegen.kel`) | The largest stage, migrated inside-out over several increments: the emit-to-host boundary plus a logical-module round-trip first, then compile, then monomorphization, then per-declaration typecheck. Yields the ops the host assembles into the module. **Increments 0 through 19 landed** (`tests/selfhost_codegen.rs`). Increment 0 proved the emit boundary; increment 1 the op vocabulary; increment 2 the first real codegen from input (single-node bodies). Increment 3 is the **recursion-free work-stack tree walk** (R3.1), the design's biggest tension after the emit boundary: the stage drains an explicit work-stack, one work item per `loop` iteration, emitting a post-order op stream for a general binary-arithmetic body of literals, parameters, and `+`/`*`, including the checked-arithmetic-then-`PopN` lowering, reaching `input * 2 + 1` computed from its tree rather than transcribed. Increment 4 has the stage **own its constant pool**: literals now carry their value, the stage interns each into a pool as it walks, emits `Const` at its own index, and emits the pool after `Return`; the host builds the module from the stage's ops and pool rather than the reference's, removing the adapter's index-precomputation crutch. Increment 5 **deduplicates the pool**, mirroring the reference compiler's constant interning (`compiler.rs add_const_value`): before appending a literal the stage scans the pool with a bounded `for` over its fixed capacity and reuses a matching index, so a repeated literal produces one entry and aligned indices. The scan accumulates the match index in a `data` field, since locals are immutable. Increment 6 compiles a **block of `let` bindings** followed by the tail expression: each `let x = e;` lowers to e's ops then `SetLocal(slot)`, slots assigned after the parameters in declaration order, and identifier references resolve to a parameter or `let` slot; the stage seeds its work-stack with the tail deepest and each statement's (VISIT-expr, EMIT-SetLocal) pair pushed last-to-first so the LIFO drain emits statements in source order, then the tail, then `Return`. Increment 7 lowers the **full binary integer arithmetic set** (`+ - * / %`): a per-operator lowering in the BinOp branch selects the op word and whether the result needs the two-word `PopN(2)` fixup, so the three checked operators (`+`/`*`/`-`) take the `PopN` and `/`/`%` (single-word `Div`/`Mod`) take none, matching the reference lowering. Increment 8 adds the **six comparison operators** (`==`, `!=`, `<`, `>`, `<=`, `>=`), each a single `Cmp*` op with no `PopN` and a `bool` result; crossing sixteen op tags exceeded the old `tag + operand*16` op-word radix (tag 16 would alias the `PENDING` marker), so the op-word radix widened to 32, a self-contained protocol change between the stage and its host driver with no runtime `BYTECODE_VERSION` impact. Increment 9 adds **`if`/`else` structured control flow** (expression form): an If node lowers to the condition ops, an `If` marker, the then-branch ops, an `Else` marker, the else-branch ops, then `EndIf`, matching the reference's structured lowering, and validated on nested `if` as well. The `If`/`Else` operands are absolute forward op indices the reference bakes in, but a single post-order walk cannot know the forward branch sizes when it emits the markers, so this increment emits placeholder targets and the host assembler resolves them by bracket-matching the nested markers. This mirrors how the constant pool first arrived with host-supplied indices (increment 3) before the stage owned it (increment 4). Increment 10 **pulls target resolution into the stage**: rather than stream one op per yield, the stage appends op words to a bounded `ops` buffer in the `data` segment and, as it appends an `Else`/`EndIf`, backpatches the matching `If`/`Else` target in place using a control-flow position stack, then streams the resolved buffer; the host `resolve_targets` step is gone and the emitted stream carries the reference's absolute targets directly. Backpatching a fixed-size buffer keeps memory statically bounded. Increment 11 has the stage **emit its own `local_count`** (the local-frame size, `param_count + let count`), streamed as one raw Word after the pool; the host builds the chunk with it and asserts it agrees with the reference. This is the one chunk-header field codegen computes; the remaining header fields (name, parameter arity and types, block type) are declaration data the parser and type checker produce, not codegen products, and stay input-adapter transcription. So the stage now emits every field codegen owns for this grammar subset. Increment 12 makes **blocks first-class and nestable**: earlier increments carried a block's `let` bindings in a top-level statement array, so `let` could appear only at the body's top level; this increment folds blocks into the node forest as a `LetIn` cons-list (`let x = e; rest` -> node `LetIn(slot, value, continuation)`), so the top-level body and each `if`-branch are the same thing (a block root), and `let` nests inside branches. Slots are assigned in tree-walk order and never reused across branches (matching the reference), so `local_count` is the parameters plus the number of `LetIn` nodes the walk processes, which the stage counts directly. Increment 13 adds **unary `not`**, which lowers to the operand ops followed by a single `Not` op. Increment 14 adds **function calls**: the argument ops are emitted left-to-right, then `Call(chunk_index, arg_count)` (the two operands packed into the op word's operand field); the callee chunk index is resolved-reference data the adapter supplies by name (name resolution is an upstream concern, not codegen arithmetic). Increment 15 adds the **short-circuit booleans `andalso`/`orelse`**, which reuse the structured control flow (`Dup` then `If`/`PopN(1)`/`Else`/`EndIf`, with a `Not` inserted for `orelse`); the existing bracket-matching backpatcher happens to produce exactly the reference's targets, and no temporary slot is allocated. Increment 16 is a **pure refactor**: the per-node scheduling, which had grown into a nine-deep `if`/`else` chain, is factored into one `fn` per node kind that mutates the work-stack `data`, and the dispatch is flattened to a flat sequence of `if nk == K { push_kind(payload); }` statements; the emitted op stream, pool, and `local_count` are byte-identical, which the unchanged conformance suite confirms. It also establishes that a Keleusma `fn` may mutate the `data` segment and that such a factoring still verifies. Increment 17 rounds out the operator surface with unary negation (`CheckedNeg` + `PopN(2)`) and the per-limb bitwise operators `band`/`bor`/`bxor` (single-op, no `PopN`); each was a one-line lowering-table and dispatch addition, a payoff of the increment-16 refactor. Increment 18 is another **pure refactor**: the two remaining nested `if`/`else` value dispatches (the node dispatch in the `loop` body and the operator-word mapping in `push_binop`) become flat `match` statements. multi-clause function heads (which the surface does support) were considered but a local `match` reads better for a value-to-value mapping. Increment 19 then **flattens the whole `loop` body into a phase dispatch**: the seed, walk, and five drain phases, previously nested five deep, each move into a `fn`, and `main_phase` (a guarded `match` — guards do work inside a `match`, unlike on a function head) selects the phase, so the body is one flat `match main_phase() { k => yield step_k(), ... }`. The `yield` stays in the Stream block, as the verifier requires (a `loop` that only delegates to a yielding `fn` fails verification), while the per-phase work lives in the steps. All are validated by structural equality against the Rust compiler and by running the built module.

Deferral ledger. No host codegen crutch remains for the current grammar; the stage produces the ops, the constant pool, the resolved jump targets, and the local-frame size. Still open: the input-adapter transcription of declaration fields (name, parameter arity and types, block type) will be retired only when the parser stage lands upstream. Unverified on the self-hosted path: division-by-zero totality (the `Div`/`Mod` cases use constant divisors). Grammar not yet covered by codegen, in rough priority order for self-compiling the stage's own source. (1) **The data segment**: scalar read/write (`GetData`/`SetData`), then indexed array read/write (`GetDataIndexed`/`SetDataIndexed`, which carry the field's data slot and the array length); this needs the stage to resolve a data field name to a slot and an array length, which is layout data the adapter supplies. (2) **Statement-form blocks**: a block statement that is not a `let` (a data assignment, or a bare call for effect), which generalizes the `LetIn` cons-list to a sequence of effectful statements. (3) **`for` loops** over a constant or bounded range, which lower to loop-variable and limit slots plus `Loop`/`BreakIf`/`EndLoop` back-edges (the back-edge target is resolvable by the same buffer-backpatching used for `If`/`Else`). (4) The eager boolean operators (`and`/`or`/`xor`, which allocate a temporary slot) and composite values. Items (1) through (3) are the large remaining frontier and the gate to the stage compiling itself; each is one or more increments. | Rust lex, parse, and initially typecheck and monomorphization, mapped into the Keleusma declaration stream. The adapter delivers progressively-less-processed declarations as the inner increments advance. |
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
