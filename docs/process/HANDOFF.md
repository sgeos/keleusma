# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `337bf17`
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

## Resume prompt — START ORDER 1. The drain item was PROBED and deferred. Do not re-ask.

**The mixed-subtree family is closed.** Six increments took the composite-equality frontier from 56
to 79 Ok. What remains in this family is NOT drain generalization:

- **Enum with a COMPOSITE payload** — PROBED 2026-08-03 and DEFERRED. It is NOT a contained
  extension of the enum block: a struct payload fails at DEPTH 1 too (4 ops against 90), so the gap is
  in the nested enum emitter generally, and array/tuple payloads fail even at TOP level. Needs new
  payload plumbing across all three stages, mirroring `push_enum_struct_payload_loop`. Do not re-probe
  it expecting a quick win.
- **A struct FIELD that is an array-of-tuple**, and **the `[bool;2]`-shaped struct field array** —
  these share ONE root cause: a struct field's array element type goes through `field_size_and_kind`,
  which accepts only an identifier, so the element layout is never recorded. Fixing that means a new
  layout table plus scanner work. A genuine context switch, and it would close two Gaps at once.
- **An array whose ELEMENT is itself composite at depth** (array blocks admit scalar elements only).

**DIRECTION: Order 1** (chosen, not open). With the drain family closed, the three Order-1 remainders are the
higher-value work — they are the whole of what stands between here and the Order-1 gate:
1. **Wire-format serialization** — self-contained and well-specified (framing header, operand-pool
   encoding, parity, CRC trailer; all host-side today, no `.kel` stage references `to_bytes`).
   Probably the best value per token.
2. **The monomorphizer** — near-identity over the subset, likely the cheapest of the three.
3. **The type checker** — largest and highest-risk (Hindley-Milner is not a streaming shape).

Start with **wire-format serialization** unless a probe says otherwise: it is self-contained, and
unlike the last six increments it does not touch the equality machinery, so their large regression
surface does not apply. `AUTONOMOUS_IMPLEMENTATION_LOOP.md` forbids prompting the operator to ORDER
any of this.

**The thirteen method rules. These found every bug across six increments; none is optional.**
1. **PROBE BEFORE PLANNING, always with a control**; confirm the REFERENCE accepts the source too.
2. **Probe what the admission ACCEPTS, not only what it rejects.** Every silent mis-compile found was
   in a construct the admission accepted.
3. **When generalizing a drain, tighten its admission IN THE SAME CHANGE.**
4. **Close an admission hole BEFORE building support over it**; expect +Gap / 0 Ok when you do.
5. **Never trust op counts as a correctness proxy.** For a deferral, assert its SHAPE.
6. **When a sufficient-looking change produces NO observable difference, suspect a BYPASSING path.**
7. **Abandon on TRAJECTORY, not attempt count.** Keep going while the divergence narrows and green
   fixtures stay green.
8. **Admission helpers call each other and R4 forbids cycles.** Inline rather than reuse.
9. **A self-compile failure in stage B can be caused by stage A merely GROWING** — and if the first
   factoring does not fix it, MEASURE which function is over. A ten-line probe over the harness names
   it; the loop-limit error names neither loop nor function.
10. **When a construct becomes supported, RETARGET the Gap fixture that pinned it.** Done three times
    on the same fixtures (tuple -> array -> enum -> enum-with-composite-payload).
11. **Read the divergence SIGNATURE.** One differing `Const` with matching lengths is a pool-ORDER
    bug; a shortfall of one compare block only where a sibling follows is a frame over-consuming; ONE
    extra `Loop` is a reused emitter that already emits its own.
12. **Edit fixtures by POSITION, not string replacement**, when the same source appears in both a
    positive and a negative test.
13. **The gate compiles the test crate more strictly than `cargo test`** (clippy `-D warnings`), and
    `EXPECTED_SELF_COMPILE` must be bumped whenever codegen.kel gains a function. Both fire ONLY in
    the full gate. Never land on targeted tests alone.

Steps: validate → familiarize (`REVERSE_PROMPT.md`, newest `DESIGN_JOURNAL.md`) → probe → implement on
a feature branch off `v0.2.3` → verify the regression surface FIRST, then the new fixtures, then the
boundary, then the FULL `scripts/release-gate.sh` → no-ff merge, push, confirm CI, record on all three
channels, prune the branch, restamp this HANDOFF.

**Git position** (as of the Parent commit)
- Branch `v0.2.3` tip is the probe-finding docs commit `337bf17` plus this restamp commit. In sync
  with origin, tree clean. Last full gate GREEN (240 suites) and CI GREEN on the code merge `239aa9c`;
  the commits after it are docs-only and passed the pre-push gate.
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
