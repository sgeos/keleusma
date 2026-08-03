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

## Next step — ARRAY and ENUM at depth (the rest of the mixed-subtree problem)

Tuples at depth are done (boundary 75 Ok / 4 Gap / 1 RefRejects). The same frame machinery now needs
the other two composite kinds, in the same three dispatch sites (nested struct, tuple element, array
element):

1. **An ARRAY sub-field at depth** (`struct I { xs: [Word;2] }` inside `struct S { i: I }`) — measured
   DIVERGE, currently deferring. The sentinel convention is already established: 40000+size is what
   the enum-payload records use for an array, mirroring the 30000+size just added for tuples.
2. **An ENUM sub-field at depth** (`struct I { e: E }` inside `struct S { i: I }`) — also deferring.
   Enums need variant dispatch, so this is the larger of the two.
3. A struct FIELD that is an array-of-tuple (`struct S { g: [(P, Word); 2] }`). NOTE this one is NOT
   drain work: the element layout is never recorded, because `parray_tuple` is parameter-only and a
   struct field's array element type goes through `field_size_and_kind`, which takes only an
   identifier. It needs a new layout table plus scanner work — a genuine context switch, which is why
   it was re-ordered behind the drain items.
4. The `[bool;2]`-shaped struct field array (element type not a recognized scalar).

Beyond this family the Order-1 gate needs the type checker, the monomorphizer, and wire-format
serialization. Do NOT prompt the operator to order any of this.

## Standing method notes

1. **PROBE BEFORE PLANNING, always with a control**, and **probe what the admission ACCEPTS**.
2. **When generalizing a drain, tighten its admission in the SAME change.**
3. **Close an admission hole BEFORE building support over it**; expect +Gap / 0 Ok when you do.
4. **When a "regression" appears, measure the pre-change behaviour before assuming authorship.**
5. **Never trust op counts or lengths as a correctness proxy.** Assert byte-identity; for a deferral,
   assert its SHAPE.
6. **When a change that should be sufficient produces NO observable difference, suspect a path that
   BYPASSES the code you changed.**
7. **Abandon on TRAJECTORY, not attempt count.** Keep going while the divergence narrows and green
   fixtures stay green.
8. **Admission helpers call each other, and R4 forbids cycles.** Relaxing one predicate by calling
   another can make the whole stage unverifiable ("recursive call detected during WCMU topological
   sort"). Inline instead — the codebase already does this in several places.
9. **A self-compile failure in stage B can be caused by stage A merely GROWING.** `LoopLimitExceeded`
   in an UNCHANGED reconstruct.kel meant a parse.kel block had crossed a per-block cap. Factor the new
   code into a helper rather than raising a limit; the error names neither the loop nor the function,
   so ask "what did I just make bigger?".
10. **When a construct becomes supported, RETARGET the Gap fixture that pinned it** rather than
    deleting it, or the deferral it guarded silently stops being tested.
