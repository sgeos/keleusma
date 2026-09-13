# BRIEF — a driven witness per opcode

## The third axis

Three questions, increasing in strength:

1. **Does the backend lower it?** — `opcode_denominator.rs`, which says in its own
   header that a `Lowered` verdict *does not mean the emitted code is correct*.
2. **Does the corpus execute a module containing it?** —
   `differential_coverage.rs`, which disclaims the stronger reading explicitly:
   an executed module does not exercise every opcode it contains.
3. **Is there a program that emits it, runs on both, and agrees?** — nothing
   answered this.

## What it cost to find out, and what that says

The driver constraint is real: `common::vm_and_native_two_arg` panics unless the
virtual machine finishes with a `Value::Int`. Witnesses are therefore written to
return `Word`, wrapping the construct under test — `(a as Byte) as Word` witnesses
`WordToByte` without asking for a `Byte` return.

**Four rows were wrong, and the emission check caught every one:**

| row | I assumed | actually |
|---|---|---|
| `BoundsCheck` | a local `xs[a]` emits it | emits NOTHING; a DATA-SLOT array index is the producer |
| `CheckedDiv` | Word `/` emits it | Word `/` emits `Div` |
| `CheckedMod` | Word `%` emits it | Word `%` emits `Mod` |
| `Div`, `Mod` | Byte/Fixed only | Word `/` and `%` emit exactly these |

Each would have **agreed while proving nothing about the opcode named**. That is
why the witness check asserts emission before comparing, and it earned that
assertion four times in one increment.

The `BoundsCheck` finding is the interesting one: a local array index emits no
bounds check at all, which is the precise premise that once caused a defect here —
the emitter assumed *"the compiler emits `Op::BoundsCheck` before the index"* and
it does not.

## Three apparent divergences were the harness

A float argument passed as `i64::MIN` instead of its bit pattern; a float return
read as an integer; and a hand-named function signature that took a **SIGBUS** on
the first composite witness. **None was a backend defect.** `common/mod.rs` warns
about the third in as many words: *"a harness that names a signature cannot see it
change."* Delegating to the canonical driver fixed it.

**A probe that reports a divergence has implicated itself first.** This one
implicated itself three times before producing a single trustworthy row.

## Wrong turns to avoid

- **Reporting a probe's divergence as a defect without clearing the probe.** Three
  would have been false alarms here.
- **Listing an opcode as driven without asserting the witness emits it.** Four
  rows would have been decorative.
- **Forcing a witness for an opcode whose producer is unclear.** `BoundsCheck` is
  recorded with what was learned instead, which is more useful than a contrived
  program.
- **Naming a lowered function's signature by hand.** The package already pays for
  that lesson; the driver reads `count_params` and sizes buffers from the
  published contract.
