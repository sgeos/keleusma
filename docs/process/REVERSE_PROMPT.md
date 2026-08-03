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

## Next step — ORDER 1 (the drain family is closed and the last drain item was PROBED and deferred)

**Probed 2026-08-03, with a control**: an enum with a COMPOSITE payload is NOT a contained extension
of the enum block just landed. A struct payload fails at DEPTH 1 as well (`struct S { e: E, w: Word }`
with `enum E { A(Q), B }` -> defers, 4 ops against 90), so the gap is in the NESTED ENUM EMITTER
generally, not in the new block. Array and tuple payloads fail even at TOP level. Supporting them
means new payload plumbing across parse, reconstruct, and codegen — mirroring what
`push_enum_struct_payload_loop` does for the top-level enum-eq. That is a full increment, not a tail.

So the drain family is closed for now, and **Order 1 is the direction**. Its three remainders are the
whole of what stands between here and the Order-1 gate:

1. **Wire-format serialization** — self-contained and well-specified: the framing header, operand-pool
   encoding, parity, and CRC trailer are all host-side today (no `.kel` stage references `to_bytes`,
   parity, or CRC). Probably the best value per token, and it has no interaction with the equality
   machinery, so the large regression surface of the last six increments does not apply.
2. **The monomorphizer** — near-identity over the subset, likely the cheapest of the three.
3. **The type checker** — largest and highest-risk (Hindley-Milner is not a streaming shape).

Deferred in this family, for whenever it is picked up again:
- Enum with a composite payload (struct, array, or tuple) — needs the payload plumbing above.
- A struct FIELD that is an array-of-tuple, and the `[bool;2]`-shaped struct field array — these share
  ONE root cause: a struct field's array element type goes through `field_size_and_kind`, which
  accepts only an identifier, so the element layout is never recorded. One scanner fix closes both.
- An array whose ELEMENT is itself composite at depth.

## Standing method notes

The thirteen rules are consolidated in [HANDOFF.md](./HANDOFF.md); they are not repeated here. The
three most expensive lessons of this arc, in short: probe what the admission ACCEPTS (both silent
mis-compiles were in accepted constructs); tighten admission in the same change as any drain
generalization (otherwise a shallow silent bug becomes a deeper one); and never land on targeted tests
alone (clippy `-D warnings` and `EXPECTED_SELF_COMPILE` fire only in the full gate).
