# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning and frontier assessments live in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-07-31 (session 36)

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

## Next step — the strategic fork is now sharp, and it needs an operator decision

The cheap work is done. What remains splits cleanly:

1. **Finish Order 1** — the type checker, the monomorphizer, and wire-format serialization. This is the
   gate for everything downstream (the validator, trap analysis, and runtime all sit behind it). The type
   checker is a multi-session effort against the seven-day window; the monomorphizer is near-identity
   over the subset and is probably the cheapest of the three; wire-format serialization is
   well-specified and self-contained, and may be the best value per token.
2. **The `for … on` outcome-arm gap** — bounded, same shape as the increments that have been landing.
3. **Continue subset-widening** — the remaining composite-nesting gaps (deeper array/enum, mixed
   subtrees involving array/enum). Formally Workstream F, whose gate depends on Order 1.

Recommendation: **wire-format serialization or the monomorphizer**, because they close Order 1 rather
than widen a subset whose gate is behind Order 1 anyway. But this is the operator's call — the three
differ by an order of magnitude in cost.

## Standing method note

PROBE BEFORE PLANNING, and always with a control. Three separate stale claims surfaced in one day (the
boundary count, the tuple-in-tuple gap premise, and four Order-1 residuals), **all in the same
direction**: the documents understated what had landed and so pointed at work already done. A
conservative admission deferral is not evidence of a gap either — the path it defers to may already be
correct. Treat any unverified status claim in the roadmap as suspect until probed.
