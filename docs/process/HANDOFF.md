# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `48a05f7`
- **Written**: 2026-08-04
- **Tree at write**: clean. One docs-only commit carrying the v2 design and this handoff, merged to
  `v0.2.3` and pushed. No product code changed.
- **Context**: the wire-format work has shifted from "replace rkyv" to a designed-from-requirements
  format with its own crate. The operator has stated a six-point plan (below). Prototyping is live in
  `secret/`, which is gitignored, and is NOT part of the documentation graph.
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** It defines what must not
  appear in this repository. Tracked material was sanitized against it on 2026-08-04.

**Validity check — run on resume, before trusting this handoff.** Compare the **Parent commit** above
to `git rev-parse HEAD~1`.

- **Match → VALID.** Proceed per the resume prompt below.
- **Mismatch → INVALID and STALE.** Report the mismatch, familiarize from `REVERSE_PROMPT.md`,
  `DESIGN_JOURNAL.md`, `TASKLOG.md`, and the git log, and wait for instruction.

## On resume after compaction — do these first

1. **Run the validity check above.** Mismatch → stop and report; do not trust anything below.
2. **Read `secret/notes/APPENDIX_B.md` before writing ANY tracked file**, commit message, or code
   comment for this work. It defines what must not appear in this repository. Tracked material was
   sanitized against it on 2026-08-04. **Hard constraint, not a preference.**
3. **Re-read the three channels fresh** — they are authoritative and current, and this handoff is a
   convenience snapshot, not the source of truth:
   [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md) (bounded latest state and next step),
   [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md) (newest entry first), [`TASKLOG.md`](./TASKLOG.md).
4. **Read the two design documents** listed under "Design documents" below. Do not re-derive what
   they already record.
5. **In-flight verification: NONE.** No gate, CI run, or background agent is pending. The one commit
   on this branch is docs-only and has NOT been gated or pushed.

## Resume prompt — THE WIRE-FORMAT PROGRAMME

The operator's plan, in order. Each step gates the next; do not skip ahead.

1. **Prototype the wire format until it can be locked in.**
2. **Add a new wire-format crate**, usable by other projects as an alternative to `rkyv`, in the same
   way `keleusma-arena` is nominally useful outside this repository.
3. **Document the WHAT of the format, without the sensitive internal reasoning about the WHY.**
4. **Implement the wire format in Rust.**
5. **Port Keleusma to it.**
6. **Self-host the wire format in Keleusma.** This implies **the Rust must be Keleusma-like**.

### Step 6 is a constraint on step 4, not a later concern

Writing the Rust in a Keleusma-transliterable style is the single highest-leverage instruction here,
because it makes step 6 a translation rather than a rewrite. Concretely, in the wire-format crate:

- **No recursion.** Keleusma forbids it (R4). Walk with an explicit, bounded stack.
- **Bounded loops** with a static cap, mirroring `for … limit N`.
- **No dynamic allocation on the read path.** Borrowed views, never owned materialization.
- **Fixed-size records, unrolled field access.** Literal place values (`1, 256, 65536, …`), not
  computed shifts — validated in `secret/kel-format-probe/` as simultaneously the most graceful
  Keleusma, the lowest-state, and the most hardware-like form.
- **No traits, generics, or dynamic dispatch** in the codec core. Keleusma has none of it.
- **State in explicit structs**, not local mutation — Keleusma has no `let mut`.

### Where the prototype stands (all in `secret/`, gitignored)

- `kel-format-probe/wireimage.kel` — Keleusma PRODUCER emitting a 160-byte artifact.
- `kel-format-probe/image.py` — independent reference emitter. **Checksums agree at 4016.**
- `silicon-prototype/wire_decode.vhd` + `tb_wire.vhd` — VHDL CONSUMER of those exact bytes.
  **PASS**: magic, region count, both regions, absent-region not-found, all three chunk descriptors.
- `silicon-prototype/tb_wire_corrupt.vhd` — one corrupted header copy outvoted AND flagged. **PASS.**
- `silicon-prototype/secded_*` — (72,64) SECDED, exhaustively validated in Python AND simulated in
  VHDL: 432/432 single-bit corrected, 15336/15336 double-bit detected.
- Toolchain: `nvc` 1.23-devel installed at `/usr/local/bin/nvc`.

### Lock-in is a judgement call, not a checklist

**RESOLVED by the operator (2026-08-04): the criterion is practical and requires judgement.** A
proof of concept needs only to be good enough to make a decision and move on. Do NOT gold-plate the
prototype or hold the format hostage to exhaustive coverage.

That said, two gaps are worth closing first because each could still change the record layouts, and
changing them after step 4 is expensive:
- The fetch path stops at the chunk descriptor. It does not follow `const_first`/`const_count` into a
  constant table, nor resolve a string slice out of the pool. **This is the remaining layout-sensitive
  step and the one most likely to force a change.**
- Emission is only tested from a terminating `fn`. A real stage is `loop main` yielding incrementally,
  which is where forward-only emission either pays off or does not.

### Resolved by the operator, 2026-08-04

1. **Crate scope: MECHANISM ONLY.** The crate provides regions, fixed-size records, pools, framing,
   the ECC plane, and the integrity primitives. It must NOT depend on the Keleusma runtime and must
   NOT hardcode `WireChunk` / `ConstValue`; Keleusma's schema layers on top, in the `keleusma` crate.
   This is what makes it usable by other projects, which is the stated point.
2. **Documentation boundary: see `secret/notes/APPENDIX_B.md`.** Read it BEFORE writing any tracked
   documentation, commit message, or code comment for this work. The tracked design documents state
   the format's engineering PROPERTIES only; requirements context lives in Appendix B and must not be
   restated in tracked files. **This is a hard constraint, not a stylistic preference.**
3. **Crate name**: `keleusma-wire`, paralleling `keleusma-arena`.
4. **Step 6 covers BOTH encoder and decoder.** Confirmed. The encoder is needed for self-hosted
   compilation; the decoder is needed because the `verify_*.kel` family consumes module data that the
   host currently marshals for it, because the natural self-hosted oracle is encode-then-decode in
   Keleusma and compare, and because the meta-circular runtime (Workstream D) and the Keleusma-hosted
   runtime (V0.5.0) both have to read artifacts.

### Design documents (authoritative, in the graph)

- [`../decisions/WIRE_FORMAT_V2_WORD_ORIENTED.md`](../decisions/WIRE_FORMAT_V2_WORD_ORIENTED.md) —
  the current design. Word-oriented, fixed-size records, parallel ECC plane, per-region encryption,
  triplicated directory, plus the Keleusma-expressibility test and its three rules.
- [`../decisions/WIRE_FORMAT_V2_FLAT_AUX.md`](../decisions/WIRE_FORMAT_V2_FLAT_AUX.md) — superseded on
  record structure, but **its P10 analysis still governs**: string constants materialise as `KStr`
  aliasing the image, so the accessor layer must be a BORROWED VIEW, never an owned decode.
- `src/wire_aux.rs` on this branch implements the SUPERSEDED variable-length design. Its primitive
  layer, tag discipline, and totality tests are reusable; **its record structure is not** and should
  not be carried into the new crate unexamined.

### Standing method rules

The fourteen rules from the previous arc still apply, are recorded in the channels, and are not
repeated here. The two that have earned their keep most recently:

- **Cross-check across independent implementations.** The Keleusma/Python checksum disagreement (3968
  vs 4016) localised a mistranscribed magic constant in one step. Build the cross-check before it is
  needed, not after.
- **Run the FULL gate before landing.** Clippy `-D warnings` and `EXPECTED_SELF_COMPILE` fire only
  there, and a documented command (`cargo doc --workspace`) once disagreed with the gate's own doc
  step, hiding a real defect in shipped docs.

**Active increment**: step 1 of the six-step wire-format programme (prototype toward lock-in), moving
into step 2 (the `keleusma-wire` crate). Plans are the two design documents below; the prototype is in
`secret/` and is gitignored, so it is absent from every commit and must be rebuilt from
`secret/silicon-prototype/README.md` if lost.

**Git position** (as of the Parent commit)
- Branch `v0.2.3` tip is the Order-1 reassessment docs commit `5b58820` plus this restamp commit. In
  sync with origin, tree clean. Last full gate GREEN (240 suites) and CI GREEN on the code merge
  `239aa9c`; every commit after it is docs-only and passed the pre-push gate.
- Local branches are pruned to `main`, `v0.2.3`, and `v0.2.3-prerebase-backup`. Origin holds only `main`
  and `v0.2.3`. **Do NOT delete `v0.2.3-prerebase-backup`**: it holds 309 commits not in `v0.2.3` and is
  a deliberate safety net, not clutter.
- `main` holds releases and sits behind `v0.2.3` by design (`docs/process/GIT_STRATEGY.md`).

**Boundary counts** — **79 Ok / 4 Gap / 1 RefRejects**, UNCHANGED by the wire-format work (no product
code has been touched for it). Pinned by
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
