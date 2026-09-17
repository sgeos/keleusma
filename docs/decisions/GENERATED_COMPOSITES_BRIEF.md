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


---

## OUTCOME — **A SECOND WIDTH GAP, WIDER THAN THE FIRST**

**The generated composite refused on its SECOND program.**

```
struct P { f0: Byte, f1: Word, f2: Byte }
  ... f1: ((3 % 7) % 7) ...
  NewComposite at op 33 has an operand of unknown packed width
```

Characterised over eleven `Word` producers before anything was touched:

| could fill a field | could not |
|---|---|
| `param`, `add`, `sub`, `mul`, `neg`, literal | `div`, `mod`, `band`, `bor`, `bxor` |

**Five producers, not one.** The byte fix covered `Div`/`Mod` for a matched byte
pair; this is the same family across two more arms and the other scalar type.

Repaired with one rule — `preserved_scalar_width`: when both operands are scalars
of equal width, the result keeps it. **`Body` is excluded on purpose**, since a
`Body(n)` operand points at its data and labelling a computed scalar as a pointer
is the confusion the `Scalar`/`Body` split exists to prevent.

## THE BRIEF ASKED WHETHER THE BREADTH ADDS POWER. IT DOES, AND HERE IS THE PROOF

The brief warned against assuming breadth implies power, and asked for a
perturbation the generated subjects catch and the five hand-written packing
subjects do not. **The answer turned out to be better than a synthetic
perturbation: the generator found a gap the five could not.**

Every hand-written packing subject fills its `Word` field with a plain parameter —
`w: b`, `lo: a`, `hi: b`. **None of them ever puts a COMPUTED word in a field**,
so none could reach the `Word` half of this gap. The generated subjects vary field
types and values independently, and hit it on the second program.

**Both perturbations of the new rule fire** across all three files — dropping the
width (refusal) and inverting it (mispack). Reach is not the question here; the
question the brief raised was power, and the answer is a finding rather than an
argument.

## COST

240 composites across two seeds and three field counts: 0.86 seconds.
