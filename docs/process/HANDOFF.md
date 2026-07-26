# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `a90ae8d`
- **Written**: 2026-07-26
- **Tree at write**: clean (all work committed)

**Validity check — run on resume, before trusting this handoff.** On the branch above, compare the
**Parent commit** to `git rev-parse HEAD~1`. Because this handoff file is itself committed, its commit
advances the tip by one, so the state it describes is the parent of the handoff commit. The two match
**only** when this handoff commit is still the branch tip and nothing has landed after it.

- **Match → VALID.** Trust the resume prompt below, familiarize from the live channels, report status,
  and wait for instruction.
- **Mismatch → INVALID and STALE.** A later commit, a no-fast-forward merge, or a rebase moved the tip.
  Do **not** trust this handoff. Report the mismatch to the human pilot (recorded parent versus actual
  `HEAD~1`), familiarize from the live channels — `REVERSE_PROMPT.md`, `DESIGN_JOURNAL.md`,
  `TASKLOG.md`, and the git log, always authoritative — and wait for instruction.

If the tree could not be committed clean at write time, that is noted here in place of the clean
assertion, with the uncommitted state described.

## Resume prompt

You are continuing the autonomous self-hosted-compiler loop for Keleusma. First run the validity check
above, then familiarize from the three resume channels and the active plan document, and report status.
Wait for operator instruction before acting; the loop's keep-going default applies inside an active
session, not at a cold resume.

**Read first**
- `docs/process/REVERSE_PROMPT.md` — bounded latest state and next step
- `docs/process/DESIGN_JOURNAL.md` — append-only increment reasoning, newest first
- `docs/process/TASKLOG.md` — current sprint state
- `docs/decisions/ENUM_STRUCT_PAYLOAD_PLAN.md` — the next increment's full plan

**Git position** (as of the Parent commit)
- Branch `v0.2.3` at `a90ae8d`, in sync with origin, working tree clean.
- `v0.2.3` is the version-development integration branch; `main` holds releases and sits behind it by
  design. Branch model in `docs/process/GIT_STRATEGY.md` (release-branch, no-fast-forward merges up the
  hierarchy).

**Done this arc**
- Increment 1: tuple-of-struct equality (merged). Increment 2: enum-in-struct equality — implemented,
  byte-identical, full release-gate green, merged, CI green. Boundary moved 48 → 49 Ok.
- Construct-support boundary now **49 Ok / 5 Gap / 1 RefRejects**, pinned by
  `self_hosted_construct_support_boundary` in `tests/selfhost_codegen.rs`.
- Process work: the release-branch git strategy was formalized, and this handoff mechanism, the Compact
  Instructions, and the context/rate-limit status line were added.

**Next task** (loop-selected, bounded, no operator prompt needed to choose)
- **enum-with-struct-payload equality**: `struct P { x: Word }`, `enum E { A(P), B }`, `a == b`.
- Scouted BOUNDED — no new opcode, record, or node kind. The mirror of enum-in-struct on the standalone
  enum-eq path (`push_enum_eq` / `eqfields`, **not** `push_struct_eq_nested` / `seb`). Reuses the
  existing op-57 struct-payload extract and the already-tracked `evfstruct` index.
- Execute the four-commit plan in `docs/decisions/ENUM_STRUCT_PAYLOAD_PLAN.md`. Target: boundary
  49 → 50 Ok; `EXPECTED_SELF_COMPILE` 69 → 70 if the inner loop is factored.

**Method and guardrails**
- Correctness signal: the byte-identical differential oracle. Correct if and only if the self-hosted
  stage output is byte-for-byte identical to the Rust reference **and** the boundary count moves as
  intended. Never weaken an assertion to pass.
- Work on a feature branch cut from `v0.2.3` (`scripts/worktree.sh`). Intermediate commits may be red;
  abandoning a non-converging branch and re-cutting is acceptable.
- Run the FULL `scripts/release-gate.sh` before claiming complete. A spot check misses op-table-capacity
  and `analyze.kel` scan-loop-bound regressions, the recurring tuple/enum failure mode.
- Merge into `v0.2.3` with a no-fast-forward merge commit after rebasing onto the current tip, push, and
  confirm CI green (CI binding, remedy red immediately).
- Rad-hard minimal ISA: no new opcode, and no `BYTECODE_VERSION` bump, without operator authorization —
  a STOP. Confirm before any irreversible or outward-facing action (publish, tag, force-push).
- After each increment update `DESIGN_JOURNAL.md` (append), `REVERSE_PROMPT.md` (overwrite), and
  `TASKLOG.md`. Overwrite this `HANDOFF.md`, restamped with the new commit, before a planned compaction.

**Standing policy**
- Among bounded roadmap tasks, order by context-switching-avoidance first, then priority; do not prompt
  the operator to choose. Surface only genuine design-decision stops.
- Account usage is the binding budget under heavy agent work. The seven-day rate-limit window governs
  how hard the loop can run; the status line shows `ctx`, `5h`, and `7d`. Pace parallel work by it.
