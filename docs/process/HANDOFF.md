# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `8df4b08`
- **Written**: 2026-07-27
- **Tree at write**: clean (all work committed and merged)
- **Context**: written immediately before a PLANNED COMPACTION. The operator will SELECT A DIRECTION on
  resume, so the resume prompt below is deliberately present-the-fork-and-wait, not continue-the-loop.

**Validity check — run on resume, before trusting this handoff.** On the branch above, compare the
**Parent commit** to `git rev-parse HEAD~1`. Because this handoff file is itself committed, its commit
advances the tip by one, so the state it describes is the parent of the handoff commit. The two match
**only** when this handoff commit is still the branch tip and nothing has landed after it.

- **Match → VALID.** Proceed per the resume prompt below.
- **Mismatch → INVALID and STALE.** A later commit, a no-fast-forward merge, or a rebase moved the tip.
  Do **not** proceed and do **not** trust this handoff. Report the mismatch to the human pilot (recorded
  parent versus actual `HEAD~1`), familiarize from the live channels — `REVERSE_PROMPT.md`,
  `DESIGN_JOURNAL.md`, `TASKLOG.md`, and the git log, always authoritative — and wait for instruction.

## Resume prompt — SURFACE THE FORK (bounded same-context work is exhausted)

**Two workstreams have now reached their bounded end: the nested-composite-equality family is fully
self-hosted (increments 1–5, boundary 52 Ok), and the CLI self-hosted-backend residual is delivered
(the `--compiler <rust|self-hosted>` flag).** There is no remaining bounded same-context task, so the
loop's keep-going default does NOT apply: the next move is a genuine operator-decision fork. After the
validity check passes, do **not** autonomously start a design decision or a new workstream. If the
validity check **fails**, report invalid-and-stale, familiarize, and wait.

Steps, in order:

1. **Validate** — run the validity check above. Valid → continue. Invalid → stop and report.
2. **Familiarize** — read `docs/process/REVERSE_PROMPT.md` (the fork options are spelled out there),
   `docs/process/DESIGN_JOURNAL.md` (newest entries: increments 3–5 and the CLI-backend workstream), and
   `docs/process/TASKLOG.md`. Confirm the live state matches this handoff.
3. **Surface the fork to the operator** — present the candidate directions and wait: (a) a NEW self-host
   language-surface area to self-host (grows the boundary again; needs operator selection of which
   construct family); (b) **third-level struct nesting** — generalize the fixed-depth drain to a bounded
   depth stack (a design effort, rated extreme); (c) **harden the new CLI backend** (thread the CLI
   preamble through self-hosted mode; widen the supported-subset error detail); (d) a different
   workstream (release cadence, other roadmap). Do not pick among them autonomously.

If the operator directs one, follow the normal increment cycle (feature branch off `v0.2.3`,
byte-identity oracle where applicable + FULL `scripts/release-gate.sh`, no-ff merge, push, confirm CI,
record on all three channels, restamp this HANDOFF before the next planned compaction).

**Git position** (as of the Parent commit)
- Branch `v0.2.3` at `8df4b08` (the CLI-self-hosted-backend merge `3a98b83` plus its handoff restamp),
  in sync with origin, working tree clean, CI green.
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
- CLI self-hosted backend (Workstream A): `keleusma-cli compile --compiler <rust|self-hosted>` (default
  rust). The self-host driver moved (history-preserving) to `keleusma/src/selfhost/mod.rs` behind a
  `self-host` feature; all ten Rust-read `.kel` relocated to `keleusma/src/selfhost/kel/` (`include_str!`);
  `compiler/` is now a thin `pub use keleusma::selfhost::*` re-export (still detached). Entry
  `keleusma::selfhost::self_hosted_compile` (host-only, `catch_unwind` → `Unsupported`). Full gate GREEN.
  Read-path caveat: after such a cross-workspace move, sweep EVERY read helper in BOTH workspaces, and
  format `compiler/` separately (`cargo fmt --all` does not reach the detached workspace).

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
