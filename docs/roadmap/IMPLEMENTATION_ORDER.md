# Implementation order: V0.3.0 → V0.4.0 → V0.5.0

**Status**: synthesis document. Consolidates the 17 design documents produced in this research loop into an actionable implementation plan. The operator can read this first to understand what each release needs and in what order.

## Top-level reading order for the operator

When the operator returns, the recommended reading order across the research and internal materials:

1. **`tmp/research/STATUS.md`** — the backlog and firing log. The where-we-are summary.
2. **This document** — the where-we-go-next plan.
3. **The R3 series** (`r3_1` through `r3_5`) — V0.3.0 design.
4. **The R4 series** (`r4_1` through `r4_5`) — V0.4.0 design.
5. **The R5 series** (`r5_1` through `r5_5`) — V0.5.0 design.
6. **The cross-cutting items** (`rc_1`, `rc_2`) — testbed and vintage homebrew.
7. **The perpetual operational scenarios** — internal materials documenting the long-duration deployment framing.

Each individual document is ~500-1500 lines. Total reading effort is one to two days at unhurried pace.

## What V0.2.x can pick up immediately

Three small documentation patches that can land without waiting for V0.3.0 implementation:

### Patch 1: correct the V0.4.0 strategy's coroutine-ABI reference

Per R4.1, the V0.4.0 strategy at `docs/roadmap/V0_4_0_NATIVE_CODEGEN.md` § "Sub-coroutine lowering" references `@llvm.coro.id.async` for the custom-allocator path. R4.1 establishes that switched-resume (`@llvm.coro.id`) is the correct choice; the async family is specific to Swift's async/await.

The patch is a one-paragraph correction: replace the `@llvm.coro.id.async` references with `@llvm.coro.id` plus the standard alloc/free hook description. The substantive design (arena-resident coroutine frames, per-coroutine allocator) is unchanged.

### Patch 2: cross-reference the new R-docs from the strategy docs

Each strategy document (`V0_3_0_SELF_HOSTING.md`, `V0_4_0_NATIVE_CODEGEN.md`, `V0_5_0_KELEUSMA_HOST.md`, `SUB_COROUTINES.md`) can gain a "Resolved design questions" section that points at the corresponding R-docs once integrated:

- V0.3.0 strategy: link R3.1-R3.5.
- V0.4.0 strategy: link R4.1-R4.5.
- V0.5.0 strategy: link R5.1-R5.5.
- SUB_COROUTINES.md: link R5.1 (surface syntax), R5.4 (mutual exclusivity).

The operator decides whether to integrate the R-docs as a `docs/decisions/` (resolved decision) cluster or as patches to the strategy documents themselves. R-docs are larger; pointer-style cross-reference is the lower-friction integration.

### Patch 3: STM32N6570-DK Phase α integration

Per RC.1's Phase α (drafted in `tmp/research/rtos_n6_testbed/phase_a/`), the operator can integrate the producer-consumer pipeline into `examples/rtos/` directly. The work is mechanical:

1. Copy the four phase-α files into `examples/rtos/`.
2. Update `examples/rtos/Cargo.toml` to declare the new binary.
3. Update `examples/rtos/build.rs` to compile the two new scripts.
4. Flash, observe, document the result.

This adds a small additional demonstrator alongside `three-task-n6` and validates that the spawn-style pattern works on real hardware before V0.5.0's surface lands.

## V0.3.0 implementation order

V0.3.0 is the next major release. The strategy at `docs/roadmap/V0_3_0_SELF_HOSTING.md` recommends Lexer → Parser → Compiler. R3.1 through R3.5 resolve the surface questions; the implementation order:

### Step 0: V0.2.x prep work

Before starting V0.3.0 implementation:

- Patch V0.4.0 strategy per Patch 1 above.
- Add the three host natives required by R3.3 (`compiler::intern_bytes`, `compiler::text_from_bytes`, `compiler::text_concat`). These can live in `src/utility_natives.rs` as optional registration.
- Implement R3.2's interner and `WordMap<V>` substrate as standalone modules in `src/` so they can be tested independently before being integrated into the self-hosted compiler.

The Step 0 work is small (one to two weeks) and lands in V0.2.x.

### Step 1: Lexer in Keleusma (V0.3.0 strategy's incremental migration step 1)

- Write `compiler/lexer.kel` using R3.1's work-stack pattern for any recursive walks.
- Apply R3.2's interner to identifier names.
- Use R3.3's `[Byte; N]` input pattern; the Rust host passes the source as a byte array.
- Validate via R3.5's three-layer comparison: every program in the regression corpus produces byte-identical bytecode under the Keleusma-lexer-plus-Rust-rest configuration.

Estimated effort: two to four weeks for a familiar Rust+Keleusma developer.

### Step 2: Parser in Keleusma

- Write `compiler/parser.kel` using R3.1's work-stack pattern for expression walks.
- Apply R3.2's interner via R3.3's source-byte-array pattern.
- Yield one `Declaration` per loop iteration.
- Validate via R3.5's three-layer comparison.

Estimated effort: four to eight weeks.

### Step 3: Type checker and compiler in Keleusma

- Write `compiler/typecheck.kel` using R3.4's per-function inference with declared bounds.
- Write `compiler/codegen.kel` using R3.1's work-stack pattern for the emitter.
- Validate via R3.5's three-layer comparison.

Estimated effort: eight to sixteen weeks.

### Step 4: Bootstrap fixed-point

- Compile the V0.3.0 self-hosted compiler under the Rust-hosted compiler to produce `kelc.0.kel.bin`.
- Recompile `compiler/*.kel` via `kelc.0` to produce `kelc.1.kel.bin`. Validate that `kelc.1 = kelc.0` (modulo canonical ordering per R3.5).
- Recompile via `kelc.1` to produce `kelc.2.kel.bin`. Validate `kelc.2 = kelc.1`.

Estimated effort: one week of validation if no bugs surface.

### Step 5: Integrate into the CLI and documentation

- Add the `--self-hosted` flag to `keleusma-cli`.
- Update documentation per the V0.3.0 strategy's success criteria.

Estimated effort: one week.

**Total V0.3.0 estimated effort: four to eight months for a single developer.**

## V0.4.0 implementation order

V0.4.0 builds on V0.3.0 plus the LLVM toolchain. The R4 series resolves the design questions.

### Step 0: V0.4.x prep work

- Land R4.1's correction in the V0.4.0 strategy document.
- Prototype the LLVM coroutine integration per R4.1's milestones M1-M3 (one to two weeks).
- Pin LLVM 17 in the build pipeline per R4.3.
- Add inkwell + llvm-sys dependencies per R4.4.

### Step 1: LLVM IR generator from bytecode

- Write a Rust crate that consumes Keleusma bytecode and emits LLVM IR.
- Use the R4.2 symbol mangling for every emitted function.
- Use R4.1's per-coroutine allocator pattern for every `loop` sub-coroutine.
- Validate by running the V0.3.0 self-hosted compiler's regression corpus through the new IR generator and confirming the IR `opt`-verifies clean.

Estimated effort: four to eight weeks.

### Step 2: LLVM lowering and linking

- Invoke LLVM through inkwell to lower IR to native object files.
- Link against a small Rust runtime providing the allocator/deallocator functions.
- Validate by running the regression corpus end-to-end through the new pipeline.

Estimated effort: four to eight weeks.

### Step 3: Tier-1 platform support

- x86-64 Linux works end-to-end first.
- AArch64 Linux is next (mostly LLVM-back-end work).
- macOS (both arches) follows.
- Each Tier-1 platform's validation: regression-corpus byte-equivalence plus CI green.

Estimated effort: two to four weeks per platform.

### Step 4: V0.4.x Tier-2 platform support

- Windows MSVC (CodeView debug info is the load-bearing extra work).
- Cortex-M55 and Cortex-M4 (`thumbv8m.main-none-eabihf` and `thumbv7em-none-eabihf`).
- Each Tier-2 platform: same shape as Tier-1 but with CI-only validation.

Estimated effort: two to six weeks per platform.

### Step 5: Integration with V0.5.0 host

V0.5.0's Phase δ migrates the Keleusma host to native code via V0.4.0. The work overlaps with V0.5.0 implementation; see below.

**Total V0.4.0 estimated effort: six to twelve months.**

## V0.5.0 implementation order

V0.5.0 builds on V0.3.0 and V0.4.0. The R5 series resolves the design questions.

### Step 0: V0.5.0 prep work

- Sub-coroutines are the load-bearing primitive. Implement them at the runtime first.
- The new keywords (`spawn`, `resume`, `release`, `complete`, `yields`, `accepts`, `completes`) extend the grammar and the type checker.
- The wire-format header gains the interface-fingerprint field per R5.2.

### Step 1: Sub-coroutine runtime

- Extend the VM to support multiple concurrent coroutines per program.
- Implement the four new opcodes (`SpawnCoroutine`, `ResumeCoroutine`, `ReleaseCoroutine`, plus the yield extension).
- Implement the per-coroutine arena slot management.
- Validate via unit tests at the VM level.

Estimated effort: four to eight weeks.

### Step 2: Sub-coroutine surface syntax

- Extend the parser to recognise the new keywords per R5.1.
- Extend the type checker to enforce handle scoping (local-only).
- Extend the compiler to emit the new opcodes.
- Validate via end-to-end programs.

Estimated effort: four to eight weeks.

### Step 3: Three-mode purity discipline

- Extend the parser to recognise `impure`, `transitive`, `pure` modifiers.
- Extend the type checker per R5.5's call-graph rules.
- Validate via test cases covering the edge cases in R5.5.

Estimated effort: four to six weeks.

### Step 4: Module system

- Two-file modules per R5.3 (`.kel` and `.def.kel`).
- Interface fingerprints per R5.2 (SHA-256 of canonical serialisation).
- Cross-module compilation: separate compilation with per-module specialisation tables.
- Validate via multi-module programs.

Estimated effort: eight to twelve weeks.

### Step 5: Arena partitioning and live update

- Master-WCMU sum allocation per V0.5.0 (no mutual-exclusivity refinement yet).
- Hot-swap acceptance rules per R5.2 (fingerprint match or signed migration entry).
- Validate via hot-swap tests at the VM level.

Estimated effort: four to eight weeks.

### Step 6: Keleusma-hosted host migration

- Write `host/main.kel` (the V0.5.0 driver in Keleusma).
- Restructure the CLI to dispatch through `impure fn main` or `impure loop main`.
- Validate the bootstrap procedure (Phase α through Phase δ from the V0.5.0 strategy).

Estimated effort: eight to twelve weeks.

### Step 7: V0.5.x follow-ups

- Mutual-exclusivity refinement per R5.4 (interval-graph case).
- State migration across compatible hot swaps.
- Quiescence-point detection for guaranteed-safe replacement.
- Each is independent; V0.5.x point releases.

Estimated effort: per V0.5.x release.

**Total V0.5.0 estimated effort: six to twelve months.**

## Total program timeline

If implementation proceeds linearly (no parallel workstreams):

| Release | Estimated effort | Cumulative effort |
|---|---|---|
| V0.2.x prep | One to two months | One to two months |
| V0.3.0 | Four to eight months | Five to ten months |
| V0.4.0 | Six to twelve months | Eleven to twenty-two months |
| V0.5.0 | Six to twelve months | Seventeen to thirty-four months |
| V0.5.x mutex | Two to four months | Nineteen to thirty-eight months |

**Total**: roughly one and a half to three years for one developer working alone. Parallelism (e.g., V0.4.0 LLVM IR generator and V0.5.0 sub-coroutine runtime in parallel) reduces wall-clock time but not engineer-time.

## What this loop did NOT produce

This loop produced design documents. It did not produce:

- Implementation code for V0.3.0, V0.4.0, or V0.5.0.
- Prototypes of the LLVM coroutine integration.
- Real measurements (the testbed phases are scaffolding, not run results).
- Integration patches against the existing strategy documents.

These remain for the operator to undertake when they return. The R-docs provide the design; the operator provides the implementation.

## Cross-references

- All R-docs in `tmp/research/`.
- `tmp/research/STATUS.md` for the backlog and firing log.
- Internal materials for the perpetual operational scenarios.
- `tmp/research/rtos_n6_testbed/` for the Phase α skeleton.
- `docs/roadmap/V0_3_0_SELF_HOSTING.md`, `V0_4_0_NATIVE_CODEGEN.md`, `V0_5_0_KELEUSMA_HOST.md` for the strategy documents the R-docs settle.
- `docs/architecture/SUB_COROUTINES.md` for the sub-coroutine spec R5.1 and R5.4 advance.

## Decision summary

| Question | Answer |
|----------|--------|
| Where to start? | V0.2.x prep work (Patch 1, host natives for R3.3, R3.2 substrate as standalone modules) |
| First major release to implement? | V0.3.0 self-hosted compiler |
| Implementation order within V0.3.0? | Lexer → Parser → Type-check + Compiler → Bootstrap → CLI integration |
| V0.4.0 dependencies | V0.3.0 must ship first (or at least the V0.3.0 bytecode artefact must exist) |
| V0.5.0 dependencies | V0.3.0 and V0.4.0 must ship first; sub-coroutine runtime can start in parallel |
| Wall-clock to V0.5.0 ship | One and a half to three years for a single developer; less with parallelism |
| Critical path | Sub-coroutine runtime (V0.5.0) is the single largest workstream; consider starting it in parallel with V0.4.0 LLVM work |
| Phase α testbed | Buildable today; ~one workday for a familiar developer to integrate into `examples/rtos/` |
