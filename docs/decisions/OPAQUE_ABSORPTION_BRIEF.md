# Brief — absorb the riskiest change of the session, then gate the result

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-02.**

---

## The goal set

| goal | state |
|---|---|
| **G16** absorption 46 — `Opaque` sized by the address width | **unblocked; the highest-risk absorption this session** |
| **G17** run the actual release gate on the absorbed tree | unblocked, but the `v0.2.3` gate is running; sequence around it |
| `f16` | no oracle |
| `Text<N>` | still the other line's |
| publication | held |

## G16: why this one is different from the last three

Absorptions 43, 44 and 45 touched one to six `src/` files and no corpus. **This one touches eleven
`src/` files with 606 insertions**, including `value_layout.rs`, `verify_typed.rs`, `vm.rs` and
`wire_schema.rs`.

**`value_layout.rs` is the canonical flat layout this backend validates every compiler-baked offset
against.** A change there is the one kind of `src/` change that can move a figure here without
touching a single line of `native_codegen`.

It also closes an item that was **blocked on the other line all session** — `Opaque` sized by
`addr_bits_log2` rather than by the word width.

## The prediction, with its falsifier

**Predicted: zero movement.** `native_codegen` stays at **459 passed, 0 failed, 88 binaries** under
default features and under `narrow-float-32`.

**The reasoning, so it can be wrong for a nameable reason**: this backend **refuses `Opaque` at every
route it can reach**. The shared-slot resolver reports *"Opaque slot; host handles are Workstream D"*,
so no lowered path carries an opaque value and a change to its size cannot reach one.

**Falsifier, named in advance**: any test asserting a composite offset, a flat-layout size, or a
verifier-derived footprint. Those consume `value_layout` directly rather than through a lowered path,
so the refusal does not protect them. Specifically at risk are the flat-composite offset tests, the
narrow-composite tests, and anything reading `RuntimeFootprint`.

**If a figure moves, report the movement and name the test rather than adjusting the figure.**

## G17: the gate, and the two steps never run on this tree

Every claim this session has been careful to say **"this is not a gate pass."** Running it would make
it one, and the back-merge plan makes that worth doing: this line merges into `v0.2.3` before
publication, so the gate that matters is the one on this tree.

**Two steps have never been run here**, and they are the plausible failures:

- `cargo clippy --workspace --all-targets -- -D warnings` — only `native_codegen` clippy has been run
  this session.
- `cargo doc` at the **docs.rs feature sets** — only `native_codegen`'s doc build has been run.

**A broken intra-doc link is invisible to `test` and `clippy` alike**, and this project's own record
notes that skipping the doc build is how a release shipped with a red documentation job.

## The wrong turns

**1. Do not run the gate while the other line's gate runs.** Warn first, as they have warned me four
times. Two full gates contend, and the perf canary's contended reading sits three per cent from its
own regression row.

**2. Measure the absorption ALONE, before the gate.** The gate changes nothing, but an absorption
measured concurrently with anything is a conflation this line has recorded three times.

**3. Gate the FINAL tree.** Absorb first, then gate — a gate on an intermediate tree describes a tree
that will not exist.

**4. Do not call a partial run a gate pass**, which has been the standing discipline all session and
does not relax because the remaining steps are the small ones.

**5. Read the tool's status, not the pipeline's**, in both directions.
