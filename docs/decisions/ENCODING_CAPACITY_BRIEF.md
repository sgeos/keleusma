# Design Brief: Encoding-Space Capacity for the Self-Hosted Pipeline

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: OPEN — operator decision requested. Prepared 2026-07-22 in response to
process-audit worklist item 6; **revised 2026-07-23** with the total-word-budget finding
(the operator asked whether 8-bit is viable — the answer surfaced that the payloads, not
the tag field, are the binding constraint). This is a brief, not a change; nothing here is
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

### The deeper wall: total word budget, not just the tag field (verified 2026-07-23)

Widening the low field is more constrained than a "the 6-bit tag has room to grow"
framing suggests, because the tag shares one signed 64-bit `Word` with its payload, and
the payload is itself already over-packed. The fattest inter-stage record, the
array-of-composite equality builder (`parse.kel:2457`, `ArrayOfEnumEqBuild`), packs seven
sub-fields whose top field (`arrsize`, an 8-bit count at bits 48 through 55, per
`reconstruct.kel:329`) puts the payload alone at 56 bits. Each word is `tag + payload *
radix`, so the used width is `payload_bits + radix_bits`, and the signed ceiling is 63
bits. The measured worst case:

| Low-field radix | Worst-case word | Fits `i64`? | Spare bits below 2^63 |
|-----------------|-----------------|-------------|-----------------------|
| `*64` (6-bit, current) | ~2^62 | yes | 1 |
| `*128` (7-bit) | 2^63 − 1 = `i64::MAX` | yes, exactly | 0 (zero margin) |
| `*256` (8-bit) | ~2^64 | **no — overflows** | — |

The current scheme therefore already sits one bit below the `i64` ceiling. Widening to
7 bits lands the worst case exactly on `i64::MAX`, leaving no margin for any new field or
larger value; widening to 8 bits overflows a signed `Word` outright. The governing
constraint is `payload_bits + radix_bits <= 63`, and the payloads are already near the
limit. This corrects the "ample headroom" characterization of Option A below: the tag
field is not the binding constraint — the total word budget is.

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
- *Pro:* clean, uniform, byte-friendly at 8 bits (decode is `word & 0xFF` / `word >> 8`);
  removes the namespace-overload footguns.
- *Con:* **little to no word-budget headroom** (see the total-word-budget table above). At
  the current worst-case payload, a 7-bit radix lands exactly on `i64::MAX` (zero margin)
  and an 8-bit radix overflows a signed `Word`. Widening the tag therefore is **not** a
  standalone change: it requires FIRST shrinking the fattest payloads (the
  array-of-composite equality builders) below 56 bits — see Option D. It is also a
  coordinated differential-oracle change touching all stages and both drivers, requiring a
  full re-baseline of the byte-identity corpus and the boundary test as one reviewable
  step, and it does nothing for precedence.

**D. De-pack the fat payloads (multi-word records or a side table), then widen.** The array-
of-composite equality builders cram seven sub-fields into a single word; carrying those
fields across two words (or in an auxiliary descriptor table keyed by record index) drops
the worst-case payload well below 56 bits and removes the single-word `i64` ceiling that is
the *actual* wall. Once the payloads are de-packed, an 8-bit (byte-aligned) radix becomes
safe with comfortable margin.
- *Pro:* attacks the real constraint (total word budget), not just the tag namespace;
  makes a subsequent clean 8-bit widening trivial; the multi-word/side-table shape is what
  the reference oracle already uses for its auxiliary descriptor tables.
- *Con:* the largest single change of the four; touches the record encode/decode on both
  the `parse -> reconstruct` and driver sides; must land as one re-baselined oracle step.
  Best sequenced *before* any radix widening, not after.

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

If the roadmap will keep widening toward full self-hosting (it will), the packed spaces
need real capacity, and the word-budget finding above changes the sequencing. A bare radix
widening is **not** safe on its own: 7 bits is zero-margin and 8 bits overflows. The sound
path is **D then A**: first de-pack the fattest payloads so the worst-case payload drops
well below 56 bits, then widen the radix to a byte-aligned **8 bits** with comfortable
margin. Widening straight to 8 bits without D would corrupt the fat records; widening to
7 bits without D buys a doubled tag namespace at the cost of the last spare bit, so it is
possible but fragile and I do not recommend it. I recommend treating **P1** as a separate,
self-contained coordinated change (it is independent of the packed-space work). All of
these are differential-oracle changes and must be landed as single, fully-re-verified steps
against the byte-identity corpus and the boundary test — which is exactly the machinery
that makes them safe to attempt.

If the roadmap is near-term-bounded and only a few more constructs are expected, **Option
B** (escape codes) or continued **C** may be proportionate, and **P2** (documented defects)
is defensible.

## Decision requested

1. For the packed token/record/op spaces: **D then A (de-pack the fat payloads, then widen
   to a byte-aligned 8 bits)** — the recommended path — or **B (escape codes)**, or **C
   (continue reuse patterns)**? A bare **A at 7 bits** is available but zero-margin and not
   recommended; a bare **A at 8 bits** is unsafe without D (it overflows the signed word).
2. For precedence: **P1 (renumber to match the reference)** or **P2 (accept documented
   defects)**?
3. Sequencing relative to the tuple-of-struct / nested-equality work in flight: land the
   capacity change first (so the nested work is clean), or continue the nested work on the
   current workarounds and revisit capacity later?

Per `PROCESS_STRATEGY.md`, this is past the autonomy boundary — it changes semantics and
carries significant tradeoffs — so no option will be implemented without the operator's
decision.
