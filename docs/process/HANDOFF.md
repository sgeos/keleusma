# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `dfef852`
- **Written**: 2026-07-26
- **Tree at write**: clean (all work committed)

**Validity check — run on resume, before trusting this handoff.** On the branch above, compare the
**Parent commit** to `git rev-parse HEAD~1`. Because this handoff file is itself committed, its commit
advances the tip by one, so the state it describes is the parent of the handoff commit. The two match
**only** when this handoff commit is still the branch tip and nothing has landed after it.

- **Match → VALID.** Proceed per the resume prompt below.
- **Mismatch → INVALID and STALE.** A later commit, a no-fast-forward merge, or a rebase moved the tip.
  Do **not** proceed and do **not** trust this handoff. Report the mismatch to the human pilot (recorded
  parent versus actual `HEAD~1`), familiarize from the live channels — `REVERSE_PROMPT.md`,
  `DESIGN_JOURNAL.md`, `TASKLOG.md`, and the git log, always authoritative — and wait for instruction.

## Resume prompt — CONTINUE THE LOOP

**This is a compaction handoff. The operator compacted mid-session and has pre-authorized the
autonomous loop to continue.** After the validity check passes, do **not** wait for a fresh go-ahead —
this handoff is the go-ahead. Proceed with the next increment. If the validity check **fails**, do the
opposite: do not proceed, report invalid-and-stale, familiarize, and wait.

You are continuing the autonomous self-hosted-compiler loop for Keleusma. Steps, in order:

1. **Validate** — run the validity check above. Valid → continue. Invalid → stop and report.
2. **Familiarize** — read `docs/process/REVERSE_PROMPT.md`, `docs/process/DESIGN_JOURNAL.md`,
   `docs/process/TASKLOG.md`, and the plan `docs/decisions/ENUM_STRUCT_PAYLOAD_PLAN.md`. Briefly
   confirm the live state matches this handoff, then say what you are about to do.
3. **Implement the next increment** — enum-with-struct-payload equality, per the plan. Work on a
   feature branch cut from `v0.2.3` (`scripts/worktree.sh`).
4. **Verify** — the byte-identical differential oracle green (the new test plus the nested-equality
   blast-radius suite and the boundary), then the FULL `scripts/release-gate.sh`.
5. **Merge** — rebase the branch onto the current `v0.2.3` tip, merge with a no-fast-forward merge
   commit, push, and confirm CI green.
6. **Record** — update `DESIGN_JOURNAL.md` (append), `REVERSE_PROMPT.md` (overwrite), `TASKLOG.md`;
   and before the next planned compaction, overwrite this `HANDOFF.md` restamped with the new commit.
7. **Continue** — pick the next gap by the task-ordering policy (context-switching-avoidance first,
   then priority; no operator prompt for a choice among bounded roadmap tasks), or surface a genuine
   design-decision stop.

**Git position** (as of the Parent commit)
- Branch `v0.2.3` at `dfef852`, in sync with origin, working tree clean.
- `v0.2.3` is the version-development integration branch; `main` holds releases and sits behind it by
  design. Branch model in `docs/process/GIT_STRATEGY.md` (release-branch, no-fast-forward merges up the
  hierarchy).

**Done this arc**
- Increment 1: tuple-of-struct equality (merged). Increment 2: enum-in-struct equality — implemented,
  byte-identical, full release-gate green, merged, CI green. Boundary moved 48 → 49 Ok.
- Construct-support boundary now **49 Ok / 5 Gap / 1 RefRejects**, pinned by
  `self_hosted_construct_support_boundary` in `tests/selfhost_codegen.rs`.
- Process work: the release-branch git strategy was formalized; this handoff mechanism, the Compact
  Instructions, and the context/rate-limit status line were added.

**Next task detail** (loop-selected, bounded)
- **enum-with-struct-payload equality**: `struct P { x: Word }`, `enum E { A(P), B }`, `a == b`. Flips
  `eq/enum_struct_payload__GAP` to Ok.
- Scouted BOUNDED — no new opcode, record, or node kind. The mirror of enum-in-struct on the standalone
  enum-eq path (`push_enum_eq` / `eqfields`, **not** `push_struct_eq_nested` / `seb`). Reuses the
  existing op-57 struct-payload extract and the already-tracked `evfstruct` index. The four-commit plan,
  the reference op sequence, the eager-interning composition, and the capacity gotchas are all in
  `docs/decisions/ENUM_STRUCT_PAYLOAD_PLAN.md`. Target: boundary 49 → 50 Ok; `EXPECTED_SELF_COMPILE`
  69 → 70 if the inner loop is factored.

**Guardrails and stops (in force)**
- Correctness signal: the byte-identical differential oracle. Correct if and only if the self-hosted
  stage output is byte-for-byte identical to the Rust reference **and** the boundary count moves as
  intended. Never weaken an assertion to pass.
- Run the FULL `scripts/release-gate.sh` before claiming complete. A spot check misses
  op-table-capacity and `analyze.kel` scan-loop-bound regressions, the recurring tuple/enum failure
  mode.
- Rad-hard minimal ISA: no new opcode, and no `BYTECODE_VERSION` bump, without operator authorization —
  a STOP. Confirm before any irreversible or outward-facing action (crates.io publish, tag,
  force-push). Never bypass the pre-push gate.
- Feature-branch intermediate commits may be red; abandoning a non-converging branch and re-cutting is
  acceptable, preferable to forcing an unsound approach to green.
- Surface a stop when the oracle diverges and two or three bounded attempts do not resolve it, or when
  no remaining candidate is a bounded roadmap task.

**Account usage** — the seven-day rate-limit window is the binding budget under heavy agent work (about
86% used when this was written, resetting 2026-07-29 ~14:00 PDT). Context fill is not the constraint (a
1M window). Pace parallel work and background agents by the seven-day figure; the status line shows
`ctx`, `5h`, and `7d`.
