# Roadmap

> **Navigation**: [Documentation Root](../README.md)

Development phases for Keleusma. Each strategy document below states its own status, gating, and scope.

## Contents

| Document | Description |
|----------|-------------|
| [V0_2_X_ROADMAP.md](./V0_2_X_ROADMAP.md) | V0.2.x release-line roadmap: self-host the whole toolchain (compiler, validator, runtime, cryptography) plus a new unhandled-trap analysis, over a subset first; full-language support defines V0.3.0 |
| [V0_3_0_SELF_HOSTING.md](./V0_3_0_SELF_HOSTING.md) | V0.3.0 strategy: self-hosted compiler as a pipeline of stream-processor stages |
| [V0_4_0_NATIVE_CODEGEN.md](./V0_4_0_NATIVE_CODEGEN.md) | V0.4.0 strategy: native code generation via LLVM; bytecode as verification IR, native as deployment shape; sub-coroutines lowered to LLVM coroutine intrinsics |
| [V0_5_0_KELEUSMA_HOST.md](./V0_5_0_KELEUSMA_HOST.md) | V0.5.0 strategy (preliminary): Keleusma-hosted Keleusma; sub-coroutine primitive as the enabling runtime feature |
| [IMPLEMENTATION_ORDER.md](./IMPLEMENTATION_ORDER.md) | Sequenced implementation plan across V0.3.0, V0.4.0, V0.5.0, and V0.5.x with wall-clock estimates and critical-path identification |
