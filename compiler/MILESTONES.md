# Self-hosting milestones: the road from V0.2.x to V0.3.0

Completing the self-hosted compiler is the V0.3.0 release. The V0.2.x line proceeds
toward it, each release landing prerequisites or a pipeline stage. This document is
the release-by-release plan; the design behind each item is in
[`docs/roadmap/V0_3_0_SELF_HOSTING.md`](../docs/roadmap/V0_3_0_SELF_HOSTING.md), which
is authoritative. Version numbers past V0.2.2 are a plan, not a promise; the ordering
is the load-bearing part.

## The migration order (roadmap Steps)

The self-hosted compiler is reached by replacing one Rust stage at a time, validating
byte-identical output against the all-Rust baseline at each step. This isolates a bug
in the migrated stage against a known-good downstream consumer, and it climbs the
natural complexity gradient (lexer simplest, compiler hardest).

| Release | Roadmap step | Deliverable | Validation gate |
|---------|--------------|-------------|-----------------|
| **V0.2.2** | Scaffolding | This subproject: the three-stage structure, the shared inter-stage data shapes (`kel/prelude.kel`), the Rust host driver skeleton, and the bootstrap harness. The three `compiler::` natives (`intern_bytes`, `text_from_bytes`, `text_concat`). Source-as-`[Byte; N]` input path. | The driver builds and drives stub stages end to end; the natives round-trip. |
| **V0.2.3+** | Prerequisite patterns | The reusable Keleusma patterns the stages need: the work-stack idiom for walking recursive data without `fn`/`yield` recursion (R3.1), the string interner plus sorted-array `WordMap<V>` and linear `LocalScope` (R3.2), bounded fixup tables. Written and unit-tested as `.kel` on the current runtime. | Each pattern compiles, verifies (bounded WCMU), and passes targeted tests. |
| **V0.2.x** | **Step 1 — lexer** | `kel/lexer.kel`: a `loop` byte scanner with a keyword table, yielding `Token`s. Wired into the Rust pipeline behind `--lexer keleusma`. | Every program in the regression corpus compiles to **byte-identical** bytecode under Keleusma-lexer + Rust-parser + Rust-compiler as under the all-Rust baseline. |
| **V0.2.x** | **Step 2 — parser** | `kel/parser.kel`: recursive-descent over the grammar using the work-stack discipline, yielding one `Declaration` per top-level declaration. Behind `--parser keleusma`. | Byte-identical corpus output under Keleusma-lexer + Keleusma-parser + Rust-compiler. |
| **V0.2.x** | **Step 3 — compiler** | `kel/codegen.kel` plus the inference-scope and monomorphization-specialization helpers. The full pipeline now exists in Keleusma. Behind `--compiler keleusma`. This step is the largest and may span several V0.2.x releases. | Byte-identical corpus output under the all-Keleusma pipeline driven by the Rust host. |
| **V0.3.0** | **Bootstrap** | Phase A cross-compiles the Keleusma source to `kelc.0.kel.bin` with the Rust-hosted compiler. Phase B self-compiles to `kelc.1`. Phase C reaches the fixed point `kelc.2` == `kelc.1`. The per-stage toggles collapse into a single `--self-hosted` flag. | `kelc.1` == `kelc.2` byte-for-byte (modulo documented non-essential ordering); the regression corpus compiles byte-identically under both the Rust-hosted compiler and `kelc.1`. |

## Surface-language and runtime prerequisites

Per the roadmap's "Required surface-language features", most of the surface is already
sufficient as of V0.2.0. The items that still need *work* (not surface changes) before
the corresponding step:

- **Before Step 1 (lexer).** The `compiler::intern_bytes` / `text_from_bytes` /
  `text_concat` natives, and the `[Byte; N]` source-input path. Both land in the
  V0.2.2 scaffolding here. No surface-language extension is required (R3.3).
- **Before Step 2 (parser).** The work-stack idiom (R3.1) proven, so recursive data is
  walked without `fn`/`yield` recursion. Whether to relax the recursion rule for the
  compiler instead of using explicit stacks is the one open surface question; the
  default is explicit stacks.
- **Before Step 3 (compiler).** The compiler-in-Keleusma is written in the
  **explicitly-annotated subset** so that the compiler checking itself does not stress
  its own Hindley-Milner inference. Inference is bounded to per-declaration scope; the
  monomorphization specialization table is bounded persistent `data`-block state
  (R3.4, R5.3). None of this requires a surface change; it is a discipline on how the
  compiler's own source is written.

## What V0.3.0 does not do

V0.3.0 does not retire the Rust-hosted compiler. The dual-compiler period is
intentional: the Rust-hosted compiler stays the reference implementation, and the
self-hosted compiler is the proof that the language admits its own toolchain. Native
code generation (Keleusma-to-LLVM-to-native) is the separate V0.4.0 effort and depends
on this milestone landing first; see [`docs/roadmap/V0_4_0_NATIVE_CODEGEN.md`](../docs/roadmap/V0_4_0_NATIVE_CODEGEN.md).
