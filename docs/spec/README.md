# Spec

> **Navigation**: [Documentation Root](../README.md)

Authoritative specifications for Keleusma. These documents are the canonical definitions of the language, the bytecode, and the binary format. They are written for reference rather than narrative reading; consult specific entries as needed.

For conceptual descriptions of the implemented system (language design philosophy, execution model, compilation pipeline, sub-coroutine architecture), see [architecture/](../architecture/README.md). For non-spec lookup material (terminology, citations), see [reference/](../reference/README.md).

## Contents

| Document | Description |
|----------|-------------|
| [GRAMMAR.md](./GRAMMAR.md) | Formal EBNF grammar specification of the surface language |
| [TYPE_SYSTEM.md](./TYPE_SYSTEM.md) | Primitive types, composite types, opaque types, type checking rules |
| [STANDARD_LIBRARY.md](./STANDARD_LIBRARY.md) | Built-in bundles: `Math`, `Audio`, `Shell` native function signatures |
| [INSTRUCTION_SET.md](./INSTRUCTION_SET.md) | Bytecode instruction reference with per-opcode operands, behaviour, and cost contributions to WCET and WCMU analyses |
| [STRUCTURAL_ISA.md](./STRUCTURAL_ISA.md) | Block-structured Instruction Set Architecture, block hierarchy, structural verification rules, streaming machinery |
| [WIRE_FORMAT.md](./WIRE_FORMAT.md) | Bytecode wire format: framing header, section-partitioned body, opcode-stream encoding, operand pool, signature extension, CRC trailer |
| [RUNTIME_FAULTS.md](./RUNTIME_FAULTS.md) | Partial-operation handling: the two-backend contract, virtual-machine trap variants, and the native code-generation contract (per-target guards and default values, deferred to V0.4.0) |
