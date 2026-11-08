# The unwind personality requirement, and its removal

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Measured and changed 2026-09-01.**
Brief: [`NOUNWIND_BRIEF.md`](./NOUNWIND_BRIEF.md). Origin:
[`NARROW_TARGET_LINKAGE.md`](./NARROW_TARGET_LINKAGE.md), which left this open rather than
explaining it.

---

## The cause, established by changing exactly one thing

**A module that calls a host native required `__aeabi_unwind_cpp_pr0`. A module with no call did
not.** That contrast is what identified the mechanism: the call to an external function with no
`nounwind` attribute forces LLVM to assume the caller may unwind, and the ARM backend then emits the
unwind machinery that reference implies.

`native_codegen/src/lib.rs` set **no function attributes at all**. The brief recorded that as a
hypothesis and warned explicitly that an absent attribute beside a present symbol is correlation.
**It was tested rather than assumed**, on `external_native_witness.kel` at
`thumbv8m.main-none-eabihf`:

| marking | undefined symbols |
|---|---|
| none | `__aeabi_unwind_cpp_pr0`, `kel_native_host__tick` |
| defined functions only | `kel_native_host__tick` |
| declarations only | `kel_native_host__tick` |
| both | `kel_native_host__tick` |

## The choice: mark what the backend defines, not what it declares

**All three markings work, so the decision is not about efficacy.** Marking the host natives'
declarations would be this backend asserting something about **code it does not generate**. Marking
the functions it defines makes the narrower claim and achieves the same result, so that is what
`mark_nounwind` does.

The brief forbade marking declarations before the measurement was taken, and the measurement did not
force a revision. **Recorded because the opposite could have happened**: had declaration-marking been
the only effective option, the brief's constraint would have had to be argued down in public rather
than quietly dropped.

## What is being asserted, and why it is not new

Keleusma has no exceptions and a fault traps, so nothing this backend generates can unwind.

**The assertion reaches the host, and that is the part worth stating plainly.** A chunk calls host
natives, so asserting the chunk never unwinds asserts that no unwind arrives from a native either.
**That constraint already existed**: natives are `extern "C"`, and unwinding out of an `extern "C"`
boundary is undefined in C and aborts in Rust. The attribute makes the existing contract explicit
rather than adding a new one.

**But the consequence of violating it changes.** An embedder whose native unwinds was already outside
the contract and would previously have crashed. After this change they may get miscompilation instead.
**That is why it is written in `native_codegen/README.md`, where an embedder would look, and not only
here.**

## The prediction, resolved against each falsifier

**Recorded before measuring**: the narrow toolchain set loses `__aeabi_unwind_cpp_pr0` and nothing
else, eleven to ten, the remaining ten unchanged as a set, host unchanged at two.

| falsifier | fired? |
|---|---|
| the personality is still required after the attribute is set | **no** |
| any other symbol enters or leaves either set | **no** — narrow is exactly the ten, host exactly the two |
| any existing differential disagrees, at any target | **no** — see the verification below |

**The prediction is confirmed exactly, and this is the first one this session that was neither
unfalsifiable nor ambiguous.** The host census's prediction could not fail; the narrow census's was
ambiguous between a count and a superset, so a disjoint result passed all three falsifiers while
contradicting the claim. This one named the set change.

## The guard is self-demonstrating, and deliberately so

`no_unwind_personality_is_required_and_removing_the_attribute_brings_it_back` **strips the attribute
and nothing else, and requires the symbol to come back.** An assertion that a symbol is absent holds
for a dozen uninteresting reasons — a refused lowering, an empty module, a probe of the wrong shape.
**The mutation is in the test rather than only in a session transcript**, so the causal claim stays
checked rather than becoming folklore.

The probe uses a module that **calls a native**, because a module with no call never needed the
personality and could not distinguish the attribute working from the probe being wrong.

## What is not claimed

**No size or worst-case-execution-time benefit is measured here.** Removing unwind tables should
shrink the object and an opaque runtime call is worse for a bound than an inline expansion, but
neither was measured, so both are directions rather than findings.

**One target for the symbol effect.** The attribute is set unconditionally; the personality routine it
removes was observed on `thumbv8m.main-none-eabihf`. Whether other targets carried an equivalent cost
was not measured.
