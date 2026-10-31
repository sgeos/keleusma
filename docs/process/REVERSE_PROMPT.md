# Reverse prompt

## V0.2.X state, 2026-10-31, after the second consolidated list

The operator asked on 2026-10-31 for every outstanding issue on `main` and on the V0.2.X line
that needed a decision or attention, with the obvious choices. A list of twenty items was
presented and answered first-hand in this session. Later answers in the same session settled
relayed rulings, the questions the work raised, and the array and ring designs. The record of
2026-10-30 is in git history and in `TASKLOG.md`.

## Rulings received first-hand, 2026-10-31, verbatim

The twenty-item list. The numbers are that list's, recorded beside each item below.

> 1. C for now. Reordering without updating the Keleusma code is a contract violation.
>    Underlying Keleusma needs to be replaced or reloaded with a patched version before
>    reordered data is passed in.
> 2. B.
> 3. B.
> 4. B.
> 5. B.
> 6. A.
> 7. A.
> 8. A.
> 9. A.
> 10. A.
> 11. A.
> 12. B. Infer if possible. Refuse when impossible.
> 13. A.
> 14. A.
> 15. A.
> 16. Yes.
> 17. Prune stale branches, local and remote.
> 18. A. If anything needs to be done manually, I can work with you on that.
> 19. A.
> 20. Yes.

What each number named, so that the bare letters can be read.

| Item | Question | Option chosen |
|---|---|---|
| 1 | B9 with B2 (c), host data the layout proof cannot cover | (c), the documented host contract |
| 2 | B7, typing the restored text operations | (b), compiler intrinsics |
| 3 | B10, a halting error for hosts | (b), `VmError` non-exhaustive with `HostHalt` |
| 4 | The `Text<N>` wire item, the collapse to one address | (b), authorize now, later replaced, see below |
| 5 | The array design's decisions | (b), answer them now |
| 6 | The ring design's D1 to D7 | (a), answer item 2 first, then the rest |
| 7 | Two behaviour changes of the B1 widening | (a), keep both |
| 8 | Publishing V0.2.3 | (a), keep holding |
| 9 | A handled write's `old` observing the new value | (a), fix now |
| 10 | Rule 2's self-hosted twin | (a), implement |
| 11 | `let (a, b) = 5` | (a), refuse in the type checker |
| 12 | An unannotated `let r = yield` | (b), infer, refuse when impossible |
| 13 | `tests/hostile_module_mutation.rs` without `floats` | (a), gate on `floats` |
| 14 | The `selfhost_typecheck` over-rejection | (a), isolate and report |
| 15 | The flaky CLI validator test | (a), watch and capture |
| 16 | The C5 self-hosted review before V0.3.0 | Yes |
| 17 | Stale branches | Prune, local and remote |
| 18 | Stale agent worktrees | (a), remove |
| 19 | Kaizen's stale records | (a), send a correction |
| 20 | The items held in the kaizen session | Yes |

Relayed rulings, each confirmed first-hand through lettered options in this session.

| Relay | Ruling relayed | Confirmed as |
|---|---|---|
| V0.3.X, sequence 172, question 22 | "b. Fold it in" | "Yes, fold it in" |
| V0.3.X, sequence 172, question 27 | "a. Both lines", V0.2.X owning the C5 review | "Yes, V0.2.X owns it" |
| Kaizen, sequence 173 | C7 D2, D3, D5, D6 and D7 as (a) | "Yes, confirm all five" |
| Kaizen, sequence 173 | Item 4 as (a), defer | Conflicted with item 4 above. The operator answered "B, authorize now" |

The collapse was then put back to the operator once its premise proved false, and the answer
was "(c) Defer collapse". Dynamic bare `Text` is still produced by natives returning `String`,
by the shell library and by host composite inputs, so a lone address cannot carry the length.

Questions the work raised, answered through lettered options.

| Question | Answer |
|---|---|
| A plain read of a private composite aliases its pool body | "(c) Copy only when needed" |
| `let 3 = 5` compiles and matches nothing | "(a) Refuse refutable let" |
| A struct destructuring `let` is refused | "(a) Support it" |
| The self-hosted compiler refuses pipes | "(b) Add pipes to self-host" |

The array design, `ARRAY_WIRE_SLOT_DESIGN.md`, and the ring design, `RING_BUFFER_DESIGN.md`.

> C3.1. Explain encodings F and R.
> C3.2. A. Authorize without version bump once a decision has been made.
> C3.3. Fingerprints (and bytecode versions) are only updated when public releases are cut. We
> are approaching a new public release, so a bumps are premature.
> C3.4. A.
> C3.5. A.
> C3.6. A, option B.
> C3.7. A.
>
> D1. A. Switch `Text<N>` to match.
> D4. A. Also, we want some sort of overflow arm if it is possible to batch push something like
> 6 elements into a 4 element ring buffer.

After the explanation of the two encodings, "Proceed with encoding F."

## Done, 2026-10-31

| Pull request | Merge | What it carries |
|---|---|---|
| 677 | `d9fb3a5f` | Item 3, `VmError::HostHalt` and a non-exhaustive `VmError`, and item 1's contract wording |
| 679 | `2065aa3e` | Kaizen N4, the shipping validator test split per stage |
| 680 | `bf5b80cd` | Question 22, the native region supplement in `auto_arena_capacity_for` |
| 682 | `86069615` | Item 2, the text operations as intrinsics behind the `text` feature |
| 684 | `29649eee` | Items 9, 11, 12 and 13, and the piped generic call of V0.3.X sequence 171 |
| 685 | `35955511` | Items 10 and 14, and the lone-join names of V0.3.X sequence 170 |

`v0.2.3` was green after each merge that combined untested pairs, at `86069615` and at
`35955511`. Items 17 to 19 are done. Fourteen remote and 180 local branches were pruned,
nineteen stale worktrees were removed, and three unmerged branches and one dirty snapshot are
kept as local `archive/*` tags. GitHub refused the tags because those commits carry the
private author email, so they exist only in the shared local repository.

## In progress

- The private read alias, option (c), on `fix/v02-private-read-alias`.
- Refutable `let` refusal, struct destructuring in `let`, pipes in the self-hosted compiler, and
  V0.3.X sequence 181's unmonomorphized call on an unannotated resume value, on
  `feat/v02-let-patterns-pipes`.

## Next, in order

1. Integrate the two branches above.
2. Switch the `Text<N>` operations to method calls, `a.concat(b)` and the like, replacing the
   free functions of pull request 682, as D1 directs.
3. Encoding F, the per-field schema hash, option B addressing, the field-based `DataLayout`,
   and then packed private storage S1, under `ARRAY_WIRE_SLOT_DESIGN.md`. No fingerprint or
   version moves before the release is cut, per C3.3. Artifacts inside the repository built
   under the old layout are regenerated in the same change, since nothing is published.
4. The ring buffer, after the batch-push overflow arm of D4 is written into
   `RING_BUFFER_DESIGN.md` for review.
5. The C5 review of the self-hosted validator, once the self-hosted changes above land.

## Decisions waiting on the operator

1. **The batch-push overflow arm**, once it is written into the ring design.
2. **V0.3.X's native header figure.** V0.3.X keeps `<prefix>_region_bytes` for a C host that
   cannot call `auto_arena_capacity_for`, and put that choice to the operator in its own
   session. That line documents that the two figures must not be added together.

## Concerns

- Item 1 leaves the field-order contract to the host. A reordered `Text` field read crosswise is
  undefined behaviour, which only the documented contract excludes.
- The fold of question 22 means `auto_arena_capacity_for` is no longer the virtual machine's
  byte-exact minimum. A host that runs only the virtual machine is over-provisioned by the
  native supplement.
- The native supplement formula in `src/native_region.rs` is a copy. The native backend stays
  the authority, and V0.3.X's differential is what catches drift.
- The `text` intrinsics trust a host-supplied `Text<N>` to keep its length at most `N` with zero
  bytes past it. A violation gives a defined but unspecified result, with every read
  bounds-checked.
- The pipe fix does not handle a local closure that shadows a generic function's name.
- The test-side self-hosted typecheck driver keys names by spelling across the whole program,
  pinned as a known over-rejection.
- Git crashed once on 2026-10-31 because a MacPorts library was briefly missing. The cause is
  unknown, and the interrupted step completed on rerun.
