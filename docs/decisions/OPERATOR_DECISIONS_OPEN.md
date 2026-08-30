# What needs your decision, and what happens if you say nothing

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

> **⚠ RULINGS RECEIVED 2026-08-29 on items 2, 4, 5 and 6 — see
> [`ABI_RULINGS.md`](./ABI_RULINGS.md).** Float and string are **settled**; `Fixed`, `Text`, `Opaque`
> and `Unit` remain **open**, one of them because a supposition in the ruling was incorrect. The
> sections below are kept as the standing statement of each question; the rulings document records
> what was decided, what was assumed by me rather than ruled, and what is still needed.

**One page, no arguments re-run.** Each case has its own record; this only says what is open, what it
costs, and what I do by default. **Where the underlying record deliberately declined to recommend, so
does this.**

## The measured position

**All remaining capability work on this line is behind a decision only you can take.** Re-derived
2026-08-29, not recalled:

| | |
|---|---|
| corpus modules the backend lowers end to end | **66 of 69** |
| chunks fully lowerable | **1070 of 1074** |
| unlowerable chunks | **4 — and all four sit in the 3 refused modules** |

**That figure is exhaustive over CORPUS-BLOCKING work, and nothing more.** An earlier version of this
page read *"there is no fourth thing to fix"*, which was wrong: **a coverage census can only surface a
decision that blocks a corpus module.** No corpus source declares a `Fixed`, `Float` or `Text` shared
slot, so those refusals block nothing, appear in no figure, and were invisible to a list built from
figures. **That is how item 4 below came to be missing from this page** until the operator asked
whether the ABI questions were resolved.

**So this page has two parts**: decisions that block lowering *today* (1–3), and decisions that are
open regardless of whether the corpus exercises them (4–6). **The completeness of the second part rests
on my search, not on a measurement**, which is a weaker guarantee and is stated as such.

---

## 1. `Stream` on `13_telemetry_stream.kel` — a soundness obligation

**Record**: [`COMPOSITE_SLOT_REUSE_OBLIGATION.md`](./COMPOSITE_SLOT_REUSE_OBLIGATION.md)

A composite built inside a loop and yielded out gets one offset for the life of the chunk, so a host
holding iteration *n*'s handle reads iteration *n+1*'s bytes — **a silently wrong value, not a `Stale`
error**. The backend refuses the shape today at **zero** cost, because the module is already refused
for `Stream`.

**The trade, which is why it is yours**: discharging it requires the planner to consume a confinement
verdict, and consuming no verdict is exactly why a wrong verdict cannot miscompile today. The option
that would convert the silent wrong value into a `Stale` error edits `src/vm.rs` and the arena, **which
this line may read and must not edit**.

**Default if you say nothing**: the refusal stands, coverage stays where it is, three tripwires fail if
the situation changes.

---

## 2. The float entry ABI — one refused module

**Record**: the float guard routes, in `native_codegen/tests/float_guard_routes.rs`.

The backend has no float representation: no `f64_type`, no float opcode lowered, and an entry ABI of
`i64` where a double belongs. **This is a live defect, not only a gap**: a host calling such an entry
under the real C ABI would read an FP register this code never wrote. The pass-through case
`fn p(a: Float) -> Float { a }` round-trips a bit pattern, which is correct by accident and only inside
the harness. Refusing the module is the guard, and it closes four routes — chunk signature, chunk
constant, native return shape, and the data-segment slot, that last one at the access rather than the
declaration. It also blocks `IntToFloat` and `FloatToInt`.

**No options document exists for this item. The options below are my derivation**, unlike item 4's,
which are recorded.

| option | cost |
|---|---|
| **A** give float entries a real `double` ABI | the honest fix and the largest: `f64_type`, FP registers, float opcodes. Also forces a decision on `Float` in shared slots and on whether the two embeddings stay source-compatible |
| **B** refuse `Float` host-visibly at the source | structural and cheap; consistent with item 4's option B. Breaking source change |
| **C** keep the refusal indefinitely | zero cost, zero progress; two opcodes stay unlowered and the surface keeps accepting programs the backend will not take |

**This item and item 4 are the same shape** — a host-visible scalar whose in-module representation is
settled and whose boundary contract is not — which is why you ruled they be settled together.

**Default if you say nothing**: unchanged. This is operator-held and I have not touched it.

---

## 3. `lower_module`'s admissibility precondition — documented, not enforced

**Record**: [`BACKEND_ADMISSIBILITY.md`](./BACKEND_ADMISSIBILITY.md)

A module that `verify()` accepts but `Vm::new` **refuses** — no statically extractable bound — is
lowered without complaint, and the code is not memory-safe. **Zero live instances**: 66 lower, 0
unbounded, pinned by a test.

**The open choice**: enforce it inside `lower_module`, which couples a pure lowering function to the
resource analysis and pays that on every call, or leave it documented.

**Default if you say nothing**: documented and pinned, as now.

---

## 4. The `Fixed` shared-slot ABI — where does the host-visible scale live?

**Record**: [`FIXED_SHARED_SLOT_ABI.md`](./FIXED_SHARED_SLOT_ABI.md)

**The representation is settled** — signed two's-complement Q-format of the runtime's word width. **The
scale is not.** `N` is carried by the opcodes and the compile-time type; the slot descriptor has no
field for it, so `Fixed<16>` and `Fixed<8>` — a factor of 256 apart — are **byte-identical to a host**.
Pinned by a test. The surface admits such a slot today: it compiles, verifies, and takes a bound.

| option | cost |
|---|---|
| **A** reuse `len` for the scale | a `len = 0` artifact reads back as **Q0 — accepted and silently wrong**, reinstating the hazard the `BYTECODE_VERSION` bump to 2 was authorised to close. Also no answer for `Fixed` inside a composite slot. |
| **A′** encode it so "absent" ≠ "Q0" | keeps A's benefit, drops its fatal objection. **Not evaluated**, and it is the `v0.2.3` line's schema; a distinct field is a wire change. |
| **B** refuse `Fixed` host-visibly at the source | breaking source change. **The only structural option** — nothing to misread. |
| **C** one canonical Q format | worst failure mode: silent precision change or refusal-with-extra-steps. **Not recommended.** |

**The recorded preference is B, then A, then C — but it is conditional and the condition is yours to
state.** You asked whether Keleusma's fixed-point interoperates across object files from other
languages. That input reverses the preference: for **convention-based** interop B stands; for
**self-describing** interop B is the wrong answer, because self-description is precisely what B
removes, and that argues for A′. **You asked the question but have not stated the goal**, and those are
different things. **This is the single input that would settle both item 4 and item 2.**

**Default if you say nothing**: the slot stays refused. Its message was corrected on 2026-08-29 to name
the missing scale rather than imply the representation is undecided.

---

## 5. The string ABI — provisional, never ratified

**Record**: the doc comment on the static-string emitter in `native_codegen/src/lib.rs`.

A literal lowers to `{ i64 len, [n+1 x i8] bytes }`, NUL-terminated: length first and explicit because
a Keleusma string is a **byte** string and a bare `char*` would silently truncate an interior NUL; the
trailing NUL costs one byte so a C host can still use `str*` functions.

**The unresolved part**: on the virtual machine a string-taking native receives an owned `String`
through marshalling; natively it receives this pointer. **The two embeddings are not source-compatible
for a string-taking native.**

| option | cost |
|---|---|
| ratify the current shape | cheapest; accepts permanent source-incompatibility between embeddings |
| make the embeddings agree | removes it; touches marshalling in `src/`, which this line may read and must not edit |
| refuse string-taking natives natively | consistent with the B posture elsewhere; loses capability |

**Default if you say nothing**: the current shape stands, undeclared.

---

## 6. The remaining unsettled shared-slot kinds

Refused by `alloc_format_kind`, each for its own recorded reason rather than a common one: **`Unit`**
(flat representation unsettled), **`Float`** (item 2, in slot form), **`Text`** (Workstream C),
**`Opaque`** (host handles, Workstream D).

These are deferrals rather than priced questions. **`Unit` looks cheap to settle and I have not
investigated it**, which is a gap rather than a judgement.

---

## Not a decision, listed so it is not mistaken for one

**`Len`** is refused because the only construct reaching it produces a loop with no statically
extractable bound, so the program is inadmissible regardless. **Nothing to decide** — see
[`LEN_FLAT_ARRAY_HAZARD.md`](./LEN_FLAT_ARRAY_HAZARD.md) for the separate hazard it exposed, which is
also reported rather than repaired.

---

## What I will do without an answer

Continue on **correctness of what already lowers** rather than on new capability, because that is what
is available: the differential's mutation family, its filters, and the instruments that report figures
to you. **Every refusal above stays in place**, which is the safe direction in each case — a refused
slot cannot be misread by a host.

**Publication remains HELD** and nothing here is a request to change that.

**A caution about my own recent work**: five defects in a row were in my instruments, all understating
the subjects, and four published figures were corrected. **This page itself was the sixth** — it was
built from a coverage measurement and inherited that measurement's blind spot. The corpus walk and the
mutation probe are now single shared functions so that class cannot recur silently, but read my figures
with that history in mind.
