# Do the proven bounds survive translation to native code?

**Status**: answered, 2026-08-14, on the `v0.3.0` line.
**Short answer**: partly, and not in the way the premise implies. One of the four properties
examined is **not sound on its own terms**, independent of native code.

Keleusma's stated value proposition is definitive worst-case execution time and worst-case
memory usage. Those bounds are proven on the **bytecode**. This document records what they do
and do not say about the native code the `native_codegen` backend emits.

Measured by `native_codegen/tests/spike_bounds_transfer.rs` over the shipped corpus: **826
chunks, 51 modules**.

---

## The four-way classification

| property | verdict |
|---|---|
| loop and call structure (termination) | **TRANSFERS** |
| machine stack depth | **TRANSFERS**, but from a different premise than the bound |
| WCET magnitude | **NOT EXPRESSIBLE** at this level |
| WCMU operand depth | **DOES NOT TRANSFER** — and is unsound on 17 of 826 chunks |

---

## 1. Loop and call structure — TRANSFERS

**Argument.** The property that makes a Keleusma bound finite at all is structural: the type
checker rejects direct and mutual recursion, so the static call graph is acyclic, and the
verifier rejects any loop it cannot bound. The lowering preserves control flow one-for-one —
it adds no back edge and removes none, which `the_range_for_lowering_actually_emits_a_back_edge`
checks directly rather than assuming.

**Evidence beyond the structural argument.** Ten `piano_roll` modules execute natively and on
the virtual machine for 2100 ticks each and agree on the entire native call sequence, the
per-tick return, and the shared data segment. A lowering that had altered iteration structure
would diverge in the call sequence long before tick 2100.

**This is the real content of the guarantee.** A program that provably terminates in the
virtual machine lowers to native code that provably terminates. Everything below concerns
magnitudes, which is a weaker and separate question.

## 2. Machine stack depth — TRANSFERS, but the bound is not the bytecode's

**The magnitude does not come from the verifier.** It comes from `.stack_sizes` emitted by
`llc` and read with `llvm-readobj`, summed along the longest weighted path through the call
graph — `native_codegen/tests/native_stack_bound.rs`.

**What the verifier contributes is the acyclicity, not the number.** Without the no-recursion
guarantee the longest-path computation would not terminate; with it, the native bound is
computable from native facts. The bytecode's `max_frame_depth` is a *different* quantity in
different units and is not an upper bound on the machine frame — the register allocator
spills independently of anything the bytecode says.

**So the honest statement is a division of labour**, not a transfer: the language supplies the
structural precondition, the backend supplies the magnitude.

## 3. WCET magnitude — NOT EXPRESSIBLE at this level

**Why not.** The bytecode WCET bound is a count of virtual-machine operations weighted by a
cost model calibrated against the interpreter by `keleusma-bench`. Native code does not
execute those operations. A number in interpreter cost units is not a claim about
nanoseconds on an unspecified machine, and no amount of care in the lowering makes it one.

**The ordering does not survive either, which is the sharper result.** A weaker hope is that
the bytecode bound at least *orders* chunks the way the native code does, so it could serve as
a proxy. Measured across 20 stream chunks with both figures, **9 of 190 comparable pairs are
inversions (4.7%)** — pairs the bytecode bound orders one way and the native code the other.
The clearest is `lexer.kel::main` at bound 164 producing **1083** native instructions against
`piano_roll_0.kel::main` at bound 498 producing **385**: the bound says one is three times
smaller, the emitted code says it is nearly three times larger.

**A caveat against over-reading this.** Native instruction count is itself only a proxy for
time — it ignores caches, pipelining and branch prediction. The inversion result therefore
refutes the *proxy* claim, which is the weaker and more useful thing to refute. It is not a
timing measurement and is not offered as one.

## 4. WCMU operand depth — DOES NOT TRANSFER, and is unsound on its own terms

This is the finding that matters most, and it is not about native code.

**`wcmu_region` drives its own running depth NEGATIVE on 17 of 826 shipped chunks**, reaching
**-5** at worst. An operand stack cannot hold a negative number of slots. Wherever this
happens the walk is not tracking the real stack, and the peak taken from that same walk is not
an upper bound on anything.

Affected modules include the self-hosted compiler stages `analyze.kel` and `parse.kel`, and
`piano_roll_0/1.kel`.

### The mechanism, on `02_struct_field.kel::manhattan_norm`

`Op::CheckedAdd` is documented in `src/bytecode.rs` as popping two operands and pushing
`(high, low, flag)` — a **gross push of three**, a **net delta of `+1`**. `stack_growth()`
returns the net `1`, and `wcmu_region` uses that value as the transient rise when computing
`peak = max(peak, current_offset + growth)`. The transient is three.

Reconstructed with real semantics:

```text
  GetLocal   -> 1
  GetField   -> 1
  GetLocal   -> 2
  GetField   -> 2
  CheckedAdd -> 3   (pops 2, pushes high, low, flag)
  PopN(2)    -> 1
  Return     -> 1
```

True peak **3**. The model reports peak **1** and ends at **-1**.

**The emitter independently allocates 3 operand slots**, which is the true figure. That
disagreement is what surfaced this: `q1` flags three chunks where the emitted operand storage
"exceeds the proven bound", and in every case **the emitter is right and the bound is low**.

Two independent computations of the same quantity, sharing no code, disagreeing — which is
exactly why the comparison was worth making.

### What this does and does not establish

**It does not establish a memory-safety fault.** The runtime grows the operand stack
dynamically and reports `OutOfArena` when a pre-size proves too small, so an under-estimate
surfaces as a refusal rather than as corruption. Eight of ten `piano_roll` modules hit exactly
that during the module differential.

**What it costs is the word "definitive"** in front of WCMU for the affected chunks. A bound
that is not an upper bound is not a bound.

### Ownership

**`src/verify.rs` belongs to the `v0.2.3` line and has NOT been modified here.** This document
and `q4_the_stack_model_goes_negative_on_shipped_code` report the defect with a reproducing
case; the repair is theirs to make. The test is deliberately written as a **report with a
guarded corpus size**, not as an assertion that the count is zero — asserting zero would fail
the suite over a defect this branch does not own, and asserting the current 17 would fail the
moment they fix it, which is the wrong signal in the other direction.

---

## What was stale, and how it was caught

The spike this work started from asserted the closed form
`allocas(O0) == sum_f (MAX_STACK + locals(f))`, which an article published. That assertion now
**fails: 4362 measured against 49267 predicted**, an eleven-fold over-estimate. `MAX_STACK`
became a refusal ceiling rather than a provisioning quantity when `ensure_slot` began growing
operand slots on demand.

The dead formula is what made the real question askable. Once the emitter emits what it
actually used, its count is an independent computation of the quantity the verifier proves,
and the two can be compared — which is how §4 was found.

**One methodological note.** The first version of the comparison paired the n-th `define` in
the IR with the n-th chunk and reported the same three violations. Positional pairing is an
assumption about emission order, and a wrong pairing produces a violation indistinguishable
from a real one. Re-indexing by the `@kel_chunk_N` symbol name reproduced the three, which is
the only reason they are reported here.

---

## Owed: the `is_lowered` resynchronisation, measured but NOT landed

The three stale copies of `is_lowered` (`spike_corpus_coverage.rs`,
`spike_stream_sufficiency.rs`, `spike_composite_split.rs`) were to be retired alongside this
work and were not. The measurement was taken so the next session starts from data rather than
from a re-derivation.

**Current staleness, in the pessimistic direction**, over the 53 modules the real lowering
accepts:

| op the model still calls unsupported | instances |
|---|---|
| `CallVerifiedNative` | **1019** |
| `NewComposite` | 225 |
| `Const` | 111 |
| `Yield` | 38 |
| `IsEnum` | 29 |
| `Reset`, `Stream` | 20 each |
| `GetIndex`, `GetTupleField` | 14 each |
| `GetEnumField` | 4 |
| `GetField` | 2 |

**Why it was not landed rather than half-landed.** Promoting
`the_lowered_predicate_is_not_stale_pessimistic` from a report to an assertion requires the
model to be in step FIRST — asserted today it fails on all eleven rows above. And the model is
per-OP while `module_refusals` is per-CHUNK, so "consume `module_refusals`" is not a
substitution; it is a change to what these spikes can report at all. That is a design decision
with a real choice in it, not a mechanical edit, and starting it with the budget remaining
would have left a third instrument in an intermediate state next to two stale ones.

The staleness is in the **safe** direction — the model understates coverage — so the figures
derived from it are conservative rather than misleading. That is why this could wait; it is
not why it should.

---

# Addendum, 2026-08-14: `CheckedAdd` does NOT stand alone

The finding above was reached by accident. This is the systematic answer, from
`native_codegen/tests/spike_opcode_stack_audit.rs`.

**Four opcodes are wrong, in two different ways.** The distinction matters, because a repair
that treats them alike will fix one kind and leave the other.

## Kind 1 — the NET is wrong: the flat scalar-field accessors

| opcode | `verify_typed` models | `stack_growth`/`stack_shrink` declare | |
|---|---|---|---|
| `GetField(Flat)` | pop 1, push 1 → **net 0** | 0 / 1 → **net -1** | **WRONG** |
| `GetTupleField(Flat)` | pop 1, push 1 → **net 0** | 0 / 1 → **net -1** | **WRONG** |
| `GetEnumField(Flat)` | pop 1, push 1 → **net 0** | 0 / 1 → **net -1** | **WRONG** |

Each pops the composite and pushes the field. The model records the pop and not the push, so
the running depth loses one slot per field access. **This is what drives the offset negative**
— `manhattan_norm` has two field reads and bottoms out at exactly `-1`.

`verify_typed` is an independent in-tree reconstruction of the same operand stack, written to
validate flat offsets. Two models of one quantity, disagreeing.

## Kind 2 — the NET is right and the TRANSIENT is wrong: checked arithmetic

`CheckedAdd`, `CheckedSub`, `CheckedMul`, `CheckedDiv`, `CheckedMod` declare `growth = 1,
shrink = 0`; `CheckedNeg` declares `growth = 2, shrink = 0`.

**The net is correct**: each pops two and pushes `(high, low, flag)`, so `+1` is right. The
defect is that `wcmu_region` uses `growth` as the **transient rise** when computing
`peak = max(peak, current_offset + growth)`, and the transient is the **gross push of 3**, not
the net `1`. The peak is under-counted by two at every checked-arithmetic site.

The comment on the shrink arm — *"peak vs. final; shrink is zero because there is no net pop"*
— shows the peak/final distinction was in mind. The value supplied for the peak is
nonetheless the net one.

## What is CORRECT, checked rather than assumed

The four flat accessors do **not** behave alike, and assuming they did would have produced a
wrong list:

| opcode | why it is right |
|---|---|
| `GetIndex(Flat)` | pops **index and array**, pushes the element → net `-1` is correct |
| `NewComposite(Flat)` | `shrink = c.count()`, arity-aware; the `2` seen in the audit table is one instance's field count, not a constant |
| `Div`, `Mod`, `If`, `SetLocal`, `SetData`, `SetDataIndexed`, `PopN` | genuine consumers |
| `GetDataIndexed`, `IsEnum`, `GetLocal`, `GetData` | net matches the typed reconstruction |

**`GetIndex` is the one that makes the point.** It sits in the same syntactic family as the
three wrong accessors and declares the same net `-1`, and it is right, because it consumes an
extra operand they do not.

## Coverage, stated rather than implied

The synthetic corpus is **19 compiled cases of 23**. The four rejections are reference-compiler
rejections, printed by the test, not backend gaps: `bitwise` and `shift` use function-call
syntax the parser does not accept for those operators, `loop_for` a `for` form it does not
parse, and `stream` fails the type checker on a `loop` body producing `()`.

**16 opcodes appear in the shipped corpus but in no isolating synthetic case**: `BitAnd`,
`BitOr`, `BitXor`, `BreakIf`, `CmpEq`, `CmpGe`, `CmpLe`, `CmpNe`, `Dup`, `Not`,
`PushImmediate`, `Reset`, `Shl`, `Shr`, `Stream`, `Yield`. These were walked by the
shipped-corpus scan and are not implicated by it, but a defect in one could be masked by
co-occurrence. **They are the audit's remaining hole and are not claimed as clean.**

## Verdict on the question asked

**No — `CheckedAdd` does not stand alone.** Four opcodes are wrong across two distinct defect
kinds, and the three flat accessors are the ones that actually produce the negative depth.
Sixteen opcodes remain unisolated and are named above rather than passed over.

Still **reported, not repaired**: `src/verify.rs` and `src/bytecode.rs` belong to the `v0.2.3`
line and are untouched here.

---

# Addendum 2, 2026-08-14: the audit's hole is closed. Nothing further is wrong.

Sixteen opcodes occurred in the shipped corpus with no isolating synthetic case and were
explicitly **not claimed clean**. Twelve new cases now isolate thirteen of them.

## The verdict: nothing further

`BitAnd`, `BitOr`, `BitXor` and all six comparisons declare **net −1**, and that is
**CORRECT** — each pops two operands and pushes one. `Reset`, `Stream`, `Yield` and `Not` are
likewise consistent.

**The wrong set is unchanged**: `GetField`, `GetTupleField` and `GetEnumField` on the net, and
the checked-arithmetic family on the transient. No new defect.

This is the outcome the goal named as a success in its own right, and it is worth stating
plainly rather than burying: **the four already reported are the whole set, over every opcode
the corpus exercises that a minimal case can reach.**

## Three remain unreachable, and why

| opcode | why no isolating case |
|---|---|
| `BreakIf` | the `break if <cond>` form the case used is **rejected by the parser**; the real syntax was not found and is not guessed at |
| `Dup` | **compiler-emitted only** — no source construct maps to it directly |
| `PushImmediate` | **compiler-emitted only** — an optimisation of `Const` for small values |

`Dup` and `PushImmediate` are not reachable from source by construction, so "no isolating
case" is a property of the language rather than a gap in the audit. `BreakIf` is reachable and
was not reached; that one is a genuine remaining hole and is named as such.

## Five of the seventeen cases are reference-compiler rejections

`bitwise` and `shift` use call syntax the parser does not accept for those operators,
`loop_for` a `for` form it does not parse, `stream` fails the type checker, and `break_if2`
fails the parser. **Rejections, not backend gaps** — printed by the test, never silently
dropped, which is the distinction this branch keeps having to redraw.
