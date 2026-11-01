# BRIEF — the tail-yielded composite that lowers with nothing executing it

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Determine what the native side actually yields for a composite | yes |
| 2 | If it can be compared against the reference, compare it | yes, conditionally |
| 3 | Absorb and push once PR #314 clears the red | **not yet — upstream** |
| 4 | Keep the gate green, running the gate rather than `cargo test` | yes |
| 5 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |

## Rationale

The stream frontier work found that **a composite yielded in tail position lowers**, and that
**nothing in the tree executes it**: the suspension differential's subjects all yield `Word`, measured
at zero composite subjects. That is untested lowering in the exact class this line has spent several
increments closing — an arm that exists but has never run.

It is **not** the cross-iteration escape hazard. A tail-yielded composite is built once and no later
iteration overwrites it. It is the marshalling of a composite across the yield boundary, which is a
different thing and equally unexercised.

**What makes this awkward, and why the goal is scoped as a question rather than a harness.** The
native side captures a yield through `kel_yield(v: i64)`. A composite is not an integer, so that
`i64` is some encoding — plausibly an address into the region buffer the harness itself provides.
**Whether it can be decoded and compared against the reference's composite is exactly what is not yet
known**, and the deliverable is that answer, with a comparison if the answer is yes.

## Prior failures to avoid repeating

1. **A test named for a canary firing did not make it fire.** Written this session. **Do not name a
   test for a property its body does not establish.**
2. **A guard can be unfalsifiable by its own precondition**, with the defeater a few lines above.
   From the `v0.2.3` line; the temptation arrives as a bonus net bolted onto a repair.
3. **`stack_growth`/`stack_shrink` are the peak model**, not pop and push counts.
4. **A census surveys a population.** Say which one any figure describes.
5. **A gate run killed by a signal, and another by a tool cap, both reported plausible numbers.**
   Read the exit status; background the long suite.
6. **Seven recorded premises have been found false in consecutive increments**, several of them this
   line's own guesses written down moments earlier. **That is the process working, not failing.**

## Specific wrong turns to avoid

- **Do not decode the yielded value by assuming a representation.** If it is an address, establish
  that it points where expected before reading through it; a wrong assumption reads arbitrary memory
  and produces a plausible number.
- **Do not compare an integer to an integer and call it a composite comparison.** If only the handle
  is compared and not the body, the test proves nothing about marshalling and must not be described
  as if it did.
- **Do not widen anything to make the comparison possible.** If the encoding cannot be decoded from
  the host side, that is the finding.
- **Do not report a partial witness as closing the gap.** If one field is compared and the rest are
  not, say which.
- **Do not push with `--no-verify`** to clear the backlog. The workspace red is real and upstream.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
