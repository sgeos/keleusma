# Design Brief: Encoding-Space Capacity for the Self-Hosted Pipeline

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: OPEN — operator decision requested. Prepared 2026-07-22 in response to
process-audit worklist item 6. This is a brief, not a change; nothing here is
implemented. It lays out the capacity constraints, the options with tradeoffs, and a
recommendation, and stops for the operator.

## The problem

The self-hosted compiler pipeline (`lexer → parse → reconstruct → codegen`, in
`compiler/kel/`) communicates between stages with packed integer words. Three of those
encodings, plus the operator precedence scale, are now **at capacity**, and the most
recent increments have been progressively more intricate workarounds. The remaining
full-language work — the nested-composite-equality frontier (enum-in-struct,
tuple-of-struct, 2+-level) and future operators — needs clean capacity to land as
oracle-checked increments rather than reuse tricks.

### The four constrained spaces (verified 2026-07-22)

| Space | Encoding | Range | State |
|-------|----------|-------|-------|
| Token (lexer → parse) | `tok + payload*64` | `tok` 0..61 (62/63 = EOF/PENDING sentinels) | **Full** — all 0..61 assigned |
| Record kind (parse → reconstruct) | `kind + arg*64` | `kind` 1..63 | **Full** — all assigned in the `Node` enum |
| Wire op (codegen → driver) | `op + operand*64` | `op` 1..63 | **Full** — all 1..63 assigned |
| Operator precedence | small integers (Orelse 1 … unary 10) | — | **Too coarse** to match the reference's logical binding powers |

The three packed spaces share the same root: a **6-bit (`*64`) low field**. Note this
is the encoding of the **intermediate inter-stage streams**, NOT the final `Module`
bytecode (which is a separate encoding in `src/wire_format.rs` and is not at capacity).

### The workarounds already in use (and their cost)

- **Token space:** eager `and`/`or` sidestep it via the *ident-by-id* pattern (lexed as
  identifiers, recognized by interned id in operator position). Works for keyword
  operators; does **not** help a new punctuation operator.
- **Record/node-kind space:** node kinds live in an un-packed forest array and may exceed
  63 (64–68 are in use). New parse *records* reuse a value that is only a *node* kind
  (the "split record/node-kind" pattern; e.g. `bnot` yields record 48 = StructEq's node
  value). Works, but reusing a node-kind value as a record kind is a latent footgun — a
  reader cannot tell from the value which namespace applies.
- **Wire-op space:** the planned `GetTupleField(FlatNested)` for tuple-of-struct will
  reuse op 53 with a nested *operand form*, distinguished in the driver by operand
  magnitude. Works, but overloads one op tag with two operand shapes.
- **Precedence:** `xor` is mapped to `NotEq` (comparison level) and `and`/`or` to coarse
  values, producing two documented faithfulness defects (`a xor b == c`, `a and b xor c`
  diverge from the reference; pinned as Gaps in the boundary test).

Each workaround is individually sound and tested, but collectively they are accumulating
fragility, and the record-kind namespace overload in particular is the kind of thing that
caused the recent subproject-decoder drift.

## Options

### For the three packed spaces

**A. Widen the low field 6→7 bits (`*64` → `*128`).** Doubles token/record/op capacity to
0..127. This is an intermediate-representation change only (not a bytecode change): every
producer that packs (`... * 64`, the `wire.radix` constant in `codegen.kel`, the lexer
token emit, the parse record yields) and every consumer that unpacks (`w % 64`, `w / 64`
in the drivers' `decode_op` and reconstruct) must change to 128 in lockstep. The payloads
are already large multiplications and fit (words are full-width in the self-host's 32-bit+
floor). Retires the split-kind and operand-overload tricks.
- *Pro:* clean, uniform, ample headroom; removes the namespace-overload footguns.
- *Con:* a coordinated differential-oracle change touching all stages and both drivers;
  requires a full re-baseline of the byte-identity corpus and the boundary test; must be
  landed as one reviewable, fully-re-verified step. Does nothing for precedence.

**B. Escape/extended-kind codes.** Reserve one value in each space as an escape meaning
"the real kind/op is in the next word (or the high bits)." Localizes the change to the
overflow cases only.
- *Pro:* minimal disruption to existing encodings; pay-as-you-go.
- *Con:* two-word decode paths add complexity exactly where clarity matters; the drivers
  and reconstruct grow special cases; still leaves the base space full.

**C. Continue the current reuse patterns (status quo).** ident-by-id for keyword
operators, split record/node kinds, operand-form reuse for ops.
- *Pro:* zero infrastructure change; each next increment is "just" more of the same.
- *Con:* increasingly intricate and error-prone; the record-kind overload is a latent
  drift hazard; punctuation operators remain impossible; does not scale to the full
  language.

### For the precedence scale

**P1. Renumber the self-host precedence scale to match the reference's ordering.** Give the
logical operators (`orelse < andalso < or < xor < and`, all below comparison) distinct
values with room, fixing the two faithfulness defects.
- *Pro:* closes the only in-set correctness gap; makes `xor`/`and`/`or` fully faithful.
- *Con:* renumbering changes operator folding for *existing* operators, so it breaks
  byte-identity broadly and requires a full re-baseline — a coordinated change guarded by
  the whole self-compile corpus and the boundary test, not a local edit.

**P2. Accept the defects (status quo), documented.** The two divergent cases are pinned as
Gaps in `self_hosted_construct_support_boundary`.
- *Pro:* no work, no risk.
- *Con:* the self-host is knowingly unfaithful to the reference for those groupings.

## Recommendation

If the roadmap will keep widening toward full self-hosting (it will), I recommend
**Option A** for the packed spaces: the reuse tricks are accumulating fragility that will
keep producing drift-class hazards, and A is the clean prerequisite that turns the
remaining nested-equality and operator work into ordinary oracle-checked increments. I
recommend treating **P1** as a separate, self-contained coordinated change (it is
independent of A). Both are differential-oracle changes and must be landed as single,
fully-re-verified steps against the byte-identity corpus and the boundary test — which is
exactly the machinery that makes them safe to attempt.

If the roadmap is near-term-bounded and only a few more constructs are expected, **Option
B** (escape codes) or continued **C** may be proportionate, and **P2** (documented defects)
is defensible.

## Decision requested

1. For the packed token/record/op spaces: **A (widen to 7 bits)**, **B (escape codes)**, or
   **C (continue reuse patterns)**?
2. For precedence: **P1 (renumber to match the reference)** or **P2 (accept documented
   defects)**?
3. Sequencing relative to the tuple-of-struct / nested-equality work in flight: land the
   capacity change first (so the nested work is clean), or continue the nested work on the
   current workarounds and revisit capacity later?

Per `PROCESS_STRATEGY.md`, this is past the autonomy boundary — it changes semantics and
carries significant tradeoffs — so no option will be implemented without the operator's
decision.
