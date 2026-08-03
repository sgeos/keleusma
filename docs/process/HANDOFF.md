# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `5b58820`
- **Written**: 2026-08-02
- **Tree at write**: clean (all work committed, merged, and pushed; `v0.2.3` tip is the
  roadmap-baseline-correction merge `81c0bd9` plus this handoff restamp commit)
- **Context**: written after nested ENUM sub-fields landed, COMPLETING the mixed-subtree family --
  tuples, arrays and enums all nest now (boundary 79 Ok / 4 Gap / 1 RefRejects). Full gate GREEN, CI
  GREEN on `239aa9c`, tree clean, all pushed. The remaining Gaps in this family are NO LONGER drain
  work, which changes the recommended direction (see below).

**Validity check — run on resume, before trusting this handoff.** On the branch above, compare the
**Parent commit** to `git rev-parse HEAD~1`. Because this handoff file is itself committed, its commit
advances the tip by one, so the state it describes is the parent of the handoff commit. The two match
**only** when this handoff commit is still the branch tip and nothing has landed after it.

- **Match → VALID.** Proceed per the resume prompt below.
- **Mismatch → INVALID and STALE.** A later commit, a no-fast-forward merge, or a rebase moved the tip.
  Do **not** proceed and do **not** trust this handoff. Report the mismatch to the human pilot (recorded
  parent versus actual `HEAD~1`), familiarize from the live channels — `REVERSE_PROMPT.md`,
  `DESIGN_JOURNAL.md`, `TASKLOG.md`, and the git log, always authoritative — and wait for instruction.

## Resume prompt — IMPLEMENT the TYPE CHECKER. It is the only unblocked Order-1 item.

All three Order-1 remainders were probed on 2026-08-03 and the earlier recommendation was WITHDRAWN:

- **Wire-format serialization — PARTIALLY BLOCKED.** The roadmap's enumeration omitted that the
  AUXILIARY BODY is `rkyv`-archived and carries everything except the opcode stream and operand pool.
  Full self-hosting needs an operator decision (reimplement rkyv, or change the aux-body encoding — a
  wire-format change, hence a `BYTECODE_VERSION` question), which is an ENUMERATED STOP. The
  non-rkyv slices are bounded but leave the aux body host-supplied and do NOT meet the gate wording.
  See [`../decisions/WIRE_FORMAT_SELFHOST_PLAN.md`](../decisions/WIRE_FORMAT_SELFHOST_PLAN.md).
  **If the operator has since decided, that decision governs — re-read it before assuming this stop.**
- **The monomorphizer — VACUOUS.** Identity over the subset; the `.kel` sources use no generics.
  Porting it changes no emitted byte. Do not pick it as "the cheapest" expecting value.
- **The type checker — UNBLOCKED and substantive.** The pipeline has NO type checking (its stages are
  lexer, parse, reconstruct, codegen, plus analyze and verify_*). Ill-typed programs are caught today
  only by the CLI's cross-check against the reference. Self-hosting it is what lets the self-hosted
  compiler reject bad programs on its own. Over the monomorphic Word/Byte subset it is far smaller
  than `typecheck.rs`'s 8601 lines.

**PROBE CAUTION, learned the hard way here:** `self_host_compile` calls `compile_src` FIRST and panics
whenever the REFERENCE rejects, so any "does the self-hosted path reject?" probe run through it is
CONFOUNDED and will report rejection for every ill-typed program. Probe the stages directly. More
generally: check what a harness itself does before trusting its verdict — the same control discipline
the byte-identity probes require.

**Note the ORACLE DIFFERS for this item.** Every increment of the last arc used byte-identity of
emitted ops. A type checker changes no output; its oracle is VERDICT AGREEMENT with the reference
(accept/reject, and ideally the same error) over a corpus of well- and ill-typed programs. Build that
corpus first, with both polarities, and treat a self-hosted ACCEPT of a reference-REJECT as the
serious direction — that is the unsound one.

Suggested first slice: the monomorphic scalar core — binary/unary operator operand types, assignment
and return types, and undefined identifiers — over the subset the stages themselves use. Widen from
there to composites.

**The fourteen method rules. These found every bug across seven increments; none is optional.**
1. **PROBE BEFORE PLANNING, always with a control**; confirm the REFERENCE accepts the source too.
2. **Probe what the admission ACCEPTS, not only what it rejects.**
3. **When generalizing a drain, tighten its admission IN THE SAME CHANGE.**
4. **Close an admission hole BEFORE building support over it**; expect +Gap / 0 Ok when you do.
5. **Never trust op counts as a correctness proxy.** For a deferral, assert its SHAPE.
6. **When a sufficient-looking change produces NO observable difference, suspect a BYPASSING path.**
7. **Abandon on TRAJECTORY, not attempt count.**
8. **Admission helpers call each other and R4 forbids cycles.** Inline rather than reuse.
9. **A self-compile failure in stage B can be caused by stage A merely GROWING** — and if the first
   factoring does not fix it, MEASURE which function is over.
10. **When a construct becomes supported, RETARGET the Gap fixture that pinned it.**
11. **Read the divergence SIGNATURE** — pool-order, over-consuming frame, and duplicated loop-open all
    have distinct shapes.
12. **Edit fixtures by POSITION, not string replacement**, when a source appears in two tests.
13. **The gate compiles the test crate more strictly than `cargo test`** (clippy `-D warnings`), and
    `EXPECTED_SELF_COMPILE` must be bumped when codegen.kel gains a function. Both fire ONLY in the
    full gate.
14. **A probe run through a harness that already invokes the reference cannot tell you what the
    self-hosted path does alone.** Check the harness before trusting its verdict.

Steps: validate → familiarize (`REVERSE_PROMPT.md`, newest `DESIGN_JOURNAL.md`) → probe → implement on
a feature branch off `v0.2.3` → verify verdict agreement on BOTH polarities, then the boundary, then
the FULL `scripts/release-gate.sh` → no-ff merge, push, confirm CI, record on all three channels,
prune the branch, restamp this HANDOFF.

**Git position** (as of the Parent commit)
- Branch `v0.2.3` tip is the Order-1 reassessment docs commit `5b58820` plus this restamp commit. In
  sync with origin, tree clean. Last full gate GREEN (240 suites) and CI GREEN on the code merge
  `239aa9c`; every commit after it is docs-only and passed the pre-push gate.
- Local branches are pruned to `main`, `v0.2.3`, and `v0.2.3-prerebase-backup`. Origin holds only `main`
  and `v0.2.3`. **Do NOT delete `v0.2.3-prerebase-backup`**: it holds 309 commits not in `v0.2.3` and is
  a deliberate safety net, not clutter.
- `main` holds releases and sits behind `v0.2.3` by design (`docs/process/GIT_STRATEGY.md`).

**Boundary counts** — **79 Ok / 4 Gap / 1 RefRejects**, pinned by
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
