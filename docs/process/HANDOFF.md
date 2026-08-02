# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `f74eb29`
- **Written**: 2026-08-02
- **Tree at write**: clean (all work committed, merged, and pushed; `v0.2.3` tip is the
  roadmap-baseline-correction merge `81c0bd9` plus this handoff restamp commit)
- **Context**: written after a loop-protocol compliance fix and the SCOPING of the next increment.
  The loop had stopped to ask which bounded roadmap task to take next, which the protocol already
  forbade; the stop list is now hardened and the rule was APPLIED rather than re-asked. The next
  increment is SELECTED and fully specified in `docs/decisions/STRUCT_TUPLE_OF_STRUCT_PLAN.md`.
  Tree clean, all pushed. CI GREEN on `81c0bd9`; the four docs commits after it are docs-only and
  passed the pre-push gate.

**Validity check — run on resume, before trusting this handoff.** On the branch above, compare the
**Parent commit** to `git rev-parse HEAD~1`. Because this handoff file is itself committed, its commit
advances the tip by one, so the state it describes is the parent of the handoff commit. The two match
**only** when this handoff commit is still the branch tip and nothing has landed after it.

- **Match → VALID.** Proceed per the resume prompt below.
- **Mismatch → INVALID and STALE.** A later commit, a no-fast-forward merge, or a rebase moved the tip.
  Do **not** proceed and do **not** trust this handoff. Report the mismatch to the human pilot (recorded
  parent versus actual `HEAD~1`), familiarize from the live channels — `REVERSE_PROMPT.md`,
  `DESIGN_JOURNAL.md`, `TASKLOG.md`, and the git log, always authoritative — and wait for instruction.

## Resume prompt — IMPLEMENT `struct { t: (P, Word) }`. The choice is MADE. Do not re-ask.

**Start here, not with a survey.** The next increment is selected, probed, diagnosed, and de-risked.
The blueprint is [`../decisions/STRUCT_TUPLE_OF_STRUCT_PLAN.md`](../decisions/STRUCT_TUPLE_OF_STRUCT_PLAN.md)
— it contains the measured op-level divergence, the stage split, and the concrete stage-1 `parse.kel`
edit written out verbatim with its two traps. Do not re-derive it.

**Read this before considering a stop.** On 2026-08-02 the loop stopped to ask the operator which
bounded roadmap task to take next. `AUTONOMOUS_IMPLEMENTATION_LOOP.md` already forbade that in two
places, and the stop list now additionally names and excludes the four rationalizations that were
used: cost asymmetry, "wants a dedicated run at the budget", "it all has to happen anyway so which
first", and "the cheap work is exhausted". The test is **"does this choice require information only
the operator holds?"** — not "is this choice significant?". Effort, risk, and sequencing are yours.

Steps, in order:

1. **Validate** — run the validity check above. Valid → continue. Invalid → stop and report.
2. **Familiarize** — the plan doc first, then `REVERSE_PROMPT.md` and the newest `DESIGN_JOURNAL.md`
   entry. Skip `TASKLOG.md` detail unless something conflicts.
3. **Implement, on a feature branch cut from `v0.2.3`.** Stage 1 parse.kel (small, written out),
   stage 2 reconstruct.kel (probably NOTHING — verify the depth-1/2 fixtures first), stage 3
   codegen.kel (the real work: thread the parent `seb` block's FlatNested variant down the `es_*`
   frame stack so the extract picks `GetTupleField` vs `GetField`), stage 4 the admission.
   Intermediate commits may be red; the tip must be green.
4. **Verify** — depth-1/2 fixtures byte-identical FIRST (a regression there is a stop), then the
   three new fixtures: `(P, Word)` → 59 ops / `local_count` 8, `(P, P)` → 74 ops, and
   `(P, Word), w: Word` → 69 ops. Then the boundary case as `SOk`, then the FULL
   `scripts/release-gate.sh`.
5. **Land** — no-ff merge into `v0.2.3`, push, confirm CI, record on all three channels, prune the
   merged branch, restamp this HANDOFF.

**This construct is currently MIS-COMPILED, not merely unsupported** — the admission admits it and
the drain compares a struct element as a scalar. It is a correctness fix, so do not downgrade it to
a coverage increment or weaken a fixture to reach green.

**If it does not converge**: two or three bounded attempts, then abandon the branch and re-cut, per
the stop list. Do not thrash, and never weaken the oracle.

**After it lands**, the measured queue continues (same context, no operator prompt needed):
array-of-tuple-of-struct and the mixed-subtree gaps reuse the SAME per-frame-accessor machinery this
increment builds, so they should follow immediately while it is fresh.

**Git position** (as of the Parent commit)
- Branch `v0.2.3` tip is the roadmap-baseline-correction merge `81c0bd9` plus this restamp commit. In
  sync with origin, tree clean, local full gate GREEN, CI GREEN on `81c0bd9` (20/20 jobs).
- Local branches are pruned to `main`, `v0.2.3`, and `v0.2.3-prerebase-backup`. Origin holds only `main`
  and `v0.2.3`. **Do NOT delete `v0.2.3-prerebase-backup`**: it holds 309 commits not in `v0.2.3` and is
  a deliberate safety net, not clutter.
- `main` holds releases and sits behind `v0.2.3` by design (`docs/process/GIT_STRATEGY.md`).

**Boundary counts** — **67 Ok / 2 Gap / 1 RefRejects**, pinned by
`self_hosted_construct_support_boundary` in `tests/selfhost_codegen.rs`. Recount with a grep rather than
trusting a remembered number; the figure in the docs was found stale by 2 on 2026-07-30.

**Measured frontier (probed, not assumed)**
- SUPPORTED and now pinned: the whole tuple-in-tuple family, array-of-tuple, struct-in-nested-tuple,
  nested-element access, array-of-array, enum tuple payload.
- REAL GAPS: array-of-array inside a struct; enum array payload; enum deep-struct payload;
  `struct { t: (P, Word) }`; `struct { i: I }` where `I` holds an enum or array; array-of-deep-struct;
  array of tuple-of-struct; enum→struct→enum; and the `for … on` outcome-arm form.
- ASYMMETRY WARNING: support does NOT generalize to the enclosing-composite form. Array-of-array is
  supported but array-of-array in a struct is not; an enum tuple payload is supported but an enum array
  payload is not. Never infer support by analogy — probe it.

**Open concern carried forward — an UNEXPLAINED MECHANISM**
- How `parse.kel` represents a nested tuple PARAMETER TYPE was never localized. `step_tuple_type`
  (~1457) reads as a flat state machine handling only `Ident`/`RParen`, has a single definition, there
  is no `tup_etuple` table analogous to `tup_estruct`, and no paren-depth state was found. That reading
  PREDICTS `a.1` on `((Word,Word),Word)` lowering to flat offset 8; the MEASURED output is 16. Anyone
  extending the tuple layout must re-derive the real mechanism FIRST.

**Environment** — the broken-Xcode issue is RESOLVED. The operator ran
`sudo xcode-select -s /Library/Developer/CommandLineTools`; native linking is confirmed working and the
`DEVELOPER_DIR` prefix is no longer needed.

**Observed pre-existing warning** (not introduced, not fixed — out of scope)
- `src/vm.rs:8 use alloc::vec;` is flagged `unused_imports` in the `--no-default-features` `cargo test`
  build only. The full gate stays GREEN because that step does not deny warnings. A correct fix needs the
  right `#[cfg(...)]` gate; left for a dedicated small fix.

**Key durable finding** (governs every remaining depth increment)
- The total-language verifier FORBIDS recursion (R4, acyclic call graph); no `.kel` stage function may
  self-recurse. So each additional composite-nesting DEPTH that genuinely needs work is an explicit extra
  phase/stack in the drain, not a copy-recurse. Byte-identity also hinges on the monotonic slot-order:
  extract temps allocate depth-first, r2 before l2, +2 per level, matching the reference's `next_slot`.

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
