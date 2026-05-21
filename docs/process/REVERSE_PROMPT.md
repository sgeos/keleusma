# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: Pre-merge documentation-sync pass landed on the `V0.2.0-isa` branch. Reference, architecture, and guide docs are now aligned with the ISA that actually shipped (69 opcodes, 64-byte wire-format header, type-checker-stage closure rejection, no f-string surface, no bundled text-composition natives, no `text` cargo feature). The V0.2.0 ISA cleanup follow-on from the prior round (gitignore, R41, soft-warning helper, narrow-bytecode CheckedXxx flag/high half) remains landed. The branch is ready for merge to `main` and for the V0.2.0 publication step.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Documentation-sync pass before merge. | Reference and architecture docs realigned with the V0.2.0 ISA: 69 opcodes (was 65/74 in stale places), 64-byte wire-format header (was 16/24/32/V0.3.0-mislabel), inline `(u16, u8)` operand shape (was incorrectly described as pool-referencing), inline-vs-pool split now 65/4 (was 58/7 with wrong totals), per-instruction WCET cost column aligned with `nominal_op_cycles` across ~26 rows, stack growth/shrink tables rebuilt from `Op::stack_growth` / `Op::stack_shrink`. `INSTRUCTION_SET.md` Cost Summary regenerated. `EXECUTION_MODEL.md` mislabelled wire format as V0.3.0 (now V0.2.0) and carried a stale framing-header byte-offset table (now matches `WIRE_FORMAT.md`). Guide and design docs realigned with the V0.2.0 surface: `README.md`, `GRAMMAR.md`, `LANGUAGE_DESIGN.md`, `STANDARD_LIBRARY.md`, `EMBEDDING.md`, `COOKBOOK.md`, `WHY_REJECTED.md`, and `BACKLOG.md` B3/B5b/B6 entries lost references to the retired f-string surface, `text` cargo feature (gone), `stddsl::Text` bundle (gone), bundled `to_string`/`concat`/`slice`/`length` natives (gone), `closure-hoisting` pipeline stage (gone), and `Op::CallIndirect` / `Op::PushFunc` / `Op::MakeClosure` / `Op::MakeRecursiveClosure` opcodes (gone). Closure rejection is now consistently described as a type-checker-stage diagnostic rather than a load-time verifier rejection. The `Text` keyword replaces stray `String` usages in surface examples. The cleanup follow-on from the prior round (gitignore `*.kel.bin`, R41 rejecting five-opcode string builder, soft-warning helper, narrow-bytecode CheckedXxx flag/high half) remains landed unchanged. |

## Verification matrix

```bash
cargo test --workspace                                                          # 797 lib + 53 rogue-script + 17 marshall + integration suites, all green (from prior round; no source changed this round)
cargo clippy --tests --workspace --all-features -- -D warnings                  # clean (prior round)
```

The documentation-sync pass in this round touched only `*.md` files. No source was modified, so the tests, clippy, and example builds from the prior round remain valid.

## Open concerns

None.

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed as not-applicable (closures retired in Phase 4) |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | Closed (Phases 1, 2, 3, 3.5, Consolidation B, 4, 5, 6, 7a, 7b, 7c, 8 complete) |

## Intended Next Step

V0.2.0-isa branch is ready for merge to `main`. The natural next step is one of:

- Merge the `V0.2.0-isa` branch into `main` and tag the release.
- Manual `cargo publish` of the V0.2.0 crate (the publication step is operator-owned; the agent does not run `cargo publish`).
- A B15 follow-on: remove `Type::Unknown` entirely now that the V0.2.0 ISA work is closed.
- Operator selection of a different directive.
