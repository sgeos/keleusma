# Brief — the unwind personality requirement, and whether it should exist

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Drafted 2026-09-01**, from an open question this line recorded
rather than explained.

---

## The goal set

| goal | owner | state |
|---|---|---|
| **G5** the unwind personality requirement | this line | **unblocked, and the subject of this brief** |
| **G2** absorption 43 | this line | still gated — a measured absorption needs a solo run and the peer's release gate is still running |
| the runtime arithmetic width, `f16`, `Text<N>`, `Opaque` | `v0.2.3` / operator | not mine |

**This is the first goal in several iterations that changes the BACKEND rather than measuring it.**
The last three produced instruments and records; this one, if the finding holds, removes a link
requirement from every object the backend emits for an ARM target.

## The finding this comes from

[`NARROW_TARGET_LINKAGE.md`](./NARROW_TARGET_LINKAGE.md) measured eleven toolchain symbols required on
`thumbv8m.main-none-eabihf`, and flagged one as an open question rather than explaining it:

> *"`__aeabi_unwind_cpp_pr0` is the one to look at first. An unwinding personality routine is an
> unexpected requirement for a language with no exceptions, on a target with no unwinder. Its origin
> is **not determined here**."*

**Read since**: `native_codegen/src/lib.rs` sets **no function attributes at all** — no `nounwind`, no
personality, nothing. So LLVM must assume every emitted function may unwind, and the ARM backend
emits the unwind machinery that reference implies.

**That is a hypothesis, not a finding.** It is consistent with the evidence and it has not been
tested. The wrong turn here is to treat "the attribute is absent and the symbol is present" as
causation, implement the fix, see the symbol disappear, and call the hypothesis confirmed — because
the same observation would follow from several other causes that the attribute happens to mask.

## What is actually being asserted, because it is a contract change and not a flag

Marking an emitted chunk `nounwind` asserts **no unwind ever propagates out of it**. A chunk calls
host natives, so the assertion covers them too.

**The justification, stated so it can be disputed:** natives are `extern "C"`. Unwinding out of an
`extern "C"` boundary is already undefined in C and aborts in Rust, so a native that unwinds is
outside the host contract before this change and stays outside it after. **The attribute makes an
existing constraint explicit rather than adding one.**

**But it must be said in the host-facing documentation, not only in a commit message.** An embedder
who today writes a native that throws is relying on behaviour that was never promised; after this
change, they get miscompilation instead of a crash. **The wrong turn is to land the attribute and
leave the contract undocumented.**

**Do NOT mark the external native declarations `nounwind`.** That would be a claim about code this
backend does not generate. Mark only the functions it defines.

## The prediction, with falsifiers named — and written to avoid the last two mistakes

The host census's prediction *could not fail*. The narrow census's prediction was *ambiguous between a
count and a superset*, so a disjoint result passed every falsifier while contradicting the natural
reading. **This one names the exact set change.**

**Predicted: on `thumbv8m.main-none-eabihf`, the toolchain set loses `__aeabi_unwind_cpp_pr0` and
nothing else. It goes from eleven symbols to ten, and the remaining ten are unchanged as a set. The
host set is unchanged at two.**

**Falsifiers, any one of which refutes it:**

1. `__aeabi_unwind_cpp_pr0` is still required after the attribute is set.
2. Any symbol other than that one enters or leaves either set.
3. Any existing differential disagrees, at any target, after the change.

**Falsifier 3 is the important one.** The first two concern a symbol table; the third concerns whether
the program still computes the same answers, and it is the only one that can catch the attribute being
wrong rather than merely ineffective.

## The specific wrong turns, from failures recorded earlier today

**1. Verifying at the wrong level.** Twice: an IR search for a fused multiply-add that could not see
codegen, and an IR-level symbol census that would have missed compiler-runtime calls entirely. **An
attribute is visible in IR, but its EFFECT is only visible in the object.** Check both, and treat the
object as the answer.

**2. A test verified alone is not verified.** The host census passed five tests under a single thread
and failed two in the suite. **Run under ordinary parallelism before believing it.**

**3. An instrument that reports a wrong answer confidently.** Two defects in the narrow census each
produced a confident wrong verdict, and both were caught only because the comparison PRINTS its
inputs. **Print the before and after sets, not just the verdict.**

**4. Do not infer a cause from a neighbouring absence.** The backend sets no attributes and the symbol
appears; that is correlation. **Establish the link by changing exactly one thing and re-measuring, and
say plainly if the symbol persists.**

**5. Do not edit test sources while a suite runs.** Broken once today on this line.

## What would make this work worthless

**Landing the attribute without re-running the differentials.** The symbol table getting shorter is
the cheap half. Whether every corpus module still agrees with the virtual machine is the half that
matters, and `nounwind` licenses LLVM to delete code it believes unreachable through an exceptional
edge. **A shorter symbol list and an unmeasured behaviour change would be a bad trade.**
