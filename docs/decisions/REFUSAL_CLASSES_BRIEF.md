# BRIEF — the refusal channel is one variant doing four jobs

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Why this, now

`isa_lowering_census` reports **NAMED REFUSED (1): ["Len"]**, a figure this line has restated every
increment. It is built by `head(msg)` — **taking the leading alphanumeric run of a free-form English
sentence** and keeping it if it happens to match an ISA opcode name.

`LowerError::UnsupportedOp(String)` is documented as *"An opcode outside the currently supported
subset."* It is in fact constructed at 24+ sites carrying **four unrelated classes**:

| class | example message | leading token |
|---|---|---|
| genuinely unsupported opcode | `GetIndex reading {..} is not lowered` | `GetIndex` — an opcode, correct |
| a type the backend lacks | `chunk 0 has a Float in its signature` | `chunk` — not an opcode |
| **malformed input** | `Const({idx}) out of range` | **`Const` — a real opcode** |
| **internal invariant violation** | `Call(..) needs the whole module; lower_module resolves it, lower_chunk cannot` | **`Call` — a real opcode** |

The float message Displays as **"native lowering does not yet support opcode chunk 0 has a Float in
its signature"**. That sentence is not merely ugly: it is the visible end of a channel where *how a
refusal reads in English decides which opcode a census attributes it to*.

## The question to answer, and it is falsifiable

**Can a refusal that is not about an unsupported opcode land in the NAMED REFUSED column?**

The `isa.contains(head)` guard excludes `chunk` and `native`. It does **not** exclude `Const`, `Call`,
or `NewComposite`. Whether the column is clean today because the tree is clean, or because the corpus
never fires those sites, is exactly the distinction recorded in
`a-clean-guard-proves-its-reach-first`. **Answer it by firing the site, not by reading the source.**

Note the two landing zones differ. An opcode the backend also lowers lands in the `both` bucket; one
it does not lowers lands in `refused_only` — the published figure. **Check what the census does with
`both`**; do not assume it asserts empty.

## Wrong turns to avoid

- **Do not fix the "chunk 0" sentence and stop.** The sentence is the symptom. The load-bearing defect
  is that a census parses English to attribute refusals.
- **Do not move a census figure to make it look better.** If separating the classes changes a number,
  the number was wrong before; report the move and say which reading was mistaken.
- **Do not widen the backend to fire a site.** Construct the malformed input directly, as the
  `Stream` blast-radius measurement did by mutating bytecode.
- **Do not conclude "no site is reachable" from a search.** The `IsStruct` precedent on this line is
  explicit: record the search, not the conclusion.
- **Do not treat class 4 as a diagnostic.** `InvalidIr` already exists for "always a defect in this
  crate rather than in the input". A consumer that cannot tell *"your program uses a feature I lack"*
  from *"I am broken"* cannot act on either.
- **Do not touch the read-only files.** This is entirely within `native_codegen/`.

## What good looks like

The class a refusal belongs to is carried by the **type**, not recoverable only from word order. The
census attributes an opcode because the error names one as data, or it does not attribute at all.
Every Display sentence is well-formed English about the condition it actually describes.
