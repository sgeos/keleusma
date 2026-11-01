# BRIEF — the last substantial exempt class, and the machinery already exists

## The goal

Exemptions are down to eleven. Most are principled and permanent: two preludes with no entry, two
deliberate backend refusals, four faults comparable by the fault observable. **Three are not**:

> `rogue_ai_boss`, `rogue_ai_hunter`, `rogue_ai_tracker` — *composite entry parameter; covered by a
> hand-written differential*

All three take **one parameter of `Flat { kind: 0, size: 40 }`** — a fixed-size flat tuple. The
harness supplies `Value::Int` arguments and cannot build one, so `params_are_scalar` exempts them.

**The machinery exists in the other direction.** A composite RETURN is already built on both sides
and compared byte for byte: `composite_stub_bytes` produces deterministic bytes, the virtual machine
side wraps them with `FlatComposite::build_in_arena`, and the native side hands back a pointer. This
is the same construction used as an ARGUMENT.

## What the value actually is, stated honestly

These modules are **already covered** by `module_differential.rs`, and that claim was audited once
and held. So this does not add coverage from nothing.

**What it adds is the argument vectors.** The hand-written differential drives them with hand-chosen
inputs; the corpus differential drives every module with twenty-four. Varying the composite body by
seed is the actual gain, and it is worth stating as that rather than as "three more modules
executing".

**If the body cannot be varied meaningfully, say so and reconsider.** Twenty-four identical bodies
would be twenty-four comparisons of one constant against itself — the inflation this line already
recorded, where half the agreeing count agreed on a single value.

## Prior failures and the specific wrong turns to avoid

- **The invented body must be a value the module can HANDLE.** The composite-return code already
  warns about this in its own comment: a value the module cannot match makes it fault, and then the
  harness is measuring the stub rather than the lowering. **Check the modules still run, not just
  that they stop being exempt.**
- **Both sides must see byte-identical bodies.** The virtual machine gets an arena-resident
  `FlatComposite`; the native side gets a raw pointer. If those disagree the modules will disagree,
  and that would be a defect in this harness rather than in the lowering — which has happened twice
  today already, once with a mask patched into one of two registration paths and once with a table
  the virtual-machine path never populated.
- **`kind: 0` is a TUPLE.** The kind mapping is explicit elsewhere (`3 => Enum, 2 => Struct,
  1 => Array, _ => Tuple`). Build the variant the signature declares; do not guess from the size.
- **A newly-driven module that DISAGREES is a finding, not a thing to mask.** Report it.
- **Do not widen `params_are_scalar` past what is handled.** A composite of unknown or variable size
  is a different problem; the exemption should survive for anything this cannot build.

## What a good outcome looks like

The three modules execute and agree, driven with bodies that VARY across seeds; the exempt count
falls to eight; and the report says plainly that these were already covered elsewhere and what the
argument vectors add.

**If they execute but the bodies cannot vary, that is a weaker result and must be reported as one** —
three modules moved from "not compared" to "compared once", which is still an improvement and is not
what twenty-four vectors implies.
