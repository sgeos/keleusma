# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `67539e7`
- **Written**: 2026-07-30
- **Tree at write**: clean (all work committed and merged)
- **Context**: written after merging the tuple-of-deep-struct increment (`67539e7`), the first payoff of
  the 3-level frame-stack machinery. The operator's active workstream is DEEPER NESTING FOR OTHER
  COMPOSITES; the smallest increment (tuple-of-deep-struct) is done. This is a natural stopping point
  given the seven-day budget (THREE CI-green increments merged this session). Resume the next increment
  (tuple-in-tuple) from `REVERSE_PROMPT.md`, or the operator may redirect.

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

**This session merged THREE CI-green increments: the CLI-backend error-surface hardening, arbitrary-depth
nested struct-in-struct equality (52 -> 53 Ok), and tuple-of-deep-struct (54 Ok).** The operator's ACTIVE
workstream is DEEPER NESTING FOR OTHER COMPOSITES; the smallest increment within it (tuple-of-deep-struct)
is done. The NEXT bounded increment is **tuple-in-tuple** (see `REVERSE_PROMPT.md` for the ordered
remaining gaps). This is a natural stopping point given the seven-day budget; on resume, either continue
the workstream with tuple-in-tuple or let the operator redirect. If the validity check **fails**, report
invalid-and-stale, familiarize, and wait.

Steps, in order:

1. **Validate** — run the validity check above. Valid → continue. Invalid → stop and report.
2. **Familiarize** — read `docs/process/REVERSE_PROMPT.md` (the fork options are spelled out there),
   `docs/process/DESIGN_JOURNAL.md` (newest entry: the 3-level nested-struct increment), and
   `docs/process/TASKLOG.md`. Confirm the live state matches this handoff.
3. **Surface the fork to the operator** — present the candidate directions and wait: (a) a NEW self-host
   language-surface area to self-host (grows the boundary again; needs operator selection of which
   construct family); (b) **native-call support in the self-hosted pipeline** — add a native-call path to
   the self-hosted codegen (larger; the increment that would make threading the CLI preamble meaningful, a
   hard boundary otherwise); (c) **deeper nesting for the OTHER composites** — the arbitrary-depth
   generalization landed only for struct-in-struct; nested tuple/array/enum still cap at their existing
   depths, and the `es_*`/`se_stk_*`/`se_nstk_*` stack machinery is now in place to extend them similarly;
   (d) a different workstream (release cadence, other roadmap). Do not pick among them autonomously. NOTE:
   "harden the CLI backend" and "third-level struct nesting" were prior fork selections and are DONE — do
   not re-offer them.

If the operator directs one, follow the normal increment cycle (feature branch off `v0.2.3`,
byte-identity oracle where applicable + FULL `scripts/release-gate.sh`, no-ff merge, push, confirm CI,
record on all three channels, restamp this HANDOFF before the next planned compaction).

**Git position** (as of the Parent commit)
- Branch `v0.2.3` at `67539e7` (the tuple-of-deep-struct merge; feature branch
  `feat/selfhost-tuple-deep-struct` merged no-ff), plus this handoff restamp on top. In sync with origin,
  working tree clean, local full gate GREEN, CI binding after push.
- `main` holds releases and sits behind `v0.2.3` by design. Branch model in
  `docs/process/GIT_STRATEGY.md` (release-branch, no-fast-forward merges up the hierarchy).

**Done this arc**
- Tuple-of-deep-struct equality (`67539e7`): a tuple whose struct element nests arbitrarily deep is now
  admitted — the tuple container's struct-element sub-fields already drained through the arbitrary-depth
  frame stack, so only the admission `tuple_eq_kind` needed widening to `struct_subtree_pure`. NO new code
  path; boundary +1 (`eq/tuple_of_deep_struct`). Demonstrates the reusability the 3-level generalization
  unlocked: extending depth to a new composite container is an admission edit, not a stage rewrite.
- Arbitrary-depth nested struct-in-struct equality (`5c93920`): the fixed depth-2 nested-equality special
  case was generalized to a bounded depth stack across FOUR stages — parse.kel (`se_stk_*` + `se_pop_cascade`),
  reconstruct.kel (`se_nstk_*` + `se_nsub_pop`, recursive `seb` grammar), codegen.kel (`push_struct_eq_subfields`
  as an explicit-stack reverse-DFS emitter + `struct_forest_end`/`nested_end`/`es_compute_sfoff`), and the
  ADMISSION scan `struct_eq_kind` (`struct_subtree_pure`). `eq/3level_struct` byte-identical; boundary
  **52 → 53 Ok**; `EXPECTED_SELF_COMPILE` 72 → 75. LESSON: a depth assumption hid in the ADMISSION/dispatch
  (a depth-3 type fell back to a primitive `==`), caught only by the byte-identical differential oracle — not
  by self-compile or verify. No opcode/record/node/`BYTECODE_VERSION` change.
- CLI-backend error hardening (`cf24f12`): `SelfHostError::ReferenceRejected` (a genuine source error the
  reference also rejects) split from `Unsupported` (a self-hosted-subset limitation);
  `rust_backend_would_help()` gates the `retry with --compiler rust` hint so a plain compile error reports
  without it; `describe_divergence` names the first diverging chunk and dimension. Threading the CLI
  preamble was NOT attempted — a hard boundary, not an oversight: the self-hosted codegen emits no
  native-call opcode (wire tags 1..=63 have `Op::Call` but no `CallExternalNative`/`CallVerifiedNative`).
  No ISA/`.kel` change; three new backend tests; full gate green.
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

**Observed pre-existing warning** (not introduced this arc, not fixed — out of scope)
- `src/vm.rs:8 use alloc::vec;` is flagged `unused_imports` in the `--no-default-features` `cargo test`
  build only. The full gate stays GREEN because that step does not deny warnings, and the clippy
  `-D warnings` step runs under a feature set where the import is used. A correct fix needs the right
  `#[cfg(...)]` gate (which feature actually uses `vec!` in `vm.rs`); left for a dedicated small fix so as
  not to widen an unrelated increment.

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
