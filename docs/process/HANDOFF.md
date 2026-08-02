# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `87e1138`
- **Written**: 2026-08-02
- **Tree at write**: clean (all work committed, merged, and pushed; `v0.2.3` tip is the
  roadmap-baseline-correction merge `81c0bd9` plus this handoff restamp commit)
- **Context**: written after the struct-field-tuple-of-struct increment (a silent MIS-COMPILE fixed,
  boundary 67 -> 69 Ok, CI GREEN on `3f97b42`) and the scoping of the next one. Tree clean, all
  pushed. The next increment is selected and blueprinted; start by implementing it, not by surveying.

**Validity check — run on resume, before trusting this handoff.** On the branch above, compare the
**Parent commit** to `git rev-parse HEAD~1`. Because this handoff file is itself committed, its commit
advances the tip by one, so the state it describes is the parent of the handoff commit. The two match
**only** when this handoff commit is still the branch tip and nothing has landed after it.

- **Match → VALID.** Proceed per the resume prompt below.
- **Mismatch → INVALID and STALE.** A later commit, a no-fast-forward merge, or a rebase moved the tip.
  Do **not** proceed and do **not** trust this handoff. Report the mismatch to the human pilot (recorded
  parent versus actual `HEAD~1`), familiarize from the live channels — `REVERSE_PROMPT.md`,
  `DESIGN_JOURNAL.md`, `TASKLOG.md`, and the git log, always authoritative — and wait for instruction.

## Resume prompt — IMPLEMENT array-of-tuple-of-struct. The choice is MADE. Do not re-ask.

Blueprint: [`../decisions/ARRAY_OF_TUPLE_OF_STRUCT_PLAN.md`](../decisions/ARRAY_OF_TUPLE_OF_STRUCT_PLAN.md).
Already probed with a control, so do not re-derive: `[(P, Word); 2]` gives 83 self-hosted ops against
113 reference; the struct-field form 73 against 128.

**Read this before considering a stop.** `AUTONOMOUS_IMPLEMENTATION_LOOP.md` forbids prompting the
operator to ORDER bounded roadmap tasks, and names the four rationalizations that do not license a
stop (cost asymmetry, "wants a dedicated run", "it all has to happen anyway", "the cheap work is
exhausted"). The test is **"does this choice require information only the operator holds?"**

**The two hard-won method rules, which found both bugs so far:**
1. **PROBE BEFORE PLANNING, always with a control** (point it at `scope/float_arith__GAP` and confirm
   DIVERGE; also check the REFERENCE accepts the source — a reference rejection is not a self-host
   gap). Two authorized increments turned out to be already done, and one plan was wrong about which
   machinery applied.
2. **Probe what the admission ACCEPTS, not only what it rejects,** and when generalizing a drain,
   TIGHTEN ITS ADMISSION IN THE SAME CHANGE. Both silent mis-compiles found so far were in constructs
   the admission accepted. On 2026-08-02, teaching a drain to descend recreated the same silent bug
   one level deeper, caught only because the impure cases were probed before declaring done.

Steps, in order:

1. **Validate** — the check above. Valid → continue. Invalid → stop and report.
2. **Familiarize** — the blueprint first, then `REVERSE_PROMPT.md`, then the newest
   `DESIGN_JOURNAL.md` entry.
3. **Implement** on a feature branch cut from `v0.2.3`. NOTE the blueprint's central correction: this
   does NOT reuse the per-frame accessor machinery. The array-of-struct/tuple family is a separate
   FLAT drain with no nested form. Prefer routing array-of-composite elements through the
   `StructEqNested` frame machinery (it would also subsume array-of-deep-struct and
   array-of-array-in-struct); fall back to giving the flat family its own nested form after two or
   three bounded attempts. Also fold in the `tup_estruct` recording for the `ps.arr == 3` scanner —
   necessary, but verified NOT sufficient alone, so it must land WITH the drain change.
4. **Verify** — the large regression surface FIRST (`eq/array_of_tuple`, `eq/struct_arrayofstruct`,
   `eq/array_in_struct`, `eq/array_of_array`, and the `!=` forms), then the two new fixtures, then the
   boundary, then the FULL `scripts/release-gate.sh`.
5. **Land** — no-ff merge into `v0.2.3`, push, confirm CI, record on all three channels, prune the
   merged branch, restamp this HANDOFF.

**If it does not converge**: two or three bounded attempts, then abandon the branch and re-cut with
the fallback approach. Never weaken the oracle or a fixture to reach green.

**After it lands**, the same-context queue continues without an operator prompt: the impure-element
subtree deferred on 2026-08-02 (the general mixed-subtree problem), enum array payload, enum
deep-struct payload, enum→struct→enum, array-of-array in a struct. Beyond this family, the Order-1
gate needs the type checker, the monomorphizer, and wire-format serialization.

**Git position** (as of the Parent commit)
- Branch `v0.2.3` tip is the array-of-tuple-of-struct SCOPING commit `87e1138` plus this restamp
  commit. In sync with origin, tree clean. Last full gate GREEN and CI GREEN on `3f97b42` (20/20
  jobs); the two commits after it are docs-only and passed the pre-push gate.
- Local branches are pruned to `main`, `v0.2.3`, and `v0.2.3-prerebase-backup`. Origin holds only `main`
  and `v0.2.3`. **Do NOT delete `v0.2.3-prerebase-backup`**: it holds 309 commits not in `v0.2.3` and is
  a deliberate safety net, not clutter.
- `main` holds releases and sits behind `v0.2.3` by design (`docs/process/GIT_STRATEGY.md`).

**Boundary counts** — **69 Ok / 3 Gap / 1 RefRejects**, pinned by
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
