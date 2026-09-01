# Brief — make the generated header state the host contract it already knows

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Drafted 2026-09-01.**

---

## The goal set

| goal | state |
|---|---|
| **G10** the generated header declares the natives the host must supply | **unblocked, and the subject of this brief** |
| absorption 44 | gated on the `v0.2.3` gate, which is still running; it carries the arithmetic width |
| `f16`, `Text<N>`, `Opaque`, publication | not mine |

**One actionable goal is the honest count.** Everything else is waiting on another line or on the
operator.

## The gap, measured

[`LINKAGE_SYMBOL_CENSUS.md`](./LINKAGE_SYMBOL_CENSUS.md) established that **43 of the 45 undefined
symbols across the corpus are host-registered natives** — the embedder's half of the contract.

**The generated header declares none of them.** `emit_object.rs` writes an entry-point prototype and
the shared-slot offsets, and does not mention natives at all. Measured: the string `native_names` does
not occur in that file.

**So an embedder whose policy calls a native discovers it at LINK time, as an undefined symbol with a
mangled name.** The backend knew the whole list at compile time and did not say.

**The roadmap's criterion is that native artefacts "link as static libraries against a host".** A
header that omits half the contract does not meet that; it defers the contract to the linker.

## What the fix must do, and the one thing it must not

**Emit `extern` declarations for the natives, from the same function the backend uses to mangle
them.**

> ⚠ **THE EXAMPLE MUST NOT RE-IMPLEMENT THE MANGLING.** `native_symbol` is currently private, so the
> straightforward route is to copy the rule into the example. **That is the exact failure class this
> line spent the day removing**: five refusal messages restated one fact and every one drifted when
> `f32` landed. A second copy of the mangling rule drifts the day the rule changes, and the symptom
> would be a header that declares a name no object defines.

So `native_symbol` becomes public. **The symbol name IS the host contract**, so it belongs in the
public surface rather than being an implementation detail — that is the argument for exposing it, not
convenience.

## Why this is worth more than it looks

**It converts a link error into a compile error.** Today a missing native is `undefined symbol:
kel_native_host__tick` from the linker, after a successful compile of the host. With declarations in
the header, a host that omits the definition fails in its own translation unit, at the line that
should have defined it, under the name the author wrote.

It also makes the contract **readable**: an embedder can see what a policy demands of them by reading
one generated file, rather than by running `nm` on an object.

## The wrong turns

**1. Do not hand-write the declarations for the example's own policy.** `motor_policy/policy.kel` uses
**no natives**, so a hand-written list would be empty and would prove nothing. The test needs a module
that actually declares one — `external_native_witness.kel` does.

**2. Do not guess the C signature.** A native's arity and types come from the module, not from the
name. If the shape is not available, declare what is known and say what is not, rather than inventing
a plausible prototype — a wrong prototype is worse than none, because C will accept the call and pass
the wrong thing.

**3. Do not claim the header now covers the whole contract.** It covers the **host** half. The
toolchain half — `__divti3`, `bzero`, and eleven others on a bare-metal target — is not something an
embedder declares, and the census records it separately. **Saying "the header states the contract"
without that qualifier would be scope deletion**, six instances of which this pair of lines produced
today.

**4. Do not edit test sources while a suite runs.** Broken once today.

**5. Verify by compiling, not by reading.** A header is a claim that a C compiler accepts it. The
existing example already links a C host; the new declarations must survive that, and a test that only
greps the generated text has not checked the thing that matters.
