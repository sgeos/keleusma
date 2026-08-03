# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `ff57dca`
- **Written**: 2026-08-02
- **Tree at write**: clean (all work committed, merged, and pushed; `v0.2.3` tip is the
  roadmap-baseline-correction merge `81c0bd9` plus this handoff restamp commit)
- **Context**: written after nested scalar-ARRAY sub-fields landed (boundary 77 Ok / 4 Gap /
  1 RefRejects), the second mixed-subtree slice after tuples. Full gate GREEN, CI GREEN on `ff57dca`,
  tree clean, all pushed. ENUM at depth is the last kind in this family.

**Validity check — run on resume, before trusting this handoff.** On the branch above, compare the
**Parent commit** to `git rev-parse HEAD~1`. Because this handoff file is itself committed, its commit
advances the tip by one, so the state it describes is the parent of the handoff commit. The two match
**only** when this handoff commit is still the branch tip and nothing has landed after it.

- **Match → VALID.** Proceed per the resume prompt below.
- **Mismatch → INVALID and STALE.** A later commit, a no-fast-forward merge, or a rebase moved the tip.
  Do **not** proceed and do **not** trust this handoff. Report the mismatch to the human pilot (recorded
  parent versus actual `HEAD~1`), familiarize from the live channels — `REVERSE_PROMPT.md`,
  `DESIGN_JOURNAL.md`, `TASKLOG.md`, and the git log, always authoritative — and wait for instruction.

## Resume prompt — IMPLEMENT enum-at-depth. The choice is MADE.

Tuples (`0bf9daf`) and scalar arrays (`ff57dca`) at depth are done. ENUM is the last composite kind
in this family and is the LARGEST of the three: enums need VARIANT DISPATCH, not a flat element or
field walk, so it will NOT be a copy of either slice.

**The right template is the DEPTH-1 enum field, not the tuple/array blocks**: `se_subisenum` with the
`se_e*` state in parse, and `push_nested_enum_loop` in codegen. Read those first. Note the eager
intern pass already replays an enum field's constant order (ename, vname, disc, per-field false, then
true, then false) — a nested enum block will have to replay the same order from wherever it sits in
the forest, which is the analogue of the array index pre-interning that `ff57dca` had to add.

After that, in the same family:
1. A struct FIELD that is an array-of-tuple. **NOT drain work** — the element layout is never
   recorded (`parray_tuple` is parameter-only; a struct field's array element type goes through
   `field_size_and_kind`, which accepts only an identifier). Needs a new layout table plus scanner
   work, a genuine context switch, which is why it keeps being ordered behind the drain items.
2. The `[bool;2]`-shaped struct field array (element type not a recognized scalar).
3. An array whose ELEMENT is itself composite at depth (array blocks admit scalar elements only).

Beyond this family the Order-1 gate needs the type checker, the monomorphizer, and wire-format
serialization. `AUTONOMOUS_IMPLEMENTATION_LOOP.md` forbids prompting the operator to ORDER any of it.

**The eleven method rules. These found every bug so far; none is optional.**
1. **PROBE BEFORE PLANNING, always with a control**; also confirm the REFERENCE accepts the source.
2. **Probe what the admission ACCEPTS, not only what it rejects.**
3. **When generalizing a drain, tighten its admission IN THE SAME CHANGE.**
4. **Close an admission hole BEFORE building support over it**; expect +Gap / 0 Ok when you do.
5. **Never trust op counts or lengths as a correctness proxy.** For a deferral, assert its SHAPE.
6. **When a change that should be sufficient produces NO observable difference, suspect a path that
   BYPASSES the code you changed.**
7. **Abandon on TRAJECTORY, not attempt count.** A hard increment may sit red for many commits and be
   healthy. Keep going while the divergence narrows and green fixtures stay green.
8. **Admission helpers call each other and R4 forbids cycles.** Inline rather than reuse.
9. **A self-compile failure in stage B can be caused by stage A merely GROWING.** Factor into a
   helper rather than raising a limit.
10. **When a construct becomes supported, RETARGET the Gap fixture that pinned it** rather than
    deleting it. This has happened twice already (tuple -> array -> enum) on the same fixtures.
11. **Read the divergence SIGNATURE.** All lengths matching with one differing `Const` means a
    constant-POOL ORDER bug. A fixed shortfall of one compare block, only where a sibling follows,
    means a frame consumed a field it should not have. Each signature points at a different layer.

**Run the FULL gate before landing, always.** On `ff57dca` targeted tests passed while the full gate
went red on `EXPECTED_SELF_COMPILE` (a deliberate counter of self-compiling codegen functions, bumped
whenever codegen.kel gains one). Adding a codegen helper REQUIRES bumping it with a comment.

Steps: validate → familiarize (`REVERSE_PROMPT.md`, newest `DESIGN_JOURNAL.md`, then the depth-1 enum
path) → implement on a feature branch off `v0.2.3` → verify the regression surface FIRST, then the new
fixtures, then the boundary, then the FULL `scripts/release-gate.sh` → no-ff merge, push, confirm CI,
record on all three channels, prune the branch, restamp this HANDOFF.

**Git position** (as of the Parent commit)
- Branch `v0.2.3` tip is the nested-array-subfield merge `ff57dca` plus this restamp commit. In sync
  with origin, tree clean, local full gate GREEN (240 suites), CI GREEN on `ff57dca`.
- Local branches are pruned to `main`, `v0.2.3`, and `v0.2.3-prerebase-backup`. Origin holds only `main`
  and `v0.2.3`. **Do NOT delete `v0.2.3-prerebase-backup`**: it holds 309 commits not in `v0.2.3` and is
  a deliberate safety net, not clutter.
- `main` holds releases and sits behind `v0.2.3` by design (`docs/process/GIT_STRATEGY.md`).

**Boundary counts** — **77 Ok / 4 Gap / 1 RefRejects**, pinned by
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
