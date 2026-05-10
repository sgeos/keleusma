# Changelog

All notable changes to `keleusma` will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
- Bytecode wire format with magic, length, version, target word and address widths, body, and CRC trailer. Self-describing through the framing header.
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
