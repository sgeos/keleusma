# Plan — bare `for` support in the self-hosted pipeline

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Written 2026-08-25 from a read of the three stage sources. It exists so the next
session executes rather than re-derives: the previous cost was a third too high
because it was inferred from the shape of the problem rather than the state of
the tree.

## What already exists, and what does not

Pinned by `the_bare_lowering_exists_in_codegen_and_is_unreached_by_the_earlier_stages`,
which **fails when this work starts** — that is the pin doing its job.

| stage | state |
|---|---|
| `codegen.kel` | **DONE.** `push_forin` (kind 16) emits the whole lowering from a seven-word `for_parts` entry |
| the Rust driver | **DONE.** Reads `for_parts` out of the reconstructed body |
| `reconstruct.kel` | `for_parts` declared, **zero writes**. No build handler |
| `parse.kel` | **nothing**: no node kind, no parts, no header path |

## The contract `push_forin` reads, position for position

```
for_parts[fp + 0]  i_slot     the loop variable's frame slot
for_parts[fp + 1]  lim_slot   the limit's frame slot
for_parts[fp + 2]  start      node index: the low bound expression
for_parts[fp + 3]  limit      node index: the high bound expression
for_parts[fp + 4]  cond       node index: `i >= limit`
for_parts[fp + 5]  body       node index: the loop body block
for_parts[fp + 6]  incr       node index: `i + 1`
```

`st.let_count` grows by **two**, not five. The continuation rides in `rhs[p]`,
and `args[p]` is the entry start — the same shape `ForLimit` uses.

## Where `cond` and `incr` come from, which is the only real design question

They are **synthetic**: no token in the source corresponds to `i >= limit` or
`i + 1`. Two options, and the second is recommended.

**A. `parse.kel` emits them as records.** Mirrors how it emits `ForLimit`'s four
literal nodes. Costs four more emission steps in the `for_emit` ladder and keeps
`reconstruct.kel` a pure assembler.

**B. `reconstruct.kel` synthesises them in the build handler.** It already has
`emit(kind, arg, lhs, rhs)`, which appends a node and pushes its index — every
node in that stage is built through it. **`reconstruct.kel` does not currently
synthesise any node that did not arrive as a record**, so this is a new
capability for that stage, but a small one, and it keeps the wire stream
narrower.

**Recommended: B**, because the synthetic nodes are a property of the LOWERING,
not of the source, and the stage that owns the lowering's shape should build
them. A also works and is defensible; what must not happen is half of each.

## The three edits

**1. `parse.kel` — the header path.** Phase 4 currently raises
`pe_bare_for()` when it sees `{`. That refusal becomes the *entry* to the bare
path: allocate the two slots, set the phase to the body phase, and mark the
loop-context entry as bare so `step_for_emit` takes the short ladder. **The
refusal is not deleted until the path is complete** — an unfinished bare path
that silently mis-parses is strictly worse than the named refusal it replaces.

**2. `parse.kel` — the emission ladder.** A bare analogue of `step_for_emit`:
the two `SlotRecord`s, then a new `ForInBuild` signal, then the statement record
with a new `ForIn` node kind.

**APPEND, DO NOT RENUMBER** — the kinds are matched by value in
`reconstruct.kel`. `Node` runs to `ArrayOfEnumEq = 64`, and record kinds up to
**68** are dispatched, so the next free pair is **69 and 70**.

**AND A KIND AT OR ABOVE 64 MUST USE THE MIGRATED EMIT PATH.** The record
transport is a hybrid: a legacy site yields one word `code + val*64`, which caps
the tag at six bits, while a migrated site sets `ps.emit_arg` to a full-word
payload and yields a RAW tag. Kinds 65, 67 and 68 arrive that way today, and 69
and 70 must too. `step_bnot` is the smallest worked example.

**The driver's own doc comment described the PRE-Option-E transport until
2026-08-25** — "today each record is one yielded word `code + val*64`; the P11
Option E change ... lands here and nowhere else" — so an implementer reading the
first thing they would read concludes the tag caps at 63 and goes renumbering.
Corrected in this increment, and it is the reason this paragraph exists.

**3. `reconstruct.kel` — the build handler.** Pop `start`, `limit`, `body`;
read the two slots from `rs.pending`; synthesise `cond` and `incr`; write the
seven words; emit the statement node folding the continuation, exactly as
`k == 23` does for `ForLimit`.

## The specific wrong turns

**Do not renumber the existing node kinds.** They are matched by value across
two stages and pinned by tests. Append.

**Do not reuse `ForBuild`.** It pops seven nodes and writes twelve words. A
shared signal with two shapes is how a stage comes to read the wrong arity from
a correct stream.

**Do not delete the refusal before the path works end to end.** A named refusal
is a better artefact than a partial implementation, and this is the increment
where that trade is live.

**Do not trust the four codegen-only cases as coverage.** They drive the
REFERENCE parser. They will keep passing while `parse.kel` is wrong, because
they never call it — that is precisely how this gap survived. **The evidence
that the work is done is `ctrl/for_bare` moving from `Refuses` to `SOk` in the
construct-support boundary**, which drives the whole pipeline.

**Retire the pins rather than deleting them.** The gap pin, the boundary case's
verdict pin, and the refusal-message test all have subjects that change when
this lands. Each should say what became of what it watched — three sibling pins
were retired that way when the refusal landed, and one was moved from absence to
verdict when the boundary case landed.

## How to know it is finished

`ctrl/for_bare` classifies `SOk`; the boundary reads 91 SOk / 1 Refuses / 3
Diverges / 1 RefRejects; and `wire.kel` reaches the byte-identity corpus, which
is the point of the whole exercise and the thing the counted-form-only pipeline
has never been able to do.
