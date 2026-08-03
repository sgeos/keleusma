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

## Next step — the TYPE CHECKER (the only unblocked Order-1 item), plus one operator decision

All three Order-1 remainders were probed on 2026-08-03 and the picture changed:

**Wire-format serialization — PARTIALLY BLOCKED, needs an operator decision.** The roadmap listed it
as "framing header, operand-pool encoding, parity, CRC trailer", which omits the dominant cost: the
AUXILIARY BODY is `rkyv`-archived and carries everything except the opcode stream and operand pool.
Reproducing rkyv's zero-copy layout byte-for-byte in Keleusma is disproportionate and fragile. Full
self-hosting of the artifact therefore needs a decision only the operator can make — reimplement
rkyv, or CHANGE the aux-body encoding, which is a wire-format change and so a `BYTECODE_VERSION`
question, an enumerated stop. The bounded non-rkyv slices (CRC-32, the opcode stream and operand
pool, the framing header) can proceed without that decision but leave the aux body host-supplied, so
they do NOT meet the gate's "no Rust scaffold borrow" wording. Scoping in
[`../decisions/WIRE_FORMAT_SELFHOST_PLAN.md`](../decisions/WIRE_FORMAT_SELFHOST_PLAN.md).

**The monomorphizer — VACUOUS over the subset.** The `.kel` sources use no generics (the `impl`/
`trait` hits in parse.kel are its own parser code for those keywords). Monomorphization is identity
here, which is why the pipeline omits the pass and still matches the reference byte-for-byte. Porting
it would tick the box without changing any output; its real cost arrives only with full-language
generics (Workstream F). Do not pick it as "the cheapest" expecting value.

**The type checker — UNBLOCKED, and the real work.** The self-hosted pipeline has NO type checking at
all: its stages are lexer, parse, reconstruct, codegen (plus analyze and the verify_* family), and
nothing validates types. Ill-typed programs are caught today only by the CLI's cross-check against the
reference. Self-hosting it is what makes the self-hosted compiler able to reject bad programs on its
own, and over the monomorphic Word/Byte subset it is far smaller than `typecheck.rs`'s 8601 lines.

CAUTION on probing this one: `self_host_compile` calls `compile_src` FIRST, so it panics whenever the
reference rejects. A naive "does the self-hosted path reject?" probe is CONFOUNDED and will report
rejection for every ill-typed program regardless. Probe the stages directly, or reason from the stage
list, rather than through that harness.

## Standing method notes

The thirteen rules are consolidated in [HANDOFF.md](./HANDOFF.md). Add one from this probe: **a probe
run through a harness that already invokes the reference cannot tell you what the self-hosted path
does on its own** — check what the harness does before trusting its verdict, exactly as the control
discipline requires for the byte-identity probes.
