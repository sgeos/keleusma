# Brief — the callee summary, the confinement analysis's last increment

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Written 2026-08-24 for autonomous execution, the iteration after `src/confine.rs`
landed. The handoff named this increment and priced it: *"the call graph is
acyclic, so a bottom-up summary terminates without a fixpoint."*

## The goal, and the number that measures it

Today every composite passed to a Keleusma `Call` yields `CannotEstablish`,
because nothing knows what the callee does with it. A summary answers that, so
a call that provably cannot leak its argument stops disqualifying the site.

**Measured before designing, not after.** All **four** `CannotEstablish`
verdicts in the flat corpus are `PassedToCall`, all in
`examples/scripts/10_multbyte.kel`. So this increment addresses **the entire
remaining class**, and the corpus count is the honest scoreboard: 33 sites /
17 confined / 12 escapes / **4 cannot-establish**.

**Be precise about what that does and does not show.** All four sit at
`Scope::Invocation`, not inside an iterating loop. **This increment therefore
changes no per-iteration verdict on the current corpus.** It closes a class and
it makes the predicate useful on programs that pass composites to helpers,
which is ordinary code. Claiming it improved per-iteration confinement would be
false.

## What the callee can actually do with an argument

Derived from the route classification rather than from imagination:

| route in the callee | leaks the argument? |
|---|---|
| `Yield` | **yes** — the host holds the handle |
| `CallExternalNative` / `CallVerifiedNative` | **yes** — trust boundary |
| `Call` to another chunk | **transitively**, and the graph is acyclic |
| `Return` | **no, but the caller must know** — the returned value may ALIAS the argument, and the caller already tracks what it does with a return value |
| `SetLocal` | **no** — the callee's frame dies at return |
| `SetData` / `SetDataIndexed` | **no** — the bytes are copied |

So the summary is two per-parameter facts: *may this parameter's region leak*,
and *may the return value alias it*. Both are needed. A summary carrying only
the first would force the caller to treat every return as aliasing every
argument, which is what it already does.

## The specific wrong turns

**Do not reuse `boundary_dead` for a parameter.** A parameter's slot is written
by the CALLER during frame setup, so the first `GetLocal` on it is a
read-before-write and the rule would report every parameter as live across the
boundary. Parameters and construction sites are different kinds of thing and
must not share a token space by accident — that is what a typed token buys.

**Do not write a second walk.** The escape routes must be followed identically
whether the question is about a site or a parameter; two walks drift, and the
drift is silent because each passes its own tests. **`route_of` and the transfer
function are the shared part.** If generalising them is awkward, that is a cost
worth paying once.

**Do not let a missing summary read as a clean one.** An unresolvable callee —
outside the module, or a chunk index the caller cannot see — must be
*conservative*, and must be conservative in the same direction as today. A
summary defaulting to "does not leak" turns a sound analysis unsound in exactly
the way that is hardest to notice, because the verdict improves.

**Do not assume the call graph is acyclic; the language guarantees it, and the
code should still terminate if the guarantee is wrong.** A cycle must produce a
conservative summary, not a stack overflow.

**Do not narrow the existing API.** `chunk_confinement` is public and tested. A
chunk analysed without summaries must keep answering exactly as it does now.
The summary path is an addition.

**Do not update the corpus count without saying which column moved.** The count
test exists so that a change is deliberate. If `cannot-establish` falls to
zero, `confined` rises by the same amount and nothing else may move.

## The failure this tree keeps repeating, in the form it took last iteration

**Three defects in the last increment were found by measurement and none by
review**: a mutation that compiled and still proved nothing because it shifted
every jump target; corpus counts written from memory; and an out-of-range slot
that under-approximated. **The pattern is that reasoning about the analysis
found nothing and running it found everything.** Write the probe before the
assertion, and make every new test fail on purpose with a change that compiles.
