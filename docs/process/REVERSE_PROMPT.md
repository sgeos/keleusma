# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI-to-human handoff, overwritten for the latest session.

# CURRENT STATE

**2026-09-29. V0.2.X Order-1 capacity increment.**

Work is on feature branch `feat/v02-order1-next`. The V0.3.X Codex line was left untouched.
The V0.2.X roadmap is being followed in priority order, starting with Order 1.

## Handoff validity

The V0.2.X handoff ancestry check passed, but `git rev-list --count e84d7cfa..HEAD` measured nine
commits, so its content claims were re-derived rather than carried forward. The required full checks
were incomplete. The full workspace `cargo nextest run --workspace` was interrupted after 2,009
passed; four in-flight tests received SIGINT and 920 did not run. The local no-fetch `cargo audit`
exited successfully using a copied advisory database dated 2026-09-28, but it did not refresh the
database. **Classify the handoff as invalid-and-stale.** The fingerprint, twelve-source inventory,
self-host source diff, shellcheck, and RTOS test checks were separately re-derived during resume.

## Current V0.2.X work

The operator selected a 640-row cap for six type-check input channels. `verify_types.kel` and its
host test seeder now agree on the expanded tables and slot offsets. The bounded fold's maximum-step
expression includes the expanded call-site, occurrence, and expression phases. The capacity price
is 7,168 words, or 56 KiB, across the parallel tables.

The real-source differential now fits and accepts all twelve stage sources. The focused test file
passed with 72 tests under `--features self-host`. This only closes the capacity gap. Rust still
walks the reference parser's abstract syntax tree to extract the facts passed to the Keleusma stage,
so the Order-1 gate remains open.

## Next work

Continue with the V0.2.X roadmap in priority order. Keep the next increment on Order 1 and use
`docs/decisions/TYPECHECK_IMPLEMENTATION_PLAN.md` and
`docs/decisions/WIRE_FORMAT_SELFHOST_PLAN.md` to identify the next integration step. Do not move to
Order 2 while Order 1 remains unmet. Do not switch to or merge from the concurrent V0.3.X line.

Before relying on a green handoff, rerun the complete workspace nextest suite and refresh the cargo
audit advisory database when network access is available. The full nextest result remains unverified
in this session.
