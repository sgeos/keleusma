# Roadmap

> **Navigation**: [Documentation Root](../README.md)

Development phases for Keleusma. Each document below states its own status, gating, and scope.

The release lines follow a repeating version ladder: each `V0.N.x` **work-line roadmap**
sequences the incremental releases toward the `V0.(N+1).0` **milestone**, and each milestone has
a paired **architecture strategy** document.

| Work line | Milestone | Milestone meaning | Strategy |
|-----------|-----------|-------------------|----------|
| [V0_2_X_ROADMAP.md](./V0_2_X_ROADMAP.md) | V0.3.0 | Full self-hosting solution | [V0_3_0_SELF_HOSTING.md](./V0_3_0_SELF_HOSTING.md) |
| [V0_3_X_ROADMAP.md](./V0_3_X_ROADMAP.md) | V0.4.0 | Full native code generation | [V0_4_0_NATIVE_CODEGEN.md](./V0_4_0_NATIVE_CODEGEN.md) |
| [V0_4_X_ROADMAP.md](./V0_4_X_ROADMAP.md) | V0.5.0 | Rust host retirement (host written in Keleusma) | [V0_5_0_KELEUSMA_HOST.md](./V0_5_0_KELEUSMA_HOST.md) |

## Contents

| Document | Description |
|----------|-------------|
| [V0_2_X_ROADMAP.md](./V0_2_X_ROADMAP.md) | V0.2.x work line: self-host the whole toolchain (compiler, validator, runtime, cryptography) plus a `Trap`-scanning unhandled-trap analysis, subset first; V0.3.0 is the full self-hosting solution |
| [V0_3_X_ROADMAP.md](./V0_3_X_ROADMAP.md) | V0.3.x work line (preliminary): add native code generation; V0.4.0 is full native code generation |
| [V0_4_X_ROADMAP.md](./V0_4_X_ROADMAP.md) | V0.4.x work line (preliminary): migrate the host to Keleusma; V0.5.0 is Rust host retirement |
| [V0_3_0_SELF_HOSTING.md](./V0_3_0_SELF_HOSTING.md) | V0.3.0 strategy: self-hosted compiler as a pipeline of stream-processor stages |
| [V0_4_0_NATIVE_CODEGEN.md](./V0_4_0_NATIVE_CODEGEN.md) | V0.4.0 strategy: native code generation via LLVM; bytecode as verification IR, native as deployment shape; sub-coroutines lowered to LLVM coroutine intrinsics |
| [V0_5_0_KELEUSMA_HOST.md](./V0_5_0_KELEUSMA_HOST.md) | V0.5.0 strategy (preliminary): Keleusma-hosted Keleusma; sub-coroutine primitive as the enabling runtime feature |
| [IMPLEMENTATION_ORDER.md](./IMPLEMENTATION_ORDER.md) | Sequenced implementation plan across V0.3.0, V0.4.0, V0.5.0, and V0.5.x with wall-clock estimates and critical-path identification |
