# Examples

> **Navigation**: [Repository Root](../README.md) | [Documentation Root](../docs/README.md)

This directory carries both Rust embedding examples (single-file `*.rs` programs runnable through `cargo run --example <name>`) and standalone Keleusma scripts (`scripts/*.kel` runnable through `keleusma run <path>`). The Rust examples illustrate the embedding API surface from minimal up through end-to-end SDL3 audio and video hosts. The standalone scripts demonstrate language features without the host harness.

Three example directories are crates rather than single files: [`rogue/`](./rogue/) is a feature-gated SDL3 video host, [`rtos/`](./rtos/) is a detached cooperative RTOS microkernel with N6 bare-metal target support, and [`scripts/`](./scripts/) holds standalone `.kel` files. See those directories' own READMEs for details.

## Rust embedding examples

Run any example with `cargo run --release --example <name>`. Examples whose `required-features` include `sdl3-example` need that feature explicitly: `cargo run --release --example <name> --features sdl3-example`.

| Example | What it demonstrates |
|---------|----------------------|
| [`string_ops.rs`](./string_ops.rs) | Basic embedding: compile a script, register a native function, drive `Vm::call`. |
| [`opaque_rust_string.rs`](./opaque_rust_string.rs) | Expose a Rust `String` to scripts as an opaque host value through the `HostOpaque` trait. |
| [`generic_identity.rs`](./generic_identity.rs) | Generic identity function with type-parameter inference. |
| [`generic_match.rs`](./generic_match.rs) | Pattern matching across generic types. |
| [`generic_struct.rs`](./generic_struct.rs) | Generic struct types and field access. |
| [`method_call.rs`](./method_call.rs) | Method-call syntax over user-defined types. |
| [`monomorphize_generic_method.rs`](./monomorphize_generic_method.rs) | Generic method dispatch through compile-time monomorphization. |
| [`struct_method_dispatch.rs`](./struct_method_dispatch.rs) | Method dispatch on struct types via `impl` blocks. |
| [`narrow_runtime.rs`](./narrow_runtime.rs) | Construct a `GenericVm<i16, u16, f32>` for a sub-64-bit native runtime. |
| [`target_aware_compile.rs`](./target_aware_compile.rs) | Compile against a `Target` descriptor (`embedded_16`, `embedded_8`, etc.) so the bytecode matches the deployment target. |
| [`wcmu_basic.rs`](./wcmu_basic.rs) | Auto-size the arena from the verifier's WCMU bound. |
| [`wcmu_attestation.rs`](./wcmu_attestation.rs) | Per-native WCMU attestation through `Vm::set_native_bounds`. |
| [`wcmu_rejection.rs`](./wcmu_rejection.rs) | Demonstrates verifier rejection when the arena capacity falls short of the WCMU bound. |
| [`yield_error.rs`](./yield_error.rs) | Error propagation through resume values across yield boundaries. |
| [`measured_wcet.rs`](./measured_wcet.rs) | Use a measured `CostModel` from `keleusma-bench/measured_cost_models/` for CPU-cycle WCET on the target. |
| [`zero_copy_include_bytes.rs`](./zero_copy_include_bytes.rs) | Embed precompiled bytecode in the binary via `include_bytes!` and execute zero-copy against an `AlignedVec<8>`. |
| [`piano_roll.rs`](./piano_roll.rs) | End-to-end SDL3 audio host with hot code swap across a song roster. Feature-gated on `sdl3-example`. See [`docs/guide/PIANO_ROLL.md`](../docs/guide/PIANO_ROLL.md). |

## Larger example crates

| Path | What it is |
|------|------------|
| [`rogue/`](./rogue/) | End-to-end SDL3 video host driving a roguelike. Nineteen Keleusma scripts for dungeon generation, monster artificial intelligence, combat, and item effects. Feature-gated on `sdl3-example`. See [`docs/guide/ROGUE.md`](../docs/guide/ROGUE.md). |
| [`rtos/`](./rtos/) | Cooperative RTOS microkernel demonstrator. Standalone crate (not a workspace member) with `std-platform` and `stm32n6570dk-platform` builds. Includes the `bench_n6` cost-model calibration binary. See [`examples/rtos/README.md`](./rtos/README.md), [`examples/rtos/MANUAL.md`](./rtos/MANUAL.md), and [`examples/rtos/SPEC.md`](./rtos/SPEC.md). |

## Standalone Keleusma scripts

The [`scripts/`](./scripts/) directory holds `.kel` files runnable through the `keleusma` CLI. Each script is a single Keleusma program demonstrating a specific language feature. See [`scripts/README.md`](./scripts/README.md) for the catalogue. Run any of them with:

```sh
keleusma run examples/scripts/<file>.kel
```

The `keleusma` CLI is installed via `cargo install --path keleusma-cli` (development) or `cargo install keleusma-cli` (released).

## Companion documentation

- [`docs/guide/`](../docs/guide/) — user-facing guides including embedding, RTOS, piano-roll, rogue, and cookbook recipes.
- [`docs/spec/`](../docs/spec/) — authoritative language and instruction-set specifications.
- [`docs/architecture/`](../docs/architecture/) — narrative descriptions of the implemented system.
- [`keleusma-bench/measured_cost_models/`](../keleusma-bench/measured_cost_models/) — pre-generated WCET cost models for supported host architectures and the STM32N6570-DK.
