# BRIEF — absorption 16, and the fifth blocker check

**Written**: 2026-08-27, sixteenth loop iteration. **For this line's own use.**

## Goal 1 — absorption 16 (#297)

The `v0.2.3` line built a **citation guard for process documents** — the same shape as this line's
`comment_citations`. **No `.kel` changes**, so **predict every census unchanged**, `bound_transfer`
at **1058**. Measure alone anyway; the prediction is the point of recording it.

## Goal 2 — is `Len`'s blocker real, or the fifth to expire?

**Four recorded blockers expired this session.** `Len` is the last named refusal in the backend and
its blocker is the one that has looked most solid. **Checking it is how the sweep earns its
conclusion**: if every blocker checked turns out stale, the sweep found rot; if this one holds, the
sweep found four specific failures rather than a universal condition.

### The recorded argument, and it is a good one

`probe_len_reachability.rs` states it structurally, not incidentally:

> *"`Op::Len` fires exactly when the for-in source has no statically known length. A loop whose trip
> count is not statically known is exactly what the bound extractor refuses. **The property that
> makes the opcode reachable is the property that makes the loop unbounded.** They are not two
> independent limitations that might be lifted separately."*

And it rules out the obvious objection by measurement — `both_arms_same_length_is_still_refused`
gives both arms length two, so the trip count is provable by inspection, **and it is refused anyway.**

### The specific thing that could have invalidated it

**The `for .. limit <const>` form.** `GRAMMAR.md`: *"A range over **runtime** endpoints is admitted by
supplying the bound explicitly with a `limit` clause."*

**That is exactly the decoupling the structural argument says cannot happen** — a loop whose trip
count is not statically known, admitted anyway, because the cap is supplied separately. **The probe
predates the form and does not mention it.**

**So the question is narrow and testable**: can a `limit` clause attach to a for-in whose SOURCE has
no static length — not merely to a range with runtime endpoints? If yes, a `Len`-emitting program may
be admissible and the blocker has expired. If no, the argument survives a real attack rather than
merely going unchallenged.

## Prior failures this is exposed to

1. **Assuming a blocker is stale because four others were.** The prior from this session cuts both
   ways and must not become the conclusion.
2. **Assuming it holds because it is well written.** That is how all four survived.
3. **Testing the wrong construct.** `limit` on a *range* is documented; the question is `limit` on a
   *source with no static length*. Those are different and conflating them answers nothing.
4. **A vacuous check.** Eleven guards or filters broke this session. If the probe program does not
   emit `Op::Len`, nothing downstream means anything — **assert the opcode is present** before
   reading any admission verdict.
5. **Reporting a figure without the command that produces it.**
6. **Running the two suites in parallel** — invalidates the perf canary. Sequential.

## Specific wrong turns to avoid

- **Do not edit `src/` or any read-only file.** If the answer is that the compiler must change, that
  is a finding for the other line.
- **Do not lower `Len` in this increment even if the blocker expires.** Establishing admissibility is
  the goal; lowering is its own increment with its own differential — `FixedDiv` took a full one.
- **Do not weaken the existing probe's claims.** If the limit form changes the picture, add the
  finding beside them and say what changed; the structural argument was correct for its time.
- **Do not report "refused" without saying WHICH stage refused it.** Parse, typecheck, verify and
  the bound extractor are different answers with different implications.
