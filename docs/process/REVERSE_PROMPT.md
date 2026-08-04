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

## Next step — wire format v2, stage 2: the accessor layer

Stage 1 (the flat aux-body codec) is merged and green. The staging is in
[`../decisions/WIRE_FORMAT_V2_FLAT_AUX.md`](../decisions/WIRE_FORMAT_V2_FLAT_AUX.md):

2. **The accessor layer** replacing the `Archived*` read surface, with the same API shape the VM uses
   so the call sites change minimally. This is where the IN-PLACE READ property is either preserved or
   lost — it is the point of the whole design, so no `Vec` materialization on the load path.
3. **Cut the VM and loader over**, delete the rkyv path, bump `BYTECODE_VERSION` to 2. This is where
   `CLAUDE.md`'s no-public-adoption policy text must change: it currently says the number stays at 1.
4. **Drop the `rkyv` dependency** if nothing else needs it, and update `Cargo.toml`, the tech-stack
   list, and `docs/spec/WIRE_FORMAT.md`.
5. **Self-host the emitter** in Keleusma — the original goal. NOTE R4 forbids recursion, so the `.kel`
   encoder must walk nested constants with an explicit stack, as the equality drains do.

Also open, unrelated: the composite-equality family's remaining Gaps (enum composite payload; the
struct-field array element scanner, which closes two Gaps at once), and the other Order-1 remainders
(the monomorphizer is vacuous over the subset; the type checker is the substantive one).

## Standing method notes

The fourteen rules are consolidated in [HANDOFF.md](./HANDOFF.md). Two reinforced this increment:
**never land on targeted tests alone** — the full gate caught both a warning introduced here and a
pre-existing doc defect that CI could not see; and **when a documented command and the gate's command
disagree, run the stricter one**, because the gap between them is where defects live.
