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

## Next step — nested support for array elements (the frontier is now HONEST)

The admission holes are closed, so every unsupported construct in this family now rejects loudly.
That makes incremental work on nesting safe: a divergence is now attributable to the change under
test, not to a pre-existing silent bug.

The remaining work is unchanged in shape and blueprinted in
[`../decisions/ARRAY_OF_TUPLE_OF_STRUCT_PLAN.md`](../decisions/ARRAY_OF_TUPLE_OF_STRUCT_PLAN.md):
give the flat array family a nested form, or (preferred, and it would subsume array-of-deep-struct
and array-of-array-in-struct) route array-of-composite elements through the `StructEqNested` frame
machinery. Six Gaps now sit behind it, so the payoff is larger than when it was scoped.

Also open, same context: the impure-element subtree (the general mixed-subtree problem), enum array
payload, enum deep-struct payload, enum→struct→enum. Beyond this family the Order-1 gate needs the
type checker, the monomorphizer, and wire-format serialization. Do NOT prompt the operator to order
these — `AUTONOMOUS_IMPLEMENTATION_LOOP.md` settles it.

## Standing method notes

1. **PROBE BEFORE PLANNING, always with a control**, and **probe what the admission ACCEPTS**. Every
   silent mis-compile found so far was in a construct the admission accepted; a rejected construct
   fails loudly and is comparatively safe.
2. **When generalizing a drain, tighten its admission in the SAME change** — descending further
   without a guard converts a shallow silent bug into a deeper one.
3. **Close an admission hole BEFORE building support over it.** Otherwise every later increment must
   distinguish "my change broke this" from "this was already wrong". Expect the boundary count to
   move +Gap / 0 Ok when doing this; that is success, not regression.
4. **When a "regression" appears, measure the pre-change behaviour before assuming authorship.** The
   `[bool;2]` case looked like damage from the new guard and was in fact a pre-existing mis-compile
   the guard had exposed; reverting would have restored a silent bug.
5. **Do not trust op counts or lengths as a correctness proxy.** The worst case found diverged at an
   identical op count, differing only in content. Only byte-identity catches that.
