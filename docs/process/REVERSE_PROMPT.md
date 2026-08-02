# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning and frontier assessments live in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-02 (session 36)

## Headline — the V0.2.x roadmap's Order-1 residual list was substantially STALE

Asked whether obvious roadmap work existed, the roadmap's own status claims were re-probed against the
code rather than relayed, because two stale claims had already surfaced the same day. **Four of six
Workstream A first-pass residuals were already CLOSED.** One previously unlisted gap was found.

**Already closed but still listed as open:**
- Module scaffold assembly — `self_host_compile_scratch` assembles data layout, enum table, signatures,
  schema hash, and the WCET/WCMU header with NO field borrowed from the reference.
- Integration into the shipping tool — the CLI's `self_hosted_compile` calls that scratch path.
- A conditional used as a call argument — byte-identical.
- A user-written `break;` statement — byte-identical inside `for … limit`.

**Genuinely open (the whole of what stands between here and the Order-1 gate):**
- The type checker in Keleusma (largest, highest risk — Hindley-Milner is not a streaming shape).
- The monomorphizer in Keleusma (near-identity over the subset).
- **Wire-format serialization** — no `.kel` stage references `to_bytes`, parity, or CRC; the framing
  header, operand-pool encoding, parity, and CRC trailer are all host-side.

**Newly identified, in no prior document:** the `for … limit … on { ok => …, break(bi) => …,
limit => … }` **outcome-arm** form DIVERGES from the reference. A bare `break;` self-hosts correctly, so
the gap is specifically the outcome-arm lowering and its index binding. Bounded and well-scoped.

## Also delivered — two more boundary pins

`eq/array_of_array` and `eq/enum_tuple_payload` were measured supported but UNGUARDED, and are now
pinned. Boundary **65 -> 67 Ok** (2 Gap / 1 RefRejects unchanged). No product code.

Their comment records a non-obvious **asymmetry**: array-of-array is supported but array-of-array inside
a STRUCT is not, and an enum TUPLE payload is supported while an enum ARRAY payload is not. Neither
generalizes to its enclosing-composite form, so support must not be inferred by analogy — probe it.

## Verification

- Every status claim above was established by differential probe against the reference, each run with a
  known-Gap CONTROL (`float_arith`) to prove the probe discriminates. A probe without a control is not
  evidence: `self_host_compile` builds on `compile_src` and replaces chunk bodies, so a skipped
  replacement would report identity trivially.
- The `for … on` divergence was isolated carefully: the first three attempts at the source were rejected
  by the REFERENCE (bad syntax on my part — the language has no `let mut`, and a `for` needs `limit`), a
  reference rejection is NOT a self-host gap, and the difference matters. Valid syntax was taken from
  `tests/for_limit.rs`. Bare `break;` then came out IDENTICAL and only the outcome-arm form diverged.
- Boundary test green at 67 Ok; full `scripts/release-gate.sh` result recorded in the commit message.

## Next step — IMPLEMENT `struct { t: (P, Word) }` (already selected; do not re-ask)

The selection is MADE, by the loop's own ordering policy (context first, then priority), and the
diagnosis is captured in [`../decisions/STRUCT_TUPLE_OF_STRUCT_PLAN.md`](../decisions/STRUCT_TUPLE_OF_STRUCT_PLAN.md).
Do not reopen the choice — `AUTONOMOUS_IMPLEMENTATION_LOOP.md` forbids prompting the operator to
order bounded roadmap tasks, and this session already violated that once.

**The construct is currently MIS-COMPILED, not merely unsupported.** The admission admits it and the
drain compares a struct element as if it were a scalar (`GetTupleField(Flat { kind: Unit })` + `CmpEq`
where the reference extracts `FlatNested { variant: Struct }`, allocates a temp pair, and recurses).
Measured: 44 self-hosted ops vs 59 reference, `local_count` 6 vs 8. That makes this a correctness fix
with a boundary movement, not just a coverage increment.

Stage split (from the plan doc):
1. **parse.kel — expected SMALL.** Mirror the sibling `sd_fstruct` branch (~2597) into the
   `se_subistuple` sub-field drain (~2593): when `tup_estruct[fidx] > 0`, emit the sentinel header
   `(tup_eoffset + (100 + sd_bytesize) * 65536)`, allocate `r2` then `l2` monotonically, and push a
   frame. The existing `se_stk_*` machinery already fits — the element IS a struct, so its sub-fields
   read `sd_*`.
2. **reconstruct.kel — probably NOTHING.** The recursive `seb` grammar already nests at any depth.
   Verify the depth-1/2 fixtures stay byte-identical before assuming work is needed.
3. **codegen.kel — the real change.** The `es_*` emitter hardcodes `getfield`; the extract of the
   struct element out of its parent TUPLE must be `GetTupleField` while the extract inside `P` stays
   `GetField`. Each emit frame needs an ACCESSOR VARIANT chosen by the parent container's kind. This
   same machinery is what array-of-tuple-of-struct and the mixed-subtree gaps will need.
4. **Admission.** Widen `struct_eq_kind`'s tuple branch to consult `tup_estruct` and require
   `struct_subtree_pure`, so a deeper or mixed element defers instead of being mis-lowered.

Fixtures that must go from DIVERGE to IDENTICAL: `(P, Word)` (59 ops, `local_count` 8), `(P, P)`
(74 ops), and `(P, Word), w: Word` (69 ops).

## Standing method note

PROBE BEFORE PLANNING, and always with a control (point the probe at `scope/float_arith__GAP` and
confirm it reports DIVERGE; also confirm the REFERENCE accepts the source, since a reference rejection
is not a self-host gap). Stale planning docs were this session's recurring theme: the boundary count,
the tuple-in-tuple premise, four Order-1 residuals, and the loop doc's own task queue were all stale,
**all understating what had landed**. Treat any recorded status claim as a lead, not a fact.
