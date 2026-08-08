# Process Strategy

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

This document describes the development process for Keleusma, adapted for library engineering with agentic AI-driven development.

## Engineering Classification

This project is classified as **Library Engineering**.

Libraries occupy a middle ground between FMCG (Fast-Moving Consumer Goods) and high-assurance engineering. Correctness matters more than in a game, because users depend on the library for their own projects. However, the cost of a bug is lower than in high-assurance systems. This classification informs the level of rigor applied to testing, documentation, and code review throughout the project.

| Dimension | Library | FMCG | Mission-Critical |
|-----------|---------|------|-------------------|
| Ship criteria | Tests pass, API stable | Tests pass, playable | Formal verification |
| Testing | Unit + integration + property | Unit + integration | Unit + integration + property + fuzz + formal |
| Failure cost | User projects break | Player experience degraded | Safety or financial loss |
| Iteration speed | Moderate | Fast | Slow |
| Documentation | API docs + guides required | Internal docs sufficient | Exhaustive specification |
| Code review | Thorough review of public API | Rapid review | Multi-reviewer with sign-off |

### Higher Rigor Subsystems

The compiler and VM require careful correctness. Bytecode execution must not panic or produce undefined behavior. The lexer and parser should produce clear, actionable error messages. These subsystems receive additional scrutiny during development and review, including edge case testing and defensive validation at module boundaries.

## Agentic AI Development Loop

The AI agent operates within a structured loop that balances autonomy with human oversight.

```
1. Identify blockers
       |
       v
2. Research (read docs, explore code)
       |
       v
3. Clear blocker (ask human or resolve independently)
       |
       v
4. Advance development (implement, test, refactor)
       |
       v
5. Update process files (TASKLOG.md, REVERSE_PROMPT.md)
       |
       v
6. Commit
       |
       v
   (return to step 1)
```

### Autonomy Boundaries

The AI agent **may proceed** autonomously with:

- Adding dependencies to Cargo.toml
- Making design decisions within the documented specification
- Creating new files and modules
- Resolving technical blockers through research and implementation

The AI agent **should stop** and consult the human pilot when:

- A decision would change the language semantics
- A technical approach has significant tradeoffs requiring human judgment
- The token limit is approaching and work is incomplete
- An assumption is unclear and cannot be resolved from existing documentation

## Tiered Verification

The full gate must be green before every **merge**. It does not need to run after every
**change**. Those are different questions, and conflating them costs hours: roughly twenty
full gates were run across one session, one per increment, where four would have given an
identical answer.

| Tier | When | Cost | Command |
|---|---|---|---|
| **0 — inner loop** | every edit | seconds | `scripts/fast-check.sh 'test(<filter>)'` |
| **1 — pre-commit** | every increment | ~3 min | the three checks below |
| **2 — pre-merge** | once per merge, batching three or four increments | ~2 h | `scripts/release-gate.sh` |

**Tier 1 is the one that is easy to skip and should not be.** These three catch defects that
targeted tests are structurally incapable of seeing, and each has drawn blood:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p keleusma --no-default-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

A broken intra-doc link is invisible to `test` and `clippy`; that is how V0.2.1 shipped with a
red CI Doc job, and how broken `src/selfhost/` links survived four releases. A test importing
behind a feature gate breaks the runtime-only build while every targeted test passes.

**Batch increments per merge.** A feature branch may sit red across many commits; only its tip
must be green. Running the full gate per increment buys nothing, because the gate answers a
question about the merge, not about the increment.

### The full gate re-runs the heaviest suite per feature configuration

`cargo test` runs once per feature config, so `selfhost_codegen` — whole-stage self-compiles,
the most expensive suite by far — executes four times. That is most of the two hours.

This is **deliberate and has not been narrowed.** Skipping it under `signatures` or
`signatures,shell` would probably be safe, since those affect signing and the docs.rs surface
rather than codegen. "Probably safe" is how the two coverage holes above were made, and the
saving is roughly two-fold where batching gives five-fold at no cost to coverage. Take the
batching. Narrowing the matrix is an operator decision, not the loop's.

### A green suite cannot see a performance regression

On 2026-08-08 the wire-format v2 cutover was merged only after this was learned the hard way:
the port was functionally perfect and roughly **forty times slower**, and every tier reported
green. One stage self-compile went from 54 seconds to over 37 minutes. Nothing in the gate
measures time, so nothing failed.

`tests/perf_canary.rs` is the answer — a tripwire, not a benchmark, with a deliberately loose
ceiling. It runs in every tier that runs the test suite and costs about two seconds.

**If it fails, profile before touching the ceiling.** The defect class it guards is a hot-path
read that has become proportional to the whole module: a rebuilt view, a re-parsed table, or a
whole-pool decode behind what should be a single-record fetch. Correctness tests will keep
saying the answers are right, because they are.

The canary was validated against the real regression rather than assumed to work: reverting the
repair takes it from 1.7 s to 67.3 s, tripping the ceiling. A performance guard that has not
been shown able to fail is not a guard.

### Reap orphans before timing anything

An interrupted gate leaves its test binary reparented to PID 1, still at full CPU. One was found
burning four cores for ten hours and halving the machine. They accumulate, one per interrupted
run, and they corrupt exactly the timing signal the canary depends on. `release-gate.sh` now
reaps them as a preflight; do the same by hand before any measurement.

## Milestone-Based Development

Development follows milestone sprints. Each milestone represents a coherent unit of work with defined entry criteria, exit criteria, and success criteria. See [COMMUNICATION.md](./COMMUNICATION.md) for the bidirectional communication protocol and work item coding system.

## Related Documents

- [COMMUNICATION.md](./COMMUNICATION.md) for the bidirectional human-AI communication protocol
- [GIT_STRATEGY.md](./GIT_STRATEGY.md) for version control conventions
