# Decisions

> **Navigation**: [Documentation Root](../README.md)

Architectural and design decisions for Keleusma.

Decisions follow a three-file lifecycle. New questions start in PRIORITY or BACKLOG. When resolved, they move to RESOLVED with rationale. Decisions are numbered sequentially within each file.

## Contents

| Document | Description |
|----------|-------------|
| [RESOLVED.md](./RESOLVED.md) | Completed decisions with rationale |
| [PRIORITY.md](./PRIORITY.md) | Blocking decisions awaiting resolution |
| [BACKLOG.md](./BACKLOG.md) | Deferred decisions for future consideration |
| [COMPOSITE_REGION_EVIDENCE.md](./COMPOSITE_REGION_EVIDENCE.md) | What the V0.2.3 runtime establishes for the composite-region-reuse proof, with provenance per claim |
| [YIELD_OWNERSHIP_MODE.md](./YIELD_OWNERSHIP_MODE.md) | Accepted in principle: `ref`/`out` on a yielding declaration's return signature, choosing machine-owned or host-owned storage for the yielded value (V0.3.0 or later) |
| [STRING_ABI_OPTION_B.md](./STRING_ABI_OPTION_B.md) | Ruled and binding on this line, received directly 2026-08-30: string marshalling makes the two embeddings agree; not yet implemented |
| [TEXT_CAPACITY_TYPE.md](./TEXT_CAPACITY_TYPE.md) | Authorized and designed 2026-08-31: static text is a `.rodata` pointer, dynamic text is the capacity-carrying `Text<N>`; not yet implemented |
| [FLOAT_FORMAT_LADDER.md](./FLOAT_FORMAT_LADDER.md) | Companion to the V0.3.X line's `FLOAT_LADDER.md`, which holds the ruling: what the ladder demands of this runtime, why widen-compute-narrow is forced rather than preferred, and two refinements to the arithmetic-width precondition |
