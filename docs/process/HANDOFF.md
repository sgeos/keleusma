# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `0bf9daf`
- **Written**: 2026-08-02
- **Tree at write**: clean (all work committed, merged, and pushed; `v0.2.3` tip is the
  roadmap-baseline-correction merge `81c0bd9` plus this handoff restamp commit)
- **Context**: written after nested TUPLE sub-fields at depth landed (boundary 75 Ok / 4 Gap /
  1 RefRejects), the first slice of the mixed-subtree problem. Full gate GREEN, CI GREEN on
  `0bf9daf`, tree clean, all pushed.

**Validity check — run on resume, before trusting this handoff.** On the branch above, compare the
**Parent commit** to `git rev-parse HEAD~1`. Because this handoff file is itself committed, its commit
advances the tip by one, so the state it describes is the parent of the handoff commit. The two match
**only** when this handoff commit is still the branch tip and nothing has landed after it.

- **Match → VALID.** Proceed per the resume prompt below.
- **Mismatch → INVALID and STALE.** A later commit, a no-fast-forward merge, or a rebase moved the tip.
  Do **not** proceed and do **not** trust this handoff. Report the mismatch to the human pilot (recorded
  parent versus actual `HEAD~1`), familiarize from the live channels — `REVERSE_PROMPT.md`,
  `DESIGN_JOURNAL.md`, `TASKLOG.md`, and the git log, always authoritative — and wait for instruction.

## Resume prompt — IMPLEMENT array-at-depth, then enum-at-depth. The choice is MADE.

Tuples at depth are done (`0bf9daf`). The same frame machinery now needs the other two composite
kinds, in the SAME three dispatch sites (nested struct, tuple element, array element):

1. **An ARRAY sub-field at depth** — `struct I { xs: [Word;2] }` inside `struct S { i: I }`, measured
   DIVERGE and currently deferring. Do this first: the sentinel convention already exists (40000+size
   for an array, mirroring the 30000+size just added for tuples), and `0bf9daf` is a near-exact
   template.
2. **An ENUM sub-field at depth** — larger, because enums need variant dispatch.
3. A struct FIELD that is an array-of-tuple. NOTE: **not drain work.** The element layout is never
   recorded — `parray_tuple` is parameter-only and a struct field's array element type goes through
   `field_size_and_kind`, which accepts only an identifier. Needs a new layout table plus scanner
   work, which is why it sits behind the drain items.
4. The `[bool;2]`-shaped struct field array (element type not a recognized scalar).

**Read `0bf9daf` first** — it is the template, and its two traps are likely to recur.

**Read this before considering a stop.** `AUTONOMOUS_IMPLEMENTATION_LOOP.md` forbids prompting the
operator to ORDER bounded roadmap tasks. The test is **"does this choice require information only the
operator holds?"**

**The ten method rules. These found every bug so far; none is optional.**
1. **PROBE BEFORE PLANNING, always with a control**; also confirm the REFERENCE accepts the source.
2. **Probe what the admission ACCEPTS, not only what it rejects.**
3. **When generalizing a drain, tighten its admission IN THE SAME CHANGE.**
4. **Close an admission hole BEFORE building support over it**; expect +Gap / 0 Ok when you do.
5. **Never trust op counts or lengths as a correctness proxy** — the worst bug diverged at an
   IDENTICAL op count. Assert byte-identity; for a deferral, assert its SHAPE.
6. **When a change that should be sufficient produces NO observable difference, suspect a path that
   BYPASSES the code you changed.**
7. **Abandon on TRAJECTORY, not attempt count.** A hard increment may sit red for many commits and be
   healthy. Keep going while the divergence narrows and green fixtures stay green.
8. **Admission helpers call each other and R4 forbids cycles.** Relaxing one by calling another can
   make the stage unverifiable ("recursive call detected during WCMU topological sort"). Inline.
9. **A self-compile failure in stage B can be caused by stage A merely GROWING.** `LoopLimitExceeded`
   in an UNCHANGED reconstruct.kel meant a parse.kel block crossed a per-block cap. Factor into a
   helper rather than raising a limit; the error names neither loop nor function, so ask "what did I
   just make bigger?".
10. **When a construct becomes supported, RETARGET the Gap fixture that pinned it** rather than
    deleting it, or the deferral it guarded silently stops being tested.

Steps: validate → familiarize (`REVERSE_PROMPT.md`, newest `DESIGN_JOURNAL.md`, then `0bf9daf`) →
implement on a feature branch off `v0.2.3` → verify the regression surface FIRST, then the new
fixtures, then the boundary, then the FULL `scripts/release-gate.sh` → no-ff merge, push, confirm CI,
record on all three channels, prune the branch, restamp this HANDOFF.

**Git position** (as of the Parent commit)
- Branch `v0.2.3` tip is the nested-tuple-subfield merge `0bf9daf` plus this restamp commit. In sync
  with origin, tree clean, local full gate GREEN (240 suites), CI GREEN on `0bf9daf`.
- Local branches are pruned to `main`, `v0.2.3`, and `v0.2.3-prerebase-backup`. Origin holds only `main`
  and `v0.2.3`. **Do NOT delete `v0.2.3-prerebase-backup`**: it holds 309 commits not in `v0.2.3` and is
  a deliberate safety net, not clutter.
- `main` holds releases and sits behind `v0.2.3` by design (`docs/process/GIT_STRATEGY.md`).

**Boundary counts** — **75 Ok / 4 Gap / 1 RefRejects**, pinned by
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
