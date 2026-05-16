# Changelog

All notable changes to `keleusma` will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Surface text type renamed `String` to `Text`.** The Keleusma surface keyword for textual data is now `Text`. The former name persistently confused readers given Rust's owned `String` type. The runtime representation (`Value::StaticStr`, `Value::KStr`) is unchanged. Existing scripts must rename `String` to `Text` in parameter and return-type annotations.
- **`Op::Add` on text operands is now arena-resident.** Concatenation through `+` no longer routes through the global allocator; the result is `Value::KStr` allocated through `KString::alloc` in the arena's top region. Allocation failure surfaces as `VmError::OutOfArena`. The bundled `to_string`, `concat`, and `slice` natives likewise produce arena-allocated `KStr` results.
- **`Value::DynStr` removed.** The global-heap dynamic-string variant present in V0.1.x is gone. All dynamic strings are arena-resident via `Value::KStr`. The cross-yield prohibition continues to apply.
- **`register_utility_natives` is now arena-aware by default.** The non-context variants of `to_string`, `concat`, and `slice` were removed. `register_utility_natives_with_ctx` is retained as a deprecated alias for compatibility.

### Added

- **`OpCost::{Fixed(u32), Dynamic(fn)}` enum.** Cost-model surface for opcodes whose cost depends on runtime data. `CostModel::heap_alloc_cost` returns the new type; `Op::Add` on text operands reports `OpCost::Dynamic` because the resulting `KString` length is the sum of operand lengths. Hosts that need the pre-V0.2.0 fixed view continue to call `CostModel::heap_alloc_bytes`, which saturates dynamic costs to zero. The WCMU text-size tracking pass scheduled for V0.2.x evaluates `OpCost::Dynamic` against an `OpCostContext` populated from the abstract-interpretation lattice.
- **`text` cargo feature, default off.** Gates the surface use of strings in scripts. With the feature off, the lexer rejects string literals (`"..."`) and f-strings (`f"..."`), and the parser does not recognise the `Text` primitive type. The `keleusma-cli` crate enables the feature on the runtime dependency so the script runner and the REPL continue to handle strings. Hosts that want the V0.1.x default surface enable the feature explicitly: `keleusma = { version = "0.2", features = ["text"] }`. Embedding hosts that target very small runtimes get a smaller compiled artifact by leaving the feature off. See the FAQ entry "Enabling text support" for details.
- **`Vm::new_with_options` and overflow policy knob.** New constructor accepting a `VmOptions` value with an `overflow_policy` field. The policy decides what happens when a module's declared WCET or WCMU header field saturated to `u32::MAX` during compilation. `OverflowPolicy::Reject` (default) treats overflow as a `VmError::VerifyError`, preserving the historic strict admissibility. `OverflowPolicy::Warn` admits the module and returns a `Vec<VerifyWarning>` describing the overflow. `OverflowPolicy::Allow` admits the module silently. The bare `Vm::new` is now a thin wrapper around `new_with_options(VmOptions::default())` and continues to reject overflow.
- **WCMU text-size tracking via abstract interpretation.** New `keleusma::text_size` module introduces the `TextSize::{NotText, Known(u32), Unbounded}` lattice with saturating addition, join, and projection into the `OpCostContext` consumed by `OpCost::Dynamic` cost evaluators. The `chunk_text_heap_alloc` function walks each chunk's bytecode linearly, mirroring the operand stack and local variables as `TextSize` lattice values, and accumulates the dynamic heap cost of every text-producing `Op::Add`. `verify::compute_chunk_wcmu` calls this pass and adds its result to the chunk's heap WCMU bound. Programs whose text allocations saturate the bound to `u32::MAX` are rejected at `Vm::new` under the default `OverflowPolicy::Reject`. The FAQ exponential-string-concat example is correctly rejected when written as a Stream block; the analysis is conservative for text operations inside loops, behind conditional branches, and against native-produced text.
- **Opaque type support.** New `keleusma::opaque` module introduces the `HostOpaque` marker trait and the `Value::Opaque(Arc<dyn HostOpaque>)` runtime variant. Host applications register Rust types as opaque values from the script's perspective; the script declares the type by name in function signatures (the type checker resolves unknown named types as `Type::Opaque`). Native functions produce opaque values through the `host_arc` constructor; consumers extract a typed reference through `dyn HostOpaque::downcast_ref`. Opaque values are host-managed through `Arc`, have a lifetime independent of the arena, may flow through the dialogue type at a yield, and contribute zero to the script-side WCMU bound. Equality is by `Arc` pointer identity. A small sealed supertrait surfaces the concrete `TypeId` without requiring `core::any::Any`.

## [0.1.1] - 2026-05-10

### Fixed

- **MSRV claim corrected.** `keleusma 0.1.0` declared `rust-version = "1.87"`, but the source uses let-chains (`if let X = a && let Y = b`) which were stabilized in Rust 1.88. The CI MSRV job surfaced the mismatch immediately after the 0.1.0 publish. This release bumps `rust-version` to `1.88` to match the actual feature use. Users on Rust 1.87 should pin `keleusma = "0.1.1"` or upgrade their toolchain. The CI workflow's `msrv-keleusma` job is correspondingly bumped from 1.87 to 1.88 so future MSRV drift is caught locally rather than at publish time. No source changes; runtime behavior is identical to 0.1.0.

## [0.1.0] - 2026-05-10

Initial release.

### Language

- Three function categories. `fn` for atomic-total computation, `yield` for non-atomic-total coroutines, `loop` for productive-divergent stream functions. Exactly one `loop` per script.
- Five static guarantees. Totality, productivity, bounded-step (WCET), bounded-memory (WCMU), and safe hot-swap.
- Hindley-Milner type inference with Robinson unification and the occurs check. Generic functions, structs, and enums with type parameters and trait bounds. Compile-time monomorphization across literals, identifiers, function-call returns, method-call returns, casts, enum variants, struct constructions, tuple and array literals, if and match arms, field access, tuple-index, and array-index.
- Closures and anonymous functions including environment capture and transitive nested capture. The safe verifier rejects programs that invoke closures through `Op::CallIndirect` because indirect dispatch cannot be statically bounded.
- Multiheaded function dispatch in Elixir style. Pattern-matched function heads tried in source order.
- Pipeline operator `|>` threading the left expression as the first argument to the right call.
- F-string interpolation with `f"text {expr}"` desugaring at lex time to `to_string` and `concat` calls.
- Two-string-type discipline. Static strings reside in the rodata region. Dynamic strings reside in the arena heap and may not cross the yield boundary.
- Data segment as the sole region of mutable state observable to the script. Schema declared through a single `data <name> { fields }` block per module.

### Runtime

- Stack-based virtual machine over a fifty-six-opcode block-structured ISA. `no_std + alloc` target.
- Dual-end bump-allocated arena via the `keleusma-arena` crate, used for the operand stack at the bottom and dynamic strings at the top.
- `KString` newtype around `keleusma_arena::ArenaHandle<str>` for arena-backed dynamic-string handles with epoch-tagged stale-pointer detection. The `&str` copy semantics live in the runtime crate; the generic epoch-handle mechanism remains in `keleusma-arena`.
- Hot code swap at the reset boundary of a `loop` script. Dialogue type, the yielded type and the resume type, must remain stable across swaps. Native registrations persist; the data segment is supplied fresh by the host.
- Bytecode wire format with magic, length, version (reset to 1 for the initial public release), target word and address widths, declared WCET in pipelined cycles per Stream-to-Reset slice, declared WCMU in bytes per Stream-to-Reset slice, body, and CRC-32 trailer. Self-describing through the framing header. Header WCET and WCMU fields use `0` to mean "auto" (runtime computes via verifier) and `u32::MAX` to mean "overflow" (rejected at safe `Vm::new`). The compiler populates declared values for Stream programs from `wcet_stream_iteration` and `wcmu_stream_iteration`; atomic-total programs ship with `0`.
- Shebang execution. Source scripts may begin with a `#!/usr/bin/env keleusma` line which the lexer skips before tokenizing; the keleusma CLI accepts any readable file path (extensionless scripts work). Compiled bytecode files may also be Unix-executable through a `#!/usr/bin/env keleusma` envelope prepended to the binary; `Module::from_bytes` strips the envelope before validating the magic and CRC residue. The CLI auto-detects bytecode versus source by inspecting the first bytes after any shebang.
- Multiheaded function dispatch with guard clauses (`fn name(pat) -> T when expr { body }`). Already supported in the parser, type checker, and compiler.
- Zero-copy execution against borrowed `rkyv` archived bytecode through the `Vm::view_bytes_zero_copy` constructor.

### Verification

- Structural verifier covering block-structured control flow, productivity rule for stream blocks, and resource bounds against the arena capacity.
- WCET analysis in pipelined cycles. WCMU analysis in bytes. Bundled `NOMINAL_COST_MODEL` provides unmeasured estimates suitable for relative ordering of programs on a single platform; hosts construct a calibrated cost model by setting `op_cycles` to a function returning measured cycle counts.
- Conservative-verification stance. Programs whose bound is not statically provable are rejected at the safe constructor. The unbounded escape hatch is `Vm::new_unchecked` and is intentional misuse outside the WCET contract.
- Native attestation via `Vm::set_native_bounds` for declaring per-native WCET and WCMU bounds.

### Host Interface

- Four native registration paths from most ergonomic to most flexible. `register_fn` accepts ordinary Rust functions and closures of arity zero through four whose argument and return types implement `KeleusmaType`. `register_fn_fallible` accepts the same surface with `Result<R, VmError>` return. `register_native` and `register_native_closure` accept raw `Value` slices for functions that need to inspect arbitrary variants.
- `KeleusmaType` derive via the `keleusma-macros` proc-macro crate. Named-field structs and enums with unit, tuple, or struct-style variants compose admissible interop types.
- Coroutine drive via `Vm::call(args)` and `Vm::resume(input)` returning `VmState::Yielded`, `VmState::Reset`, or `VmState::Finished`.
- Error recovery via `Vm::reset_after_error` clearing volatile state while preserving the data segment.

### Tooling

- Standalone `keleusma` CLI in the `keleusma-cli` workspace member providing `run`, `compile`, and `repl` subcommands, modeled after the Rhai CLI ergonomics.
- Cost-model calibration tool in the `keleusma-bench` workspace member, emitting a measured `CostModel` source fragment for the host CPU. Architecture extensibility through the `CycleCounter` trait with built-in implementations for x86_64 (RDTSC), AArch64 (CNTVCT_EL0), and a portable `Instant`-based fallback.

### Examples

- Eight standalone scripts under `examples/scripts/` covering primitives, structs, enums, for-in iteration, the pipeline operator, multiheaded dispatch, f-string interpolation, and trait method dispatch. Each runs through `keleusma run`.
- Rust embedding examples covering WCMU computation, native attestation, error propagation through yield, string interoperability, generics and method dispatch, target-aware compilation, and zero-copy execution.
- Feature-gated end-to-end SDL3 audio demonstration `piano_roll`. Three voices sequenced by a Keleusma tick loop with hot code swap between two precompiled songs. Run with `cargo run --release --example piano_roll --features sdl3-example`.

### Documentation

- Knowledge graph under `docs/` covering language design, execution model, compilation pipeline, grammar, type system, instruction set, decisions, and process.
- Onboarding section under `docs/guide/` with three audience-focused documents. `GETTING_STARTED.md` for installation through embedding, `EMBEDDING.md` for the host-facing API surface, `WHY_REJECTED.md` for verifier rejection interpretation.

### Licensed

- BSD Zero Clause License (`0BSD`).

### Notes

This is the initial public release. The 0.x version line indicates that breaking changes are expected as the language and host API mature. Workspace members `keleusma-macros` and `keleusma-arena` are versioned independently. `keleusma-arena` is generally useful as a standalone allocator. `keleusma-macros` is the proc-macro implementation crate for the `KeleusmaType` derive and is published only because Cargo requires proc-macro crates to be separate; users should consume the derive through `keleusma::KeleusmaType` and treat `keleusma-macros` as an implementation detail.
