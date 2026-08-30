# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Keleusma is a Total Functional Stream Processor that compiles to bytecode and runs on a stack-based virtual machine. It is a lightweight, embeddable scripting language targeting `no_std+alloc` environments. The ecosystem value proposition is **definitive WCET and WCMU**. Programs whose worst-case execution time or worst-case memory usage cannot be statically bounded are rejected by the safe verifier. Without host-plugged functions, the language admits only pure total functions and the productive divergent `loop` block. All domain functionality is provided by native Rust functions registered by the host application.

**Status**: V0.2.3 in development (the nominal next publication, not yet released); work lands on short-lived feature branches cut from `v0.2.3` and merged back after a green gate. The V0.2.x line is groundwork that supports an eventual self-hosted compiler (the V0.3.0 goal, a Keleusma compiler written in Keleusma) without achieving self-hosting; incremental language, standard-library, and tooling work that such a compiler will depend on lands across these releases, and no self-hosted compiler ships in the V0.2.x line. The self-hosted compiler's reusable core — the four pipeline stages (`lexer`, `parse`, `reconstruct`, `codegen`), `analyze.kel`, the `verify_*.kel` family, and the Rust driver — now lives canonically in the shipping `keleusma` crate at `src/selfhost/` (the driver `src/selfhost/mod.rs` behind an off-by-default `self-host` cargo feature; the twelve stage sources under `src/selfhost/kel/`, eleven of them embedded in the driver via `include_str!` and `verify_types.kel` embedded by its tests), so it can ship in the CLI. The self-hosted compiler is selectable in the shipping binary with `keleusma compile --compiler self-hosted` (default `rust`, the reference compiler): it runs the user source through the Keleusma-written pipeline, accepts only the self-hosted subset at the host target, and fails loudly (its output is cross-checked against the reference, so any divergence — floats, generics, `Text`, a non-host target — errors with a `retry with --compiler rust` hint) rather than emitting a wrong module. The `compiler/` subproject remains a detached-workspace package (like `examples/rtos/`, excluded from the crate tarball) that re-exports the driver (`compiler/src/selfhost.rs` is now a thin `pub use keleusma::selfhost::*`), keeps only `prelude.kel` under `compiler/kel/`, and holds the bootstrap harness and its status/fixed-point tooling in `compiler/src/`. See `compiler/README.md`, the release plan in `compiler/MILESTONES.md`, and the authoritative design in `docs/roadmap/V0_3_0_SELF_HOSTING.md`. V0.2.2 was released: published to crates.io as `keleusma` 0.2.2, `keleusma-cli` 0.2.2, `keleusma-bench` 0.2.2, `keleusma-macros` 0.2.2, and `keleusma-arena` 0.3.1, and tagged `v0.2.2`; it repaired V0.2.1's cross-target regressions (a 32-bit `no_std`/embedded build failure and a verify-without-floats build failure surfaced by stable 1.97+), ported the learning guide to a bilingual mdbook under `book/`, scaffolded the self-hosted compiler subproject at `compiler/`, and codified the release process with a mandatory CI-green gate, with no wire-format or `BYTECODE_VERSION` change. V0.2.1 was released: published to crates.io as `keleusma` 0.2.1, `keleusma-cli` 0.2.1, `keleusma-bench` 0.2.1, `keleusma-macros` 0.2.1, and `keleusma-arena` 0.3.1, and tagged `v0.2.1`. V0.2.0 published cryptographic module signing (Ed25519), the V0.2.0 ISA reset (fixed-size opcode records, a separately addressed operand pool, a section-partitioned body), information-flow labels including negative labels, calibrated WCET cost models via the `keleusma-bench` crate, and a docs/spec/ reorganization that consolidates authoritative specifications. V0.2.1 completes the B28 flat-byte composite representation: composite `Value` bodies are pure flat bytes resident in the host arena (no global-heap `Vec`/`String` indirection), the `Value` slot is 32 bytes (down from 40, pinned by a `const` size assertion), the P4 `NewComposite` consolidation took the instruction set from 69 to 66 opcodes, the `shared data` segment became a host-owned borrowed `&mut [u8]` buffer driven through `call_with_shared`/`resume_with_shared` and `get_shared`/`set_shared` (the `set_data`/`get_data` slot vector is removed), private composite data persists in the arena's persistent region across RESET, and worst-case-memory-usage bounds are correspondingly tighter; B28 also subsumes B26 and B27. V0.2.1 also lands the B19 operator surface, the B40 general const generics, and the Annex A.2.1 typed operand-stack verifier pass (all built on top of B28). B19 adds the multi-word fixed-point family `Multiword<N, F>` (construction, indexing, scale-independent add and subtract, the six comparisons, integer and fixed-point multiply, divide and modulo, the four assembly-mnemonic shifts `lsl`/`asl`/`lsr`/`asr` with a constant or runtime-variable amount, and the per-limb bitwise `band`/`bor`/`bxor`/`bnot`), plus the eager `and`/`or`/`xor`/`not` and short-circuit `andalso`/`orelse` booleans; `Byte` is admitted by the scalar shift and bitwise operators through promote-operate-truncate masking, and no opcode is added. B40 adds const parameters on functions, structs, and enums, usable as `Word` values in bodies, with total const arithmetic over `+`, `-`, `*`; const parameters are fully erased to literals at monomorphization so the WCET and WCMU analyses see no symbolic constant, and no opcode or `BYTECODE_VERSION` change results. The A.2.1 typed operand-stack verifier pass (`src/verify_typed.rs`) is wired into `verify()`, so every module load and hot swap reconstructs the flat shape of each operand-stack entry and local slot by a JVM/WebAssembly-style type-preservation abstract interpretation and validates every compiler-baked composite, field, and array-element offset against the canonical flat layout, closing audit findings B1, B2, B6, and B8 and the structural stack-balance holes 3, B4, and B5; operand shapes are seeded from three additive auxiliary-body descriptor tables (per-chunk signatures, native returns, and enum layouts) carried with no `BYTECODE_VERSION` change. The pass runs in a sound defer-on-unknown mode: an operand whose flat shape it cannot reconstruct (an unsignatured native result or a per-yield reentrant reply) defers to a retained runtime bounds guard rather than a load-time rejection, and `FlatComposite::nested_view` is correspondingly hardened from a `debug_assert` to a real fault so a release build never performs out-of-bounds pointer arithmetic on a corrupt nested-composite offset. See `docs/standard/STANDARD.md` Standard 8.2 and Annex A. V0.1.x retired surface features (closures, f-strings, `text` bundled DSL) are gone; programs that used them must be rewritten under host-registered natives. The runtime is at `BYTECODE_VERSION = 2`, and the number is fixed at 2 for now. The V0.2.3 line replaced the auxiliary-body encoding wholesale with wire format v2, a word-oriented container built by the standalone `keleusma-wire` crate and given meaning by Keleusma's schema layer in `src/wire_schema.rs`. The container provides a triplicated prologue and region directory read by majority-of-three vote, fixed-stride record tables, byte-addressed pools, CRC-32, and an optional (72,64) SECDED parity plane held parallel to the data; rkyv no longer encodes or reads the auxiliary body, though the dependency remains for six unrelated `AlignedVec` buffer-alignment uses. The operator authorized the bump on 2026-08-06 on the grounds that the substrate itself had changed. That supersedes the earlier no-public-adoption policy, under which the number was held at 1 through the V0.2.3 widening of the shared-data operands from 16 to 24 bits, raising the shared ceiling from 64 KB to 16 MB. Because the number moved, the hazard that policy accepted is closed: a version-1 artifact is now rejected on the version check rather than accepted and then mis-read. Any further bump still requires operator authorization. Measured 2026-08-28 at `b2ce257a`: 1263 keleusma lib tests (`cargo test --lib --features self-host`; 1256 under default features) plus 1194 integration `#[test]` functions across 90 files (`ls tests/*.rs | wc -l`), covering among others the rogue-script and marshall suites, the multi-word fixed-point suite, the const-generics suite, the struct-field-index and generic-methods suites, the flat-composite, persistent-data, and narrow-word VM suites, the last also covering multi-word arithmetic at a 16-bit word width, and the typed-verifier conformance corpus that mutates real bytecode per audit finding; 59 keleusma-arena (51 lib plus 8 integration) and 6 keleusma-bench tests across the workspace, all passing under the three feature sets continuous integration actually runs, which are default features, `--features signatures,shell`, and `--features self-host`. **`--all-features` is not one of them and does not pass.** It cascades the mutually exclusive `narrow-word-*` and `narrow-address-*` selectors into the narrowest configuration, under which a test that pins 64-bit checked-addition semantics fails, and it pulls in `sdl3-example`, which builds SDL3 from source. The continuous-integration workflow says so in a comment on its broad-features job; this file claimed the opposite until 2026-08-16. Hindley-Milner inference, generics with traits and bounds, compile-time monomorphization, target descriptor for cross-architecture portability, hot code swap, and the conservative-verification stance remain in place.

**Conservative-verification stance.** The compile pipeline admits a broader surface than the WCET and WCMU analyses can prove bounded. The verifier rejects programs whose bound is unprovable (first category) or whose bound is provable in principle but the analysis is not yet implemented (second category). See [`docs/architecture/LANGUAGE_DESIGN.md`](docs/architecture/LANGUAGE_DESIGN.md#conservative-verification) for the full statement. `Vm::new_unchecked` exists for trust-skip of precompiled bytecode and is intentional misuse if used to admit programs that would fail verification.

**Engineering Classification**: Library. See `docs/process/PROCESS_STRATEGY.md`.

## Repository Structure

```
keleusma/
├── CLAUDE.md                  # AI agent instructions
├── Cargo.toml                 # Workspace + runtime package definition
├── src/                       # Runtime package source
│   ├── lib.rs                 # Crate root (no_std, module declarations, re-exports)
│   ├── token.rs               # Token definitions and keyword recognition
│   ├── lexer.rs               # Tokenization (public API: tokenize), includes f-string desugaring
│   ├── ast.rs                 # Abstract Syntax Tree node definitions
│   ├── parser.rs              # Recursive descent parser (public API: parse)
│   ├── visitor.rs             # MutVisitor and Visitor traits with default walk methods over Block, Stmt, Expr, Iterable
│   ├── typecheck.rs           # Hindley-Milner type checker (public API: check), generics, traits, impl method validation
│   ├── monomorphize.rs        # Compile-time monomorphization for generic functions, structs, enums (public API: monomorphize)
│   ├── target.rs              # Target descriptor for cross-architecture portability (public API: Target)
│   ├── bytecode.rs            # Runtime values, instruction set, wire format, target-aware width fields
│   ├── compiler.rs            # Source-to-bytecode compilation (public API: compile, compile_with_target)
│   ├── vm.rs                  # Stack-based VM with coroutine support (public API: Vm), per-op decode cache
│   ├── verify.rs              # Structural verifier (public API: verify, wcet_stream_iteration, wcmu_stream_iteration, verify_resource_bounds, module_wcmu)
│   ├── marshall.rs            # KeleusmaType trait and IntoNativeFn family
│   ├── audio_natives.rs       # Built-in audio and math native functions
│   ├── utility_natives.rs     # to_string, length, concat, slice, println, math utilities
│   ├── value_layout.rs        # Canonical flat layout: ScalarKind, CompositeKind, offsets
│   ├── flat_value.rs          # FlatComposite and its nested views over arena bytes
│   ├── wire_schema.rs         # Meaning for the wire-format v2 container
│   ├── selfhost_host.rs       # Shared-slot layouts the stages are seeded through
│   ├── confine.rs             # Composite-escape confinement analysis
│   ├── word.rs, float.rs, address.rs   # Width-parameterised numeric and address traits
│   ├── …                      # And further modules; this listing is ILLUSTRATIVE, not complete
│   └── selfhost/              # Self-hosted compile driver (feature `self-host`, off by default)
│       ├── mod.rs             # Driver + self_hosted_compile (CLI --compiler self-hosted backend)
│       └── kel/               # The twelve stage sources (lexer/parse/reconstruct/codegen/wire/analyze/verify_*)
├── tests/                     # Integration tests
│   └── marshall.rs            # KeleusmaType derive and register_fn end-to-end
├── keleusma-macros/           # Proc-macro crate (workspace member)
│   ├── Cargo.toml
│   └── src/lib.rs             # #[derive(KeleusmaType)]
├── keleusma-arena/            # Standalone arena allocator (workspace member)
│   ├── Cargo.toml
│   ├── README.md
│   └── src/lib.rs             # Arena, BottomHandle, TopHandle, Budget, marks
├── keleusma-wire-derive/      # Derive macro for keleusma-wire records (workspace member)
├── keleusma-wire/             # Standalone wire-format container (workspace member)
│   ├── Cargo.toml
│   ├── README.md
│   └── src/                   # layout, scalar, crc, view (reader), build (encoder)
├── examples/
│   ├── rogue/                 # Roguelike example (workspace [[example]])
│   ├── piano_roll.rs          # SDL3 audio + hot-swap (workspace [[example]])
│   ├── rtos/                  # Cooperative RTOS microkernel (standalone crate, not a workspace member)
│   │   ├── Cargo.toml         # Detached [workspace]; embassy git deps under stm32n6570dk-platform feature
│   │   ├── README.md          # Overview, quick-start commands, file table
│   │   ├── MANUAL.md          # Operator manual: hardware setup, build matrix, troubleshooting
│   │   ├── SPEC.md            # Architectural rationale and roadmap
│   │   ├── memory.x           # AXISRAM2 layout for the STM32N6570-DK bin
│   │   ├── build.rs           # Target-conditional link args (no_std target only)
│   │   ├── .cargo/config.toml # probe-rs runner for thumbv8m.main-none-eabihf
│   │   ├── scripts/           # Keleusma scripts (prelude, led, sensor, heartbeat)
│   │   └── src/               # Kernel core, platform impls, natives, bins
│   └── …                      # Other Rust embedding examples and standalone .kel scripts
└── docs/                      # Documentation knowledge graph
    ├── README.md              # Documentation root
    ├── DOCUMENTATION_STRATEGY.md
    ├── architecture/          # Language design and compilation pipeline
    ├── design/                # Grammar, type system, standard library
    ├── decisions/             # Resolved, priority, and backlog decisions
    ├── process/               # Workflow, communication, and task tracking
    ├── reference/             # Glossary, related work (the instruction set is in spec/)
    └── roadmap/               # Development phases
```

## Documentation

A knowledge graph is maintained in `docs/`. Start at [`docs/README.md`](docs/README.md) for navigation.

| Section | Path | Description |
|---------|------|-------------|
| Guide | [`book/src/`](book/src/introduction.md) | Onboarding for new users and embedders |
| Architecture | [`docs/architecture/`](docs/architecture/README.md) | Narrative descriptions of the implemented system |
| Spec | [`docs/spec/`](docs/spec/README.md) | Authoritative specifications: grammar, type system, standard library, instruction set, structural ISA, wire format |
| Decisions | [`docs/decisions/`](docs/decisions/README.md) | Architectural and design decisions |
| Process | [`docs/process/`](docs/process/README.md) | Development workflow and task tracking |
| Reference | [`docs/reference/`](docs/reference/README.md) | Glossary and citations |
| Roadmap | [`docs/roadmap/`](docs/roadmap/README.md) | Development phases |
| Extras | [`docs/extras/`](docs/extras/README.md) | Supplementary references for specific examples |

## Development Process

See `docs/process/PROCESS_STRATEGY.md` for the library engineering approach and agentic development loop.

**Session startup protocol**:
1. Read [`docs/process/HANDOFF.md`](docs/process/HANDOFF.md) and run its validity check (compare its recorded parent commit to `git rev-parse HEAD~1`). Report the handoff as valid, or as invalid-and-stale on a mismatch, per its Validity section.
2. Read [`docs/process/TASKLOG.md`](docs/process/TASKLOG.md) for current task state.
3. Read [`docs/process/REVERSE_PROMPT.md`](docs/process/REVERSE_PROMPT.md) for last AI communication.
4. Wait for human prompt before proceeding.

**After completing each task**:
1. Update task status in `docs/process/TASKLOG.md`.
2. Overwrite `docs/process/REVERSE_PROMPT.md` with verification, questions, concerns, and intended next step.
3. Commit changes with conventional commit referencing the task.
4. If blocked or uncertain, document in REVERSE_PROMPT.md and **stop**.

**Working documents**:

| File | Purpose |
|------|---------|
| `docs/process/TASKLOG.md` | Current sprint source of truth |
| `docs/process/PROMPT.md` | Human to AI instruction staging (read-only for AI) |
| `docs/process/REVERSE_PROMPT.md` | AI to Human communication |

## Compact Instructions

When compacting this conversation (automatically or via `/compact`), preserve the following so a
post-compaction turn resumes the autonomy loop without loss. Prefer pointers to the on-disk source of
truth over prose, since these files are authoritative and current, and the summary is a convenience,
not the source of truth.

- **The handoff prompt** [`docs/process/HANDOFF.md`](docs/process/HANDOFF.md), the self-contained
  imperative resume prompt. It is written/overwritten before a planned compaction and stamped with the
  commit it describes. On resume, validate it (compare its recorded parent commit to `git rev-parse
  HEAD~1`); report it invalid-and-stale on a mismatch rather than trusting it.
- **The three resume channels**, plus the instruction to re-read them fresh after compaction:
  [`docs/process/REVERSE_PROMPT.md`](docs/process/REVERSE_PROMPT.md) (bounded latest state and the
  next intended increment), [`docs/process/DESIGN_JOURNAL.md`](docs/process/DESIGN_JOURNAL.md)
  (append-only increment reasoning, newest first), and
  [`docs/process/TASKLOG.md`](docs/process/TASKLOG.md) (current sprint state).
- **The active increment and its plan.** Which self-hosted-compiler gap is in progress or next, and
  the path to its plan document under `docs/decisions/`. Do not re-derive a plan a persisted document
  already holds.
- **The construct-support boundary counts** (Ok / Gap / RefRejects), pinned by
  `self_hosted_construct_support_boundary` in `tests/selfhost_codegen.rs`.
- **Git position.** The active branch, its head commit, whether it is merged, and the origin state of
  the version branch. Preserve any uncommitted or unmerged work and its verification status.
- **In-flight verification.** Any running gate, CI run, or background agent, and what its result gates.
- **The governing rules** that are easy to lose: the release-branch git strategy
  ([`docs/process/GIT_STRATEGY.md`](docs/process/GIT_STRATEGY.md)), the rad-hard minimal-ISA
  no-new-opcode constraint, the no-`BYTECODE_VERSION`-bump-without-authorization rule, the
  byte-identical differential oracle as the correctness signal, and that irreversible or
  outward-facing actions need confirmation.

After compaction, before acting, validate `HANDOFF.md` and re-read the three resume channels and the
active plan document. They, the boundary test counts, and the git state are the true resume anchors.

## Git Workflow

Release-branch model with a four-level hierarchy: `main` holds releases (always green; releases cut only from a green `main`); a `vX.Y.Z` version branch integrates the next version (green before merging to `main`); short-lived feature branches are cut from the version branch (intermediate commits may be red, tip green before merge) and merged back via a **no-fast-forward merge commit**; sub-feature branches are cut from and merged back into a feature. A merge proceeds on a green local `scripts/release-gate.sh`, with CI binding afterward (a red result remedied immediately). Direct commits to the version branch are allowed only for small green docs/process changes; all code flows through a feature branch. See [`docs/process/GIT_STRATEGY.md`](docs/process/GIT_STRATEGY.md) for full details. For running multiple agents concurrently (worktree isolation via `scripts/worktree.sh`, per-branch handoffs, and merge/gate serialization) see [`docs/process/PARALLEL_DEVELOPMENT.md`](docs/process/PARALLEL_DEVELOPMENT.md).

Use scoped conventional commits: `<scope>: <imperative summary>`. Common scopes: `feat`, `fix`, `docs`, `refactor`, `chore`, `test`. Include `Co-Authored-By: Claude <noreply@anthropic.com>` when AI-assisted.

The AI agent commits once after all tasks in a prompt are complete, including the `REVERSE_PROMPT.md` update. `PROMPT.md` is read-only for the AI agent but must be included in the commit if the human pilot has modified it.

## Common Commands

```bash
# Build
cargo build

# Run tests
cargo test

# Check without building
cargo check

# Format and lint
cargo fmt
cargo clippy -- -D warnings

# Everyday verification
cargo fmt --check && cargo clippy --tests --features signatures,shell,self-host -- -D warnings && cargo test

# Documentation gate (broken/private intra-doc links fail here, not in test/clippy)
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Pre-release gate — mirrors CI; run whole before publishing (add --miri for a release)
scripts/release-gate.sh
```

The everyday verification omits the documentation build; a broken intra-doc link is
invisible to `test`/`clippy` and only fails the CI `Doc` job. Before any publication,
run the full `scripts/release-gate.sh` (which includes `cargo doc -D warnings`) and
follow [`docs/process/RELEASE_PROCESS.md`](docs/process/RELEASE_PROCESS.md). Skipping
the doc build is how V0.2.1 shipped with a red CI Doc job.

## Coding Conventions

### no_std + alloc

This crate targets `no_std` with `alloc`. All allocations use `alloc` collections (`Vec`, `String`, `BTreeMap`). No standard library types.

### Generics Over Dynamic Dispatch

Prefer trait-bounded generics over dynamically dispatched trait objects (`&dyn Trait`). Define type aliases and trait bounds at the top of the file to keep generic signatures readable.

### Functional Core

Prefer pure functions that take inputs and return outputs without side effects. State mutation should be confined to the VM execution loop.

### Error Handling

All public API functions return `Result` types with error structs that include source location (`Span`) for precise error reporting. Error types: `LexError`, `ParseError`, `CompileError`, `VmError`.

## Technology Stack

- **Rust** (edition 2024)
- **no_std + alloc** (no standard library dependency)
- **libm 0.2** (math functions for no_std environments)
- **allocator-api2 0.4** (stable polyfill of the unstable allocator API, used by `keleusma-arena`)
- **syn 2, quote 1, proc-macro2 1** (compile-time only, used by `keleusma-macros`)
- **rkyv 0.8** (zero-copy archived bytecode format)
- Cargo workspace with members: `keleusma` (runtime), `keleusma-macros` (proc-macro), `keleusma-arena` (standalone arena allocator), `keleusma-wire` (standalone wire-format container), `keleusma-bench` (cost-model calibration), `keleusma-cli` (CLI frontend), and `keleusma-wire-derive` (the derive macro backing `keleusma-wire`'s `derive` feature; an implementation detail, not a direct dependency).
- Measured 2026-08-28 at `b2ce257a`: 1263 keleusma lib tests (`cargo test --lib --features self-host`; 1256 under default features) plus 1194 integration `#[test]` functions across 90 files (`ls tests/*.rs | wc -l`), 59 keleusma-arena (51 lib plus 8 integration) and 6 keleusma-bench tests across the workspace. **These figures move with every increment; re-derive them rather than trusting the number.** They cover the rogue-script and marshall suites, the multi-word fixed-point suite, the const-generics suite, the struct-field-index and generic-methods suites, the flat-composite, persistent-data, narrow-word VM, and zero-copy suites, and more broadly lexer, parser, type checker, monomorphizer, compiler, VM, verifier, marshall, flat-byte composites, multi-word fixed-point arithmetic, const generics, trait methods on generic types, operator families (bitwise, shift, boolean), arena, audio natives, utility natives, target descriptor, visitor pattern, signing, IFC labels, cost-model calibration, and integration tests.
- The `examples/rtos/` directory carries a standalone crate (not a workspace member) implementing a cooperative RTOS microkernel; it depends on the parent `keleusma` runtime by path and ships its own toolchain pin, build.rs, memory.x, and probe-rs runner. Run with `cd examples/rtos && cargo run --release --bin three-task-std` (host) or `cd examples/rtos && cargo run --release --bin three-task-n6 --target thumbv8m.main-none-eabihf --no-default-features --features stm32n6570dk-platform` (STM32N6570-DK). See `examples/rtos/MANUAL.md`.
