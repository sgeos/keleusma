# `Op::IsStruct` — intent, producers, and disposition

**Status**: investigated, **recommendation is NOT to remove**. Raised by the operator on the
`v0.3.X` line, whose opcode-coverage census reports `IsStruct` as the one opcode of sixty-six with
no corpus witness.

**Scope note.** This document was written by the `v0.3.X` line, which does not own `src/` or
`tests/`. Everything below is a **reading of that line's files, not a change to them.** The two
actions it recommends both belong to the `v0.2.3` line.

## The question, as posed

> I want to know the intent behind the opcode. If there is no documented intent, no obvious intent,
> and no producers, then it is a strong candidate for removal.

Three conditions, conjoined. **Only one has to fail for the candidacy to fail, and two of the three
fail plainly.** The third fails in a more interesting way than either "yes" or "no".

## 1. Documented intent — PRESENT

`IsStruct` is specified in two normative documents, not merely implemented:

| Document | Line | Text |
|---|---|---|
| [`docs/spec/INSTRUCTION_SET.md`](../spec/INSTRUCTION_SET.md) | 183 | *Peek the top of the stack; push true if it matches the struct type.* |
| [`docs/spec/STRUCTURAL_ISA.md`](../spec/STRUCTURAL_ISA.md) | 38 | *Peek the top of the stack, push true if it matches the struct type.* |

It further carries **a deliberate specification correction**. The V0.2.x spec-conformance audit
found the shipped specification had drifted on this opcode and repaired the specification to match
the implementation — recorded in `CHANGELOG.md` as *"the peek-not-pop semantics and unit stack
growth of `IsEnum`/`IsStruct`"*. An opcode whose semantics were argued over and then pinned in a
conformance pass is the opposite of an undocumented one.

That correction is load-bearing in four separate places, each of which would need editing to remove
the opcode: `Op::stack_growth` (`src/bytecode.rs:2918`), the verifier's push/pop model
(`src/verify.rs:2367`), the typed abstract interpreter (`src/verify_typed.rs:876`), and the operand
encoding tables (`src/wire_format.rs`, opcode number **44**).

## 2. Obvious intent — PRESENT

`IsStruct` is the **struct analogue of `IsEnum`**: the runtime type test for a refutable pattern.
**Six sites in `src/` handle the two in a single match arm** — `Op::IsEnum(_, _, _) |
Op::IsStruct(_)` — in `bytecode.rs` (`nominal_op_cycles`, `stack_growth`,
`stack_shrink`), `verify.rs`, `verify_typed.rs`, and `selfhost/mod.rs`. The intent is not inferred from the
name; it is readable from the symmetry. (Other sites handle `IsStruct` alone or beside `Const`,
where it is the *operand shape* that pairs rather than the semantics.)

## 3. Producers — THE EMISSION SITE EXISTS AND NO CONSTRUCT KNOWN TO THIS TREE REACHES IT

This is the condition worth measuring rather than asserting, and the answer has **moved twice in
three days**. It is stated here as a date-stamped measurement, not as a property.

### The single emission site

`src/compiler.rs:11399`, inside `compile_pattern_test`, guarded by:

```rust
if ty.is_some() && named_type_name(ty) != Some(type_name.as_str()) {
```

In prose: *emit a runtime type test when the scrutinee has a known type and that type is not the
struct the pattern names.*

### THE EPISTEMIC STANDARD, WHICH IS NOT MINE TO WEAKEN

**No claim of unreachability is made here, and that restraint is deliberate.**
`native_codegen/tests/miscompilation_reach.rs` already records this finding, in the
`v0.2.3` line's own wording:

> A reader who can construct a survivor should treat this as incomplete rather than as a boundary.

This line adopted that wording because **it falsified the `v0.2.3` line's first producerless claim
within the hour** — a generic struct destructured in a parameter compiled, verified, took a bound,
loaded, and died with `InvalidBytecode`, the whole chain. Having done that once, this line has no
standing to make the stronger claim itself. **"No producer found by a bounded search" is the honest
statement and it is the one used throughout.**

### The load-bearing argument is the emission condition, not the probe list

`ty` is supplied at exactly **two roots** — function parameters (`src/compiler.rs:5398`) and match
arms (`src/compiler.rs:9273`) — with every other site recursing from one of those two. **Both roots
now run the nominal check first**, and that check rejects precisely the mismatch that would satisfy
the emission condition. That argument, not the enumeration below, is why the site is believed
unreached.

The probe enumeration is the weaker half and is reported as such: a probe that fails to *compile* is
weaker evidence than one that compiles and does not reach the site.

### Every route into that guard is now closed, by three separate repairs

| Route | Closed by | Where |
|---|---|---|
| un-annotated parameter (`ty` absent) | folded — an absent type is not an unconfirmed one | `0f369a70` |
| generic struct in a parameter (`P<Word>` specialized to `P__Word`, pattern still `P`) | the pattern now follows the type through monomorphization | `6d217f0a` |
| annotated with a different struct, a tuple, or an array | **refused by the type checker** — *"does not match scrutinee type"* | `6d217f0a` |

**`tests/opcode_reachability.rs` today contains five assertions about `IsStruct` and every one of
them is a NEGATIVE** — `!ops_of(src).iter().any(matches!(Op::IsStruct(_)))`. Counted rather than
estimated: **fifteen distinct source shapes across two negative loops** (seven match-scrutinee
routes, eight parameter-pattern routes), **three singleton controls**, and **three further shapes
the type checker refuses outright** before lowering is reached. **Not one test asserts a live
producer.**

### A stale comment in `src/compiler.rs` says otherwise, and it is wrong

`src/compiler.rs:11385` currently reads:

> So the fold NARROWS the fallback; it does not eliminate it, and `Op::IsStruct` still has
> producers. **Four are pinned in `tests/opcode_reachability.rs`, TWO OF WHICH STILL REACH THE
> LOAD-TIME HOLE**: a generic struct destructured in a parameter, and a pattern annotated with a
> different struct. Both verify, receive a memory bound, load, and then trap `InvalidBytecode`.

That comment was written at **`2ada8791`, 2026-08-21**. The repair that closed both named routes is
**`6d217f0a`, the same day**, and `git merge-base --is-ancestor` confirms the repair came **after**
the comment. The comment was correct when written and has been stale since.

**This matters more than a stale comment usually does.** It names a *live load-time hole* — a module
that `verify()` accepts and that then traps `InvalidBytecode`, which is precisely the class
`verify()` exists to exclude. A reader auditing the load-time guarantee finds this comment and
concludes the guarantee is breached. It is not, and the tests beside it prove it is not.

> **ACTION 1, for the `v0.2.3` line**: retract that paragraph. The accurate statement is that the
> guard remains, no source construct reaches it, and the fallback is retained deliberately.

## Why the corpus has no witness, which is not a corpus deficiency

The census reads **65 of 66**. The natural inference — *write a better test* — is wrong here, and
was already wrong once for `Op::Len` for a different reason.

**The emission guard is satisfied exactly when the test's answer is statically FALSE.** It fires
only when the scrutinee's type is known and differs from the pattern's name; in that state a
correctly-typed value can never carry the pattern's type name. So a witness would have to be a
program the type checker now refuses. **The last witness this tree ever had was a compiler defect,
and repairing the defect removed the witness.**

That is why 65 is the honest ceiling and the sixty-sixth is not obtainable by writing a better
script.

## Why it should nonetheless NOT be removed

Two of the operator's three conditions fail outright, which already settles it. Three further
reasons, in descending strength:

1. **The opcode is reachable in BYTECODE whether or not any source construct reaches it.** `verify()`,
   `verify_typed()`, and the VM all accept and execute it. The `v0.3.X` native backend's founding
   premise is that it consumes bytecode and does not care which compiler produced it, and
   `Vm::new_unchecked` exists so trusted precompiled bytecode can skip verification entirely.
   **"No producer in the reference compiler" is not "absent from the input domain."**

2. **The VM's behaviour on it is a deliberate safety check, not a leftover.** On a `Flat` body it
   returns `InvalidBytecode("Op::IsStruct on a flat struct; the type test is a compile-time
   constant")`. That refusal is what turned the three defects above into diagnosable failures rather
   than silent wrong answers. Removing the opcode removes the detector along with it.

3. **Removal is not free on the wire.** Opcode **44** is an encoded number with a decode arm at
   `src/wire_format.rs:1004` and a round-trip fixture. Removing it either leaves a hole in the
   numbering or renumbers everything above it, and renumbering is a wire-format change. The
   rad-hard minimal-ISA constraint argues for **not adding** opcodes; it does not by itself pay for
   a renumbering to drop one.

## Recommendation

**Keep `Op::IsStruct`. Record it as "specified, retained, with no producer found by a bounded
search as of 2026-08-24."** Do not record it as "unreachable" — the guard, the verifier arms, and
the VM execution path all remain live, the input domain is bytecode rather than source, and the
search that found no producer is a search rather than a proof.

> **ACTION 2, for the `v0.2.3` line**: consider whether the compiler's remaining guard should fold
> to **false** rather than emit the test, which would make the emission site provably dead rather
> than merely unreached.
>
> **This carries a caveat that must be settled before it is done, and it is a trust-boundary
> question rather than a compiler one.** The fold-to-false is sound only if a struct value's runtime
> type name always agrees with its static type. Inside the language it does. **Across the host
> boundary it is an assumption**: a native's declared return type is checked by signature, but the
> `StructBody::Boxed` a host hands back carries whatever `type_name` the host put in it. Whether
> that boundary is trusted is the `v0.2.3` line's call, and it is exactly the kind of assumption
> that should be written down before it is relied upon rather than after.

## What this line will do with the answer

Nothing in `src/`. The `v0.3.X` backend **refuses to lower `IsStruct`** and that refusal is correct
and stays: bytecode reaching the native backend may legitimately contain it, and the backend has no
way to establish that a module is reference-compiled. The census will continue to report 65 of 66
with `IsStruct` named, and `witness_integrity` will announce it if that ever changes.
