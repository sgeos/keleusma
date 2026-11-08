# BRIEF — the indexed composite data slot

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-11.

## What is refused, and why it was

`private data log { items: [F; 3], count: Word }` with `log.items[i]` is refused:

> an INDEXED composite data slot: every element carries its own pool entry, and the stride is not
> proven uniform across the range here

That refusal was correct when written. The direct case had just been implemented, and extrapolating
`base + index * size` from it would have assumed a uniform stride that nothing had checked.

## What makes it implementable now

`src/compiler.rs` assigns an array-of-composite field **one pool entry per element slot**, in a single
loop, each advancing the running total by the same `body` — so the entries for one field are
consecutive and uniformly spaced **by construction**. That is a fact about the producer, not a
hypothesis, and it is checkable in the table this backend already reads.

**Checkable, therefore checked.** Uniformity is validated across the declared element range rather
than assumed from the shape of the loop that built it, because a validated stride is what separates
this from the extrapolation that was refused.

## The wrong turns

1. **Do not re-derive the bound.** `Op::GetDataIndexed`/`SetDataIndexed` carry the declared element
   count, and the arm already emits an unsigned bounds check against it before the shared/private
   split. A second bound computed here could disagree with the one actually enforced.
2. **Do not forget the initialisation word.** Each element slot has its own entry in the pool table
   and therefore its own flag. The flag index moves with the element index; using the base slot's
   flag would let one written element make every sibling read as written.
3. **Do not validate only the first pair of entries.** Two consecutive offsets differing by `size`
   proves nothing about the third. Walk the whole declared range.
4. **Do not accept a range the table does not fully cover.** A missing element entry means the
   compiler did not place it, and computing its address from its neighbours invents a location.
5. **Do not let the element size come from anywhere but the same derivation the direct case uses.**
   Two computations of one size are free to drift.

## What done looks like

An indexed composite slot lowers and agrees with the reference for reads and writes across several
elements; an unwritten element still faults; a table that does not partition its range uniformly is
refused; and the refusal that remains says which property failed.
