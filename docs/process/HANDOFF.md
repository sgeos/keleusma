# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `4396719`
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

## Resume prompt — CONTINUE THE LOOP

**The operator directed the loop to keep going on bounded work and order by the criteria (context-first,
then priority), surfacing only at a genuine fork.** After the validity check passes, do **not** wait for
a fresh go-ahead — proceed with the next increment. If the validity check **fails**, do the opposite:
do not proceed, report invalid-and-stale, familiarize, and wait.

Steps, in order:

1. **Validate** — run the validity check above. Valid → continue. Invalid → stop and report.
2. **Familiarize** — read `docs/process/REVERSE_PROMPT.md` (the next candidate is spelled out there),
   `docs/process/DESIGN_JOURNAL.md` (newest entries: increments 3 and 4), and `docs/process/TASKLOG.md`.
   Confirm the live state matches this handoff, then say what you are about to do.
3. **Next increment: struct-of-array-of-struct equality** — `eq/struct_arrayofstruct__GAP`
   (`struct Q { ps: [P; 2] }`, a struct field that is an array-of-struct). Already SCOUTED to a complete
   four-stage blueprint at [`docs/decisions/STRUCT_ARRAYOFSTRUCT_PLAN.md`](../decisions/STRUCT_ARRAYOFSTRUCT_PLAN.md);
   confirmed BOUNDED (no new opcode/record/node kind — the element-struct index is already tracked via
   `sd_farraylen` + `sd_fstruct`, and `push_array_of_struct_eq` is the exact per-element template).
   Implement DIRECTLY from that document — no re-scout needed. Sharp edge: emit the per-element loop
   INLINE (not factored) so eager interning keeps the element indices before false/true. Work on a
   feature branch cut from `v0.2.3`.
4. **Verify** — the byte-identical differential oracle (the new construct test, all five whole-stage
   self-compiles, the nested-eq blast-radius suite, `validate_module_via_kel`, the boundary, the codegen
   count) then the FULL `scripts/release-gate.sh`.
5. **Merge** — rebase onto the current `v0.2.3` tip if it advanced, no-fast-forward merge, push, confirm
   CI green.
6. **Record** — `DESIGN_JOURNAL.md` (append), `REVERSE_PROMPT.md` (overwrite), `TASKLOG.md`; restamp
   this `HANDOFF.md` with the new commit before the next planned compaction.
7. **Continue or stop** — if struct-of-array-of-struct needs a new record/node kind or a general depth
   stack, STOP and surface. Otherwise, after it lands, the same-context frontier is the deferred tail
   (a third struct level → likely a general-depth-stack design decision; floats/generics → out of
   scope). At that point re-weigh a workstream switch (e.g. wiring the self-hosted stages into the
   shipping binary) and surface the choice.

**Git position** (as of the Parent commit)
- Branch `v0.2.3` at `9cfbe8a` (the increment-4 merge commit), in sync with origin, working tree clean.
- `main` holds releases and sits behind `v0.2.3` by design. Branch model in
  `docs/process/GIT_STRATEGY.md` (release-branch, no-fast-forward merges up the hierarchy).

**Done this arc**
- Increments 1-4: tuple-of-struct, enum-in-struct, enum-with-struct-payload, 2-level-struct-nesting —
  all implemented, byte-identical, full-gate-green, merged. Boundary now **51 Ok / 3 Gap / 1 RefRejects**,
  pinned by `self_hosted_construct_support_boundary` in `tests/selfhost_codegen.rs`;
  `EXPECTED_SELF_COMPILE` is 71.

**Key durable finding** (governs every remaining depth increment)
- The total-language verifier FORBIDS recursion (R4, acyclic call graph); no `.kel` stage function may
  self-recurse. So each additional composite-nesting DEPTH is an explicit extra phase/stack in the
  drain, not a copy-recurse — this is the real cost, not the ISA (which is untouched). Byte-identity
  also hinges on the monotonic slot-order: extract temps allocate depth-first, r2 before l2, +2 per
  level, matching the reference's `next_slot` (never rewound by `end_scope`).

**Guardrails and stops (in force)**
- Correctness signal: the byte-identical differential oracle. Correct iff the self-hosted stage output
  is byte-for-byte identical to the Rust reference **and** the boundary count moves as intended. Never
  weaken an assertion to pass.
- Run the FULL `scripts/release-gate.sh` before claiming complete. When running gates in parallel with
  other cargo work on the SAME worktree, they contend on the cargo build lock — run ONE gate per
  worktree.
- Rad-hard minimal ISA: no new opcode, and no `BYTECODE_VERSION` bump, without operator authorization —
  a STOP. Confirm before any irreversible or outward-facing action (crates.io publish, tag, force-push).
  Never bypass the pre-push gate.
- Feature-branch intermediate commits may be red; abandoning a non-converging branch and re-cutting is
  acceptable.

**Account usage** — the seven-day rate-limit window is the binding budget under heavy agent work.
Context fill is not the constraint (a 1M window). Pace parallel work by the seven-day figure; the
status line shows `ctx`, `5h`, and `7d`.
