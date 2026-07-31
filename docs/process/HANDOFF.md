# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `4bac4a1`
- **Written**: 2026-07-31
- **Tree at write**: clean (all work committed, merged, and pushed; `v0.2.3` tip is the
  tuple-in-tuple merge `4bac4a1` plus this handoff restamp commit)
- **Context**: written after the tuple-in-tuple increment, which found the PLANNED WORK UNNECESSARY --
  the construct already self-compiled byte-identically, so the increment became regression pinning plus
  a corrected frontier map. Full gate GREEN, pushed, CI confirmed on `4bac4a1`. The operator's active
  workstream is DEEPER NESTING FOR OTHER COMPOSITES, but the frontier map below is now MEASURED rather
  than assumed. NOTE the environment caveat (broken Xcode) before running anything.

**Validity check — run on resume, before trusting this handoff.** On the branch above, compare the
**Parent commit** to `git rev-parse HEAD~1`. Because this handoff file is itself committed, its commit
advances the tip by one, so the state it describes is the parent of the handoff commit. The two match
**only** when this handoff commit is still the branch tip and nothing has landed after it.

- **Match → VALID.** Proceed per the resume prompt below.
- **Mismatch → INVALID and STALE.** A later commit, a no-fast-forward merge, or a rebase moved the tip.
  Do **not** proceed and do **not** trust this handoff. Report the mismatch to the human pilot (recorded
  parent versus actual `HEAD~1`), familiarize from the live channels — `REVERSE_PROMPT.md`,
  `DESIGN_JOURNAL.md`, `TASKLOG.md`, and the git log, always authoritative — and wait for instruction.

## Resume prompt — PICK A MEASURED-REAL GAP, or redirect

**The last increment's headline is a CORRECTION: tuple-in-tuple did NOT need implementing.** The prior
handoff recorded it as a Gap requiring a multi-stage drain generalization. That premise was FALSE. A
differential probe (with a CONTROL -- see below) showed the pipeline already emits
`GetTupleField(FlatNested { variant: Tuple })` plus a nested compare loop, byte-identical to the
reference. The increment was redirected to pinning the previously unguarded support and correcting the
frontier map. ZERO product code changed.

**THE METHOD LESSON, which governs the next increment: PROBE BEFORE PLANNING.** A conservative ADMISSION
deferral is NOT evidence of a capability gap -- the path it defers to may already be correct. One probe
with a control cost minutes and saved a rewrite of three `.kel` stages. ALWAYS run the control (point the
same probe at a known Gap such as `float_arith`); without it a false "identical" is indistinguishable
from a real one, because `self_host_compile` builds on `compile_src` and replaces chunk bodies.

Steps, in order:

1. **Validate** — run the validity check above. Valid → continue. Invalid → stop and report.
2. **Fix the environment FIRST** — the OS update broke `Xcode.app` (`xcrun` cannot find `clang`; every
   Rust link fails). Until the operator runs `sudo xcode-select -s /Library/Developer/CommandLineTools`,
   EVERY cargo and git command needs the prefix `DEVELOPER_DIR=/Library/Developer/CommandLineTools`
   (including `git push`, whose pre-push hook runs the gate). Check whether the fix has been applied
   before assuming a build failure is a code problem.
3. **Familiarize** — read `docs/process/REVERSE_PROMPT.md` (holds the MEASURED frontier map),
   `docs/process/DESIGN_JOURNAL.md` (newest entry: tuple-in-tuple, incl. the unexplained mechanism), and
   `docs/process/TASKLOG.md`.
4. **Confirm direction with the operator, then proceed.** Do NOT auto-start. Options, measured this
   session rather than assumed:
   - **Cheapest (no product code)**: pin array-of-array (`[[Word;2];2] == [[Word;2];2]`) and an enum
     tuple payload (`enum E { A(Word, Word), B }`) — both verified already supported but UNPINNED.
   - **Smallest genuine capability gaps** (all measured DIVERGE): enum with an array payload;
     `struct { t: (P, Word) }` (tuple-of-struct inside a struct); `struct { i: I }` where `I` holds an
     enum or an array; array-of-array nested in a struct.
   - **Larger gaps**: array-of-deep-struct; array of tuple-of-struct; enum with a deep struct payload;
     enum containing a struct containing an enum.
   - Or redirect entirely (new self-host language surface, native-call support, release cadence).
   NOTE: tuple-in-tuple, tuple-of-deep-struct, 3-level struct nesting, mixed subtrees INVOLVING TUPLES,
   and the CLI backend hardening are all DONE — do not re-offer them.

Follow the normal increment cycle (feature branch off `v0.2.3`, byte-identity oracle + FULL
`scripts/release-gate.sh`, no-ff merge, push, confirm CI, record on all three channels, restamp this
HANDOFF before the next planned compaction).

**Git position** (as of the Parent commit)
- Branch `v0.2.3` tip is the tuple-in-tuple merge `4bac4a1` (feature branch
  `feat/selfhost-tuple-in-tuple` merged no-ff) plus this handoff restamp commit. In sync with origin,
  working tree clean, local full gate GREEN, CI GREEN on `4bac4a1`.
- `main` holds releases and sits behind `v0.2.3` by design. Branch model in
  `docs/process/GIT_STRATEGY.md` (release-branch, no-fast-forward merges up the hierarchy).

**Boundary counts** — **65 Ok / 2 Gap / 1 RefRejects**, pinned by
`self_hosted_construct_support_boundary` in `tests/selfhost_codegen.rs`. WARNING: the previously
documented "54 Ok" was STALE BY 2 (the case list already held 56 before the last increment). Trust the
test file, not a remembered number; recount with a grep if it matters.

**Open concern carried forward — an UNEXPLAINED MECHANISM**
- How `parse.kel` represents a nested tuple PARAMETER TYPE was never localized. `step_tuple_type`
  (~1457) reads as a flat state machine handling only `Ident`/`RParen`, has a single definition, there
  is no `tup_etuple` table analogous to `tup_estruct`, and no paren-depth state was found. That reading
  PREDICTS `a.1` on `((Word,Word),Word)` lowering to flat offset 8; the MEASURED output is 16. The
  reading is therefore wrong somewhere. Behavior is established by the oracle with working controls, but
  anyone extending the tuple layout must re-derive the real mechanism FIRST.

**Done this arc**
- Tuple-in-tuple (`4bac4a1`): no implementation needed; nine boundary cases plus
  `self_host_compiles_tuple_in_tuple_equality` pin both element positions, three levels, a `Byte` leaf,
  `!=`, a struct beside a nested tuple, array-of-tuple, and nested-element ACCESS (`a.1` → offset 16,
  pinning the LAYOUT not just the equality). Boundary 56 → 65 Ok.
- Tuple-of-deep-struct (`67539e7`), arbitrary-depth struct-in-struct (`5c93920`), CLI-backend error
  hardening (`cf24f12`), and increments 1-5 (tuple-of-struct, enum-in-struct, enum-with-struct-payload,
  2-level struct, struct-of-array-of-struct) — all merged, byte-identical, gate-green. See
  `DESIGN_JOURNAL.md` for the reasoning on each.
- Capacity: the lexer `src.bytes` source buffer is 393216 and the `dl_reject_module_via_kel` arena 4 MB.
  Resizing a shared byte-array buffer expands the per-element data layout and can cascade into
  layout-verifier arena limits — bump together.

**Observed pre-existing warning** (not introduced, not fixed — out of scope)
- `src/vm.rs:8 use alloc::vec;` is flagged `unused_imports` in the `--no-default-features` `cargo test`
  build only. The full gate stays GREEN because that step does not deny warnings. A correct fix needs the
  right `#[cfg(...)]` gate; left for a dedicated small fix.

**Key durable finding** (governs every remaining depth increment)
- The total-language verifier FORBIDS recursion (R4, acyclic call graph); no `.kel` stage function may
  self-recurse. So each additional composite-nesting DEPTH that genuinely needs work is an explicit extra
  phase/stack in the drain, not a copy-recurse. Byte-identity also hinges on the monotonic slot-order:
  extract temps allocate depth-first, r2 before l2, +2 per level, matching the reference's `next_slot`
  (never rewound by `end_scope`).

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
