# The oracle's blind spot is systematic — a sweep, and what it found

Written 2026-08-20 after the boolean-literal miscompile. **The hypothesis was that
the bool bug was not special**: the differential oracle covers only constructs the
stage corpus uses, so any construct absent from the corpus is unverified. Tested by
compiling twenty small programs through both compilers and comparing bytes.

**Two more silent mis-lowerings in the first twenty cases.**

## 1. THE CAST DIRECTION IS INVERTED, and the target type is discarded at parse time

```
fn main() -> Byte { 7 as Byte }
  reference:   Const(0), WordToByte, Return
  self-hosted: Const(0), ByteToWord, Return
```

`codegen.kel` says so in its own comment: *"Cast (kind 26): a `Byte as Word`
widening. Emit the operand ops then ByteToWord"*, and `push_cast` emits
`wire.bytetoword` unconditionally. It cannot do otherwise, because `parse.kel` at
the cast site says of the target type name: **"skip it"**. The direction never
reaches the node.

So `x as Byte` and `x as Word` lower identically, and one of them is wrong. A
`let b = 7 as Byte; b as Word` chain gets the first cast wrong and the second right.

**THE FIX HAS THE SAME SHAPE AS THE BOOLEAN ONE.** `Cast` is a UNARY node whose
payload is unused, exactly as `Unit`'s was. Carry the target in it — 0 for `Word`
(today's behaviour, so existing programs stay byte-identical) and 1 for `Byte` —
and have `push_cast` select the opcode. **`parse.kel` already has `word_id`,
`byte_id` and `bool_id` shared slots for recognising type names**, which is what
they are for; the cast site simply does not consult them.

## 2. A STRING LITERAL BECOMES AN INTEGER

```
fn main() -> Word { let s = "hi"; 1 }
  reference constants:   [StaticStr("hi"), Int(1)]
  self-hosted constants: [Int(3), Int(1)]
```

The ops are identical; the CONSTANT POOL is not. The stage emits the lexer's intern
id as an `Int` where the reference emits a `StaticStr`. `codegen.kel` has
`intern_str` (pool tag 1) and it is not reached from the string-literal path, which
falls through to `intern_int` (tag 0).

`parse.kel` line 282 states the intent — *"A string literal emits Const of a
StaticStr pool entry"* — so the parse side believes it is doing this. **Verify where
the tag is lost before changing anything.** Note `Text` is listed in `CLAUDE.md`
among the divergence classes the CLI refuses, so this MAY be a known limitation
rather than a defect; check that before reporting it as new.

## 3. THE BOUNDARY TABLE IS NOT A BOUNDARY. IT IS A CENSUS OF ONE FEATURE AREA.

Labels by family, derived from the table:

| family | cases |
|---|---|
| `eq` | **41** |
| `bool` | 10 |
| `op` | 8 |
| `comp` | 8 |
| `scalar` | 6 |
| `prec` | 5 |
| `ctrl` | 4 |
| `tuple` | 1 |

**Forty-one of eighty-eight cases are equality lowering.** There is no `cast`
family and no string-literal case. Boolean literals had none until tonight. The
table's name promises a support boundary; its contents describe how thoroughly one
feature was tested.

**This is the finding that generalises.** Both silent miscompiles found tonight sit
in families the table does not cover, and the sweep that found them took twenty
cases. The productive work is not fixing these two — it is widening the table
family by family and fixing what that exposes.

## What limits every one of these

`self_hosted_compile`, the shipping path, cross-checks ops, constant pool and local
count against the reference and refuses on divergence. **Every defect in this
document produces a loud error for a user and a wrong module only for a direct
caller of `self_host_compile` that skips the check.** State that whenever reporting
one, or the severity reads far higher than it is.

## Method notes for the next sweep

- **A `PARSE-FAIL` in the probe is usually the probe's fault.** `for .. limit` came
  back PARSE-FAIL because the test source used `let mut`, which this language does
  not have. Confirm the REFERENCE accepts a generated program before concluding
  anything about the stage.
- **Compare BYTES, not ops.** The string-literal case has identical ops and a
  different module; an ops-only comparison would have called it clean.
- **Classify three ways, not two**: identical, self-refuses loudly, and DIFFERS.
  Only the third is dangerous. A loud refusal is an honest gap.

---

# Round two of the sweep (2026-08-20, later)

Twenty-two more cases, corpus-verified syntax. **19 identical, 1 honest loud
refusal, 1 DIFFERS, 1 bad probe.** The yield is falling, which is itself the
useful signal: the first twenty cases found two defects, the next twenty-two found
one.

## THE FINDING: nested array literals mis-size the outer composite

```
fn main() -> Word { let a = [[1, 2], [3, 4]]; 1 }
  reference outer: NewComposite(Flat { kind: Array, count: 2, byte_size: 32 })
  self-hosted:     NewComposite(Flat { kind: Array, count: 2, byte_size: 16 })
```

**The outer array of two 16-byte arrays is sized as 16 rather than 32.** The
element byte size is not propagated; the outer composite is sized as if its
elements were scalars.

With a chained index the body is additionally **TRUNCATED**:

```
fn main() -> Word { let a = [[1, 2], [3, 4]]; a[0][1] }
  reference:   ... SetLocal(0), GetLocal(0), Const(4),
               GetIndex(FlatNested { size: 16, variant: Array }),
               Const(0), GetIndex(Flat { kind: Int }), Return
  self-hosted: ... NewComposite(..16), Return
```

No `SetLocal`, no `GetLocal`, neither `GetIndex`. The stage returns the constructed
value instead of the indexed element.

**A FLAT array is byte-identical** (`let a = [1, 2]; a[1]`), so this is specific to
nesting, not to arrays.

## WHY THIS ONE IS RECORDED RATHER THAN FIXED

The four fixes taken tonight -- the `bool`/`Bool` tag, the boolean literals, the
cast direction -- were each a small change with a precedent already in the tree,
and each was mutation-verified in under an hour. **This one has neither property.**
It is two defects (a byte-size computation and a dropped index chain) in the
composite-layout machinery, which the B28 flat-byte representation makes
load-bearing for worst-case memory bounds.

**Attempting it unsupervised at this hour would violate the brief's own rule**:
pin rather than repair when the change is not small and well-precedented, and say
so. The boundary case records the truth so the table stops overstating support; the
fix is a well-specified increment for a session with an operator awake.

## Proportionality, as always

`self_hosted_compile` cross-checks ops against the reference and refuses on
divergence, so this is a loud error on the shipping path and a wrong module only
for a direct caller that skips the check.

## Honest gaps found, which are NOT defects

`struct P { a: Word }` with a field read makes the stage **refuse loudly**. That is
the correct behaviour for an unsupported construct and is worth distinguishing from
the silent cases -- a loud refusal is an honest gap.

## Probe error to avoid repeating

`checked_add` came back PROBE-BAD on a parse failure. The checked-arithmetic arm
syntax is not what I guessed; take it from `examples/scripts/10_multbyte.kel`, which
uses `overflow(_, l) => (1, l)` inside the construct's own form rather than as a
trailing expression clause. **Second probe-syntax error in two sweeps** -- generate
probes from corpus sources, not from memory of the grammar.
