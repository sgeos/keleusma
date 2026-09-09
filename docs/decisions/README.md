# Decisions

> **Navigation**: [Documentation Root](../README.md)

Architectural and design decisions for Keleusma.

Decisions follow a three-file lifecycle. New questions start in PRIORITY or BACKLOG. When resolved, they move to RESOLVED with rationale. Decisions are numbered sequentially within each file.

## What a `*_BRIEF.md` is, and what its absence of a marker does NOT mean

This directory also holds many `*_BRIEF.md` files, most paired with a
`*_COMPLETION_CONDITION.txt`. They are **dated working documents written for a single
increment**, not live plans. A brief captures the rationale, the prior failures, and the
specific wrong turns to avoid for the work it was written for; its companion states the
observable end state that increment was judged against.

**Do not resume a brief's goals without checking the tree first.** Several briefs describe
work that is now complete, and **most carry no marker saying so**, because marking them was
never the convention — only a handful say `landed` or similar. **The absence of a status
marker therefore says nothing about whether the work is done.** The tree is the authority;
the brief records how a past increment was reasoned about.

Only the substantive decision and evidence documents are indexed below. The briefs are not,
deliberately: an index of every increment's working notes would need maintaining on every
increment, and a table that drifts is worse than no table.

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
| [FLOAT_ARITH_WIDTH_BRIEF.md](./FLOAT_ARITH_WIDTH_BRIEF.md) | Why float arithmetic must track the module's declared width, the ten narrowing sites, and the mutation result that measured their coverage |
| [FLOAT_FORMAT_LADDER.md](./FLOAT_FORMAT_LADDER.md) | Companion to the V0.3.X line's `FLOAT_LADDER.md`, which holds the ruling: what the ladder demands of this runtime, why widen-compute-narrow is forced rather than preferred, and two refinements to the arithmetic-width precondition |
| [INVALID_BYTECODE_CENSUS.md](./INVALID_BYTECODE_CENSUS.md) | Every site where the runtime raises `InvalidBytecode`, with a verdict per group; no site is claimed unreachable |
| [FEATURE_COMBINATION_SWEEP.md](./FEATURE_COMBINATION_SWEEP.md) | Which feature and width configurations actually build, and which are built by nothing |
| [NARROW_WIDTH_FAILURE_CLASSIFICATION.md](./NARROW_WIDTH_FAILURE_CLASSIFICATION.md) | A verdict for every `narrow-word-16` failure, separating the suite's wide-host assumptions from the one runtime defect among them |
| [FLAT_FIELD_WIDTH_AUDIT.md](./FLAT_FIELD_WIDTH_AUDIT.md) | Whether the repaired opaque-width sites were the only ones, by a stated method validated against the pre-repair tree |
| [DISCARD_ARM_REACHABILITY_BRIEF.md](./DISCARD_ARM_REACHABILITY_BRIEF.md) | Which self-hosted-stage discard arms are reached by a driven program, and what each unreached one is waiting on |
