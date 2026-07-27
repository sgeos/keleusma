# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `1022d72`
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

## Resume prompt — SURFACE THE FORK (the bounded same-context frontier is exhausted)

**The nested-composite-equality family is now fully self-hosted (increments 1–5).** There is no
remaining bounded same-context increment, so the loop's keep-going default does NOT apply here: the next
move is a genuine operator-decision fork. After the validity check passes, do **not** autonomously start
a design decision or a workstream switch. If the validity check **fails**, report invalid-and-stale,
familiarize, and wait.

Steps, in order:

1. **Validate** — run the validity check above. Valid → continue. Invalid → stop and report.
2. **Familiarize** — read `docs/process/REVERSE_PROMPT.md` (the fork is spelled out there),
   `docs/process/DESIGN_JOURNAL.md` (newest entries: increments 3–5), and `docs/process/TASKLOG.md`.
   Confirm the live state matches this handoff.
3. **Surface the fork to the operator** — the two remaining boundary Gaps are NOT bounded roadmap
   increments: (a) **third-level struct nesting** needs a GENERAL depth stack in the drain (the
   total-language verifier forbids the recursion that would make arbitrary depth trivial) — a design
   decision; (b) **floats / generics** are out of scope for the self-hosted subset. The highest-leverage
   alternative is a **workstream switch**: wiring the self-hosted stages into the shipping binary
   (Workstream A). Present these options (design the depth stack / switch workstreams / pause) and wait
   for direction. Do not pick among them autonomously — this is the operator's call per the loop's stop
   conditions.

If the operator directs one, then follow the normal increment cycle (feature branch off `v0.2.3`,
byte-identity oracle + FULL `scripts/release-gate.sh`, no-ff merge, push, confirm CI, record on all
three channels, restamp this HANDOFF before the next planned compaction).

**Git position** (as of the Parent commit)
- Branch `v0.2.3` at `1022d72` (the increment-5 merge commit), in sync with origin, working tree clean.
- `main` holds releases and sits behind `v0.2.3` by design. Branch model in
  `docs/process/GIT_STRATEGY.md` (release-branch, no-fast-forward merges up the hierarchy).

**Done this arc**
- Increments 1-5: tuple-of-struct, enum-in-struct, enum-with-struct-payload, 2-level-struct-nesting,
  struct-of-array-of-struct — all implemented, byte-identical, full-gate-green, merged. The
  nested-composite-equality family is fully self-hosted. Boundary now **52 Ok / 2 Gap / 1 RefRejects**,
  pinned by `self_hosted_construct_support_boundary` in `tests/selfhost_codegen.rs`;
  `EXPECTED_SELF_COMPILE` is 72.
- Capacity: the lexer `src.bytes` source buffer was raised 245760 → 393216 (parse.kel outgrew it) and
  the `dl_reject_module_via_kel` layout-verifier test arena to 4 MB. Resizing a shared byte-array buffer
  expands the per-element data layout and can cascade into layout-verifier arena limits — bump together.

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
