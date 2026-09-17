# BRIEF — generate composites, not just expressions

## Why this surface next

**The only genuine codegen defect this line has ever found was composite-return
aliasing**, and three of the ten recorded defects were composite-layout faults: a
slot storing a body ADDRESS, a slot read before write answering 0, a declared
initializer never applied. **Layout is where this backend has actually been
wrong.**

The generator has just paid for itself on scalars, finding a width gap no
enumerated cell could see. The packing follow-up used **five hand-written
subjects** — chosen by me, covering the shapes I thought of. That is the same
limitation the scalar matrices had, one level up.

## What varies, and why each axis matters

- **Field count and order.** A wrong width shifts every SUBSEQUENT field, so the
  defect appears in a neighbour. More fields means more neighbours and more
  offsets.
- **Field types mixed.** `Byte` beside `Word` is where a packed width is actually
  consumed; a uniform struct hides offset errors that cancel.
- **Field values from composed arithmetic**, including the newly reachable byte
  division, so the generated subjects reach what the fix opened up.
- **Every field read back**, because an unread field cannot report a shifted
  offset.

## Prior failures to avoid

- **Reading one field.** The five hand-written subjects already encode this
  lesson: a single-field struct returns the right answer under a wrong width
  because nothing follows it. **Read them all, and combine with distinct
  multipliers** so two fields swapping cannot cancel.
- **A probe implicating itself, six times so far.** Composite syntax must be taken
  from a form the tree already compiles, not assumed. `let mut` did not exist;
  neither does an implicit conversion between `Byte` and `Word`.
- **The trap hazard.** Byte bounds already derived apply unchanged; violating them
  kills the binary with `SIGTRAP` rather than failing a comparison.
- **Combining fields so that errors cancel.** Summing two fields hides a swap.
  Distinct powers of ten make position observable in the result.
- **Claiming this covers composites.** It covers flat structs of scalars built and
  read in one function. Nested composites, arrays, composite returns across calls,
  and composites crossing a yield are all out of scope and must be said to be.

## The measurement that decides whether it was worth it

**A green run here is weaker evidence than the scalar generator's**, because the
hand-written packing subjects already cover the shape. What would make it
worthwhile is either a divergence, or a perturbation showing the generated
subjects catch something the five hand-written ones do not. **Try that
perturbation** — for instance a width error that only manifests at a field index
the five do not use — rather than assuming breadth implies power.
