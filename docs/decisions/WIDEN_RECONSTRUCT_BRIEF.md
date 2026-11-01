# BRIEF — widen `reconstruct.kel`'s subject set

**Written**: 2026-08-27, seventh loop iteration. **For this line's own use.**

## The goal, and why it is now reachable

The Order-1 gate names its own residual: **`reconstruct.kel` sees ONE subject**, the fewest of any
seeded stage. Every other seeded stage carries three to five.

The recorded obstacle: *"its seed is a parsed multiheaded group and it asserts the subject declares
exactly four heads. The corpus has one such file."* **That is true of the MULTIHEAD path and it is
not the only path.**

**Last iteration established the single-head path works**: `seed_reconstruct_shared` takes
`records: &[(i64, i64)]`, and `ParsedFn::body_records()` returns exactly those and is public. A
comment in `corpus_differential.rs` claimed the single-head form "stays blocked … cannot be built
without the field accessors"; that was true when written, stopped being true, and is now corrected.

**And a survey across every head of every corpus file found reconstruct accepts many** — roughly
twenty `(file, head)` pairs, with node counts from 1 to 21.

## What to do

Add single-head subjects alongside the existing multihead one, taking `reconstruct.kel` from 1 to
several. **Keep the multihead subject**: it exercises the `seed_reconstruct_multihead_shared` path,
which the single-head form does not reach. Exercising both entry points is strictly better than one.

## What "driven" has to mean here, and it is not "the seed applied"

**`reconstruct.kel` REFUSES some inputs by design**, yielding `rc_fail_base() - code` — a negative
tag in the same slot that otherwise carries a node count. Two of three qualifying *single-head files*
are refused with `rc_range_arity`, which is a fact about single-head files rather than a general
acceptance rate.

**So every added subject must be checked to return a POSITIVE node count**, and a refusal must be
reported as a decline rather than counted as a subject. The gate already prints `N subject(s)
seeded, D declined`; a subject that lands in `D` is honest, one that lands in `N` while refusing is
not.

## Prior failures this is exposed to

1. **Counting a seed as a subject because it applied.** "APPLIED, N bytes, K non-zero" says the
   bytes were written, not that the stage consumed them.
2. **The already-holds-the-answer trap** — `analyze.kel`'s `out_valid` was 1 unseeded. Reconstruct's
   observable is its yielded node count and the unseeded run yields something; **know what before
   asserting on it.**
3. **Subject-shopping** — if a chosen subject is refused, report it; do not silently swap.
4. **Sharing a measurement between two changes** — nothing else is in flight, but the widening moves
   the gate's comparison count and that must be re-derived, not assumed.
5. **A truncated read reported as complete** — committed twice this session.
6. **A filter sharing a namespace with its input** — committed twice this session, in opposite
   directions.
7. **Running the two suites in parallel** — invalidates the perf canary. Sequential.
8. **Citing a name that does not exist** — the citation guard has fired on this twice.

## Specific wrong turns to avoid

- **Do not edit `src/selfhost/`** or the other read-only files.
- **Do not remove or replace the multihead subject.** It is the only exercise of that entry point,
  and its "exactly four heads" assertion is deliberate — the handoff says do not delete it.
- **Do not pick subjects by scanning until one passes.** Pick by stated property, then report every
  outcome including the refusals.
- **Do not widen so far that the gate's runtime balloons.** Each subject is sixty ticks against a
  stage; four or five is the shape every other seeded stage has, and matching that is the goal
  rather than maximising.
- **Do not claim the gate is met.** The residual moving does not make the gate met, and this file has
  inflated a headline before.
