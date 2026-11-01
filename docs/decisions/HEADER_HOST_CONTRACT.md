# The generated header states the host contract, and the obvious claim about it is false

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Built and measured 2026-09-01.**
Brief: [`HEADER_HOST_CONTRACT_BRIEF.md`](./HEADER_HOST_CONTRACT_BRIEF.md).

---

## The gap

[`LINKAGE_SYMBOL_CENSUS.md`](./LINKAGE_SYMBOL_CENSUS.md) found that **43 of 45 undefined symbols
across the corpus are host-registered natives**. The generated header declared **none** of them, so an
embedder met the requirement at link time as a mangled undefined symbol — while the backend had known
the whole list at compile time.

## ⚠ WHAT THE DECLARATIONS BUY IS NOT WHAT I WROTE FIRST

The first version of the generated comment said that declaring the natives makes **omitting** one a
compile error. **That is false, and it was nearly shipped into an artefact embedders read.**

Measured with a C compiler, all three cases:

| case | result |
|---|---|
| missing definition, declaration present | **compiles clean** — still a link error |
| wrong arity, no declaration | **compiles clean** — garbage crosses the ABI |
| wrong arity, declaration present | **`conflicting types`** |

**So the guarantee is about a MISMATCHED definition, not a missing one** — and that is the more
valuable one. It makes the boundary symmetric: the backend already refuses a module whose two call
sites disagree on a native's arity, on the ground that *"LLVM would accept the call and the host would
read a garbage argument"*. The host side now checks the same thing.

**The completion condition is what caught it**, by demanding a demonstration rather than a claim.
A property phrased as "an omitted native fails earlier, and this is shown" cannot be satisfied by
reasoning.

## Where the list comes from, and why not from the names

**Arity exists nowhere but the lowered module.** A `Module` records native NAMES and return shapes;
the argument count comes from the call sites and is resolved during lowering.

The brief proposed exposing the backend's name-mangling function so the header could call it.
**Implementing found a better source**: read the declarations off the module about to be written to
the object. The header is then derived from the artefact rather than computed alongside it, and
**cannot drift, because there is nothing to drift from.**

Building a prototype from the name alone would have been the invented signature the brief forbids,
and C would have accepted the mismatched call.

## Scope, stated because omitting it would be the day's recurring error

**This is the HOST half only.** A linked binary also needs compiler-runtime and C-library symbols —
two on this host, eleven on `thumbv8m.main-none-eabihf` — which an embedder does not declare. The
generated comment says so and points at the censuses. See [`SCOPE_DELETION.md`](./SCOPE_DELETION.md).

## Structure

`host_native_declarations` is public and lives in the library, so the example and the test share one
implementation rather than two. `NATIVE_SYMBOL_PREFIX` now has a single spelling, used by the mangler
and the extractor — **two copies of that string is the drift class this package spent the day
removing** from five refusal messages.
