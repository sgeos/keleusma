# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `81c0bd9`
- **Written**: 2026-07-31
- **Tree at write**: clean (all work committed, merged, and pushed; `v0.2.3` tip is the
  roadmap-baseline-correction merge `81c0bd9` plus this handoff restamp commit)
- **Context**: written after two back-to-back correction increments. Neither wrote product code;
  BOTH found that the recorded plan pointed at work already done. Full gate GREEN and CI GREEN on
  `81c0bd9` (20/20 jobs). The cheap work is now EXHAUSTED — what remains is a genuine strategic fork
  that needs an operator decision (see step 4).

**Validity check — run on resume, before trusting this handoff.** On the branch above, compare the
**Parent commit** to `git rev-parse HEAD~1`. Because this handoff file is itself committed, its commit
advances the tip by one, so the state it describes is the parent of the handoff commit. The two match
**only** when this handoff commit is still the branch tip and nothing has landed after it.

- **Match → VALID.** Proceed per the resume prompt below.
- **Mismatch → INVALID and STALE.** A later commit, a no-fast-forward merge, or a rebase moved the tip.
  Do **not** proceed and do **not** trust this handoff. Report the mismatch to the human pilot (recorded
  parent versus actual `HEAD~1`), familiarize from the live channels — `REVERSE_PROMPT.md`,
  `DESIGN_JOURNAL.md`, `TASKLOG.md`, and the git log, always authoritative — and wait for instruction.

## Resume prompt — THE CHEAP WORK IS DONE; GET A DECISION BEFORE SPENDING BUDGET

**Read this first: the last two increments both found the plan STALE rather than finding work to do.**
Tuple-in-tuple was recorded as a Gap needing a multi-stage rewrite — it already worked. Then four of six
Workstream A Order-1 residuals were recorded as open — they were already closed. Three stale claims
surfaced in one day (the boundary count, the tuple-in-tuple premise, four Order-1 residuals), **all in
the same direction: understating what had landed.** Do not trust a status claim in any planning document
until you have probed it.

**THE GOVERNING METHOD: PROBE BEFORE PLANNING, ALWAYS WITH A CONTROL.** Point the same probe at a known
Gap (`scope/float_arith__GAP`) and confirm it reports DIVERGE. Without that control a false "identical"
is indistinguishable from a real one, because `self_host_compile` builds on `compile_src(src)` and
replaces chunk bodies — a skipped replacement would report identity trivially. Also: a REFERENCE
rejection is NOT a self-host gap. Check `compile_src` alone first; several probe sources were rejected
for bad syntax (the language has no `let mut`, and a `for` needs `limit` — take valid syntax from
`tests/for_limit.rs`).

Steps, in order:

1. **Validate** — run the validity check above. Valid → continue. Invalid → stop and report.
2. **Familiarize** — read `docs/process/REVERSE_PROMPT.md` (the sharpened fork and the measured
   frontier), `docs/process/DESIGN_JOURNAL.md` (newest two entries), and `docs/process/TASKLOG.md`.
3. **Do NOT auto-start an increment.** The cheap, no-decision work is exhausted.
4. **Put the fork to the operator.** What actually remains before the Order-1 gate is exactly three
   things — the type checker, the monomorphizer, and wire-format serialization:
   - **Wire-format serialization** — well-specified and self-contained (framing header, operand-pool
     encoding, parity, CRC trailer; all host-side today, no `.kel` stage references `to_bytes`).
     Probably the best value per token.
   - **The monomorphizer** — near-identity over the subset, likely the cheapest of the three.
   - **The type checker** — the largest and highest-risk port (Hindley-Milner is not a streaming
     shape). Wants a dedicated run at the seven-day budget.
   - **The `for … limit … on { ok/break(bi)/limit }` outcome-arm gap** — bounded, well-scoped, same
     shape as recent increments. A bare `break;` self-hosts fine; only the outcome-arm form diverges.
   - **More subset-widening** (deeper array/enum nesting, mixed subtrees involving array/enum) —
     steady and low-risk, but Workstream F, whose gate sits BEHIND Order 1.
   RECOMMENDATION: wire-format serialization or the monomorphizer, because they close Order 1 rather
   than widen a subset gated behind it.

Follow the normal increment cycle (feature branch off `v0.2.3`, byte-identity oracle + FULL
`scripts/release-gate.sh`, no-ff merge, push, confirm CI, record on all three channels, prune the merged
feature branch, restamp this HANDOFF).

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
