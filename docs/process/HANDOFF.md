# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `f079c33`
- **Written**: 2026-07-26
- **Tree at write**: clean (all work committed and merged)

**Validity check — run on resume, before trusting this handoff.** On the branch above, compare the
**Parent commit** to `git rev-parse HEAD~1`. Because this handoff file is itself committed, its commit
advances the tip by one, so the state it describes is the parent of the handoff commit. The two match
**only** when this handoff commit is still the branch tip and nothing has landed after it.

- **Match → VALID.** Proceed per the resume prompt below.
- **Mismatch → INVALID and STALE.** A later commit, a no-fast-forward merge, or a rebase moved the tip.
  Do **not** proceed and do **not** trust this handoff. Report the mismatch to the human pilot (recorded
  parent versus actual `HEAD~1`), familiarize from the live channels — `REVERSE_PROMPT.md`,
  `DESIGN_JOURNAL.md`, `TASKLOG.md`, and the git log, always authoritative — and wait for instruction.

## Resume prompt — CONTINUE THE LOOP, BUT THE NEXT STEP IS A SCOUTING GATE THAT MAY STOP

**This is a compaction handoff. The operator pre-authorized the autonomous loop to continue.** After
the validity check passes, do **not** wait for a fresh go-ahead. BUT the same-context nested-equality
frontier is now exhausted of clearly-bounded work, so the next increment begins with a boundedness
re-scout that may itself be a STOP. If the validity check **fails**, do the opposite: do not proceed,
report invalid-and-stale, familiarize, and wait.

Steps, in order:

1. **Validate** — run the validity check above. Valid → continue. Invalid → stop and report.
2. **Familiarize** — read `docs/process/REVERSE_PROMPT.md` (the decision point is spelled out there),
   `docs/process/DESIGN_JOURNAL.md` (newest entry: increment 3), and `docs/process/TASKLOG.md`. Confirm
   the live state matches this handoff, then say what you are about to do.
3. **Re-scout 2-level nesting for boundedness** — `struct O { m: M }` where `M` has a composite field.
   The current flat-record streaming drains one level of sub-fields via a fixed sub-phase; a second
   level needs the drain to RECURSE (a stack, not a fixed phase). Use a Plan agent. Determine whether a
   BOUNDED approach exists with NO new opcode, record, or node kind (reusing op 48/57 nested extract and
   a recursion representation in the existing record stream).
4. **Fork on the verdict:**
   - **BOUNDED** → implement it as the next increment on a feature branch cut from `v0.2.3`, verify
     byte-identity plus the FULL `scripts/release-gate.sh`, no-ff merge, record, restamp this handoff.
   - **NEEDS A DESIGN DECISION or an ISA change** → **STOP**. Write the options into `REVERSE_PROMPT.md`
     and surface them to the operator (recurse-in-the-stream design vs a workstream switch such as
     wiring the self-hosted stages into the shipping binary). Do not force an unsound approach to green,
     and do not switch workstreams without operator direction.
5. **Continue** only if step 4 implemented a bounded increment; otherwise the loop is at an operator
   fork and waits.

**Git position** (as of the Parent commit)
- Branch `v0.2.3` at `f079c33` (the increment-3 merge commit), in sync with origin, working tree clean.
- `main` holds releases and sits behind `v0.2.3` by design. Branch model in
  `docs/process/GIT_STRATEGY.md` (release-branch, no-fast-forward merges up the hierarchy).

**Done this arc**
- Increment 1: tuple-of-struct (merged). Increment 2: enum-in-struct (merged). Increment 3:
  enum-with-struct-payload — implemented, byte-identical, full release-gate green, merged, boundary
  49 → 50 Ok, `EXPECTED_SELF_COMPILE` 69 → 70.
- Construct-support boundary now **50 Ok / 4 Gap / 1 RefRejects**, pinned by
  `self_hosted_construct_support_boundary` in `tests/selfhost_codegen.rs`.

**Remaining Gaps** (the frontier tail)
- `eq/2level_struct__GAP` — the step-3 re-scout target; likely a design-decision stop.
- `eq/struct_arrayofstruct__GAP` — an intentional `struct_eq_kind` defer.
- Plus the deferred out-of-scope tail (floats, generics).

**Guardrails and stops (in force)**
- Correctness signal: the byte-identical differential oracle. Correct iff the self-hosted stage output
  is byte-for-byte identical to the Rust reference **and** the boundary count moves as intended. Never
  weaken an assertion to pass.
- Run the FULL `scripts/release-gate.sh` before claiming complete — a spot check misses op-table-capacity
  and `analyze.kel` scan-loop-bound regressions.
- Rad-hard minimal ISA: no new opcode, and no `BYTECODE_VERSION` bump, without operator authorization —
  a STOP. Confirm before any irreversible or outward-facing action (crates.io publish, tag, force-push).
  Never bypass the pre-push gate.
- Feature-branch intermediate commits may be red; abandoning a non-converging branch and re-cutting is
  acceptable.

**Account usage** — the seven-day rate-limit window is the binding budget under heavy agent work.
Context fill is not the constraint (a 1M window). Pace parallel work by the seven-day figure; the
status line shows `ctx`, `5h`, and `7d`.
