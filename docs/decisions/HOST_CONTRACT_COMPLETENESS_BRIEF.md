# BRIEF — the host contract, stated completely

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-11.

## The gap

The entry takes three pointers. The generated header states the layout of **one** of them:

| buffer | published in the header? | what the shipped C host does |
|---|---|---|
| shared segment | **yes** — `KEL_SHARED_BYTES` and a named offset per slot | sizes from the macro |
| private/persistent region | **no** | `int64_t private_region[8]`, a guess |
| composite region | **no** | `int64_t composite_region[64]`, a guess |
| the private initial image | **no** | `memset(..., 0, ...)` |

The figures exist — `required_persistent_capacity_for` plus `persistent_supplement_bytes`,
`host_arena_supplement_bytes`, and now `private_init_image`. **They are published in Rust to a host
that is written in C.**

## Why this is worth an increment rather than a note

The weakest part of this backend's design is the class of obligation it can only state. Its own
documentation concedes it: *"a figure the host must remember to add is not one the runtime's sizing
function includes, and publishing the figure does not change that."*

**The shipped example is the one artefact that turns a stated obligation into a demonstrated one.**
Today it demonstrates guessing. A host programmer copying it learns to size two of three buffers by
eye — and `policy.kel` happens to declare no private data, so the guess is currently harmless and
would stop being harmless the moment the example grew a `private data` block.

## The wrong turns

1. **Do not hand-write the figures into the header.** They are derived from the module, like
   `KEL_SHARED_BYTES` — the header's own banner says the layout "cannot drift from the code it
   describes", and a transcribed number would make that banner false.
2. **Do not emit an image for a module that has none.** An empty array is not valid C, and a host
   memcpy-ing zero bytes from a zero-length array is a construct to avoid rather than to generate.
3. **Do not size the composite region from the entry chunk alone.** `region_total_bytes` is
   transitive; a call site receives a disjoint block of the caller's region, so the entry needs
   everything it can reach. The existing harness already learned this.
4. **Do not assume the example proves the figures are right.** `policy.kel` has no private data and no
   composite construction. The test must state what the example does NOT exercise, or it claims
   coverage it does not have.
5. **Do not break the existing agreement test.** It builds, links and runs the example against the
   reference. If a C compiler is absent it must stay skippable rather than become a hard failure.

## What done looks like

The header states every buffer the entry requires and the initial image when there is one; the
shipped host sizes from the header instead of guessing and installs the image; a test asserts the
header's figures equal the published functions; and what the example does not exercise is written
down beside it.
