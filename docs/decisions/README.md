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
| [IS_STRUCT_DISPOSITION.md](./IS_STRUCT_DISPOSITION.md) | `Op::IsStruct` intent, producers, and disposition: **keep it** — specified, retained, no source-level producer as of 2026-08-24, and still reachable in bytecode |
| [FIXED_SHARED_SLOT_ABI.md](./FIXED_SHARED_SLOT_ABI.md) | Open: the `Fixed` shared-slot ABI gap is the host-visible **scale**, not the representation — `Fixed<16>` and `Fixed<8>` compile to byte-identical layouts |
| [OPERAND_WIDTH_GAP_BRIEF.md](./OPERAND_WIDTH_GAP_BRIEF.md) | Three producers dropped a composite operand's packed width, refusing `bool` and `Fixed` composite members; `Unit` deliberately still does |
| [UNEXERCISED_ARMS_BRIEF.md](./UNEXERCISED_ARMS_BRIEF.md) | The kind-discriminated lowering arms nothing exercises: 40 combinations, the corpus reaches 8, and the corpus never produces a `Byte` or `Bool` composite read at all |
| [REACHABLE_ARMS_BRIEF.md](./REACHABLE_ARMS_BRIEF.md) | Splitting that residue into arms ordinary source can reach, which get witnesses, and kinds the backend refuses, which stay recorded |
| [COVERAGE_RESIDUAL_BRIEF.md](./COVERAGE_RESIDUAL_BRIEF.md) | What the backend's uncovered 2 chunks and 86 opcode instances actually are: one finding, not two, and both refusals deliberate — with the recommendation to stop chasing the last 0.1% |
| [FLOAT_IN_COMPOSITE_BRIEF.md](./FLOAT_IN_COMPOSITE_BRIEF.md) | A `Float` inside a composite body, which needed no ruling because a body field is internal; sized from a probe written first, with the corpus population measured at zero float field reads over 256 construction sites |
| [FLOAT_SHARED_SLOT_BRIEF.md](./FLOAT_SHARED_SLOT_BRIEF.md) | The `Float` shared data slot, the settled half of the operator's Option A float ruling: IEEE-754 bytes at the stated offset, built 2026-08-31, with the outcome and two mis-scopings recorded |
| [STRING_ABI_OPTION_B.md](./STRING_ABI_OPTION_B.md) | Ruled and binding on the `v0.2.3` line, which owns `src/`, received directly there 2026-08-30: string marshalling makes the two embeddings agree; not yet implemented. **Authored by that line; "this line" in the document means theirs.** |
