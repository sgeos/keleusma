# Process Strategy

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

This document describes the development process for Keleusma, adapted for library engineering with agentic AI-driven development.

## Engineering Classification

This project is classified as **Library Engineering**.

Libraries occupy a middle ground between FMCG (Fast-Moving Consumer Goods) and mission-critical engineering. Correctness matters more than in a game, because users depend on the library for their own projects. However, the cost of a bug is lower than in safety-critical systems. This classification informs the level of rigor applied to testing, documentation, and code review throughout the project.

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

## Milestone-Based Development

Development follows milestone sprints. Each milestone represents a coherent unit of work with defined entry criteria, exit criteria, and success criteria. See [COMMUNICATION.md](./COMMUNICATION.md) for the bidirectional communication protocol and work item coding system.

## Related Documents

- [COMMUNICATION.md](./COMMUNICATION.md) for the bidirectional human-AI communication protocol
- [GIT_STRATEGY.md](./GIT_STRATEGY.md) for version control conventions
