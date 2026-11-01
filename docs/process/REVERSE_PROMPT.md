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

## Rulings received first-hand, third list, 2026-10-31, verbatim

A third consolidated list of decisions and concerns was answered first-hand in this session.

> 1. A. Yes, when an overflow is possible or guranteed.
> 2. A. Runtime batch length should eventually be supported subject to WCET and WCMU analysis
>    feasibility. A ring buffer has fixed memory, so my understanding is that this is gated on
>    WCET.
> 3. Later. I will address this once your line is otherwise idle. I plan to pull the V0.2.X work
>    and V0.3.X work together and address loose ends before a release.
> 4. B.
> 5. B.
> 6. B.
> 7. A.
> 8. Recommendation accepted.
> 9. Recommendation accepted.
> 10. Recommendation accepted.
> 11. Acknowledged.
> 12. Acknowledged.
> 13. Acknowledged.

> Re: Concerns.
>
> - We already know that contract violations can cause problems.
> - The goal of `auto_arena_capacity_for` is to right-size an arena automatically. If the current
>   implementation does not do this, then fix it so it does. This might mean you need another
>   function or two to handle other use cases.
> - Yes, we want to stick to defined results. If it will work for [a deployment context withheld
>   under Appendix B], the solution is likely acceptable.
> - We want full language coverage, including the pipe operator.
> - Aknowledged.
> - Probably an update in progress.

One phrase in the third concern is replaced by a bracketed reference, because the words name
context that Appendix B keeps out of tracked files. The rest is verbatim.

| Item | Question | Ruling |
|---|---|---|
| 1 | Ring batch push B1, a required `overflow` arm | (a), required when an overflow is possible or guaranteed |
| 2 | Ring batch push B2, a run-time batch length | (a) for the first pass, a run-time length later where the time bound allows |
| 3 | When to cut V0.2.3 | Later, when this line is otherwise idle, together with the V0.3.X work |
| 4 | A resume value whose type only a later use fixes | (b), feed the type checker's types to monomorphization |
| 5 | Nested struct and single-variant enum `let` in the self-hosted compiler | (b), extend the stages now |
| 6 | The local `archive/*` tags | (b), recreate with the noreply address and push |
| 7 | Stale worktrees of other lines | (a), tell the owners, the operator unlocks the locked one |
| 8 | A unit variant pattern matching a `Word` scrutinee | Refuse it |
| 9 | A delegated callee yielding another type than the entry | Refuse it at compile time |
| 10 | Clippy under `--no-default-features` on tests | Gate the dead code |

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

Later on 2026-10-31, pull requests 687, 690 and 691 merged, the rulings record, the private read
alias with a VM overlap fix that Miri under Tree Borrows passed, and the fuzz corpus editorconfig
exemption. Item 6 is done, with the four `archive/*` tags on GitHub, three of them recreated
with the noreply address and identical trees.

## In progress

- Pull request 694, refutable `let` refusal, struct destructuring, pipes in the self-hosted
  compiler and the monomorphized resume type.
- Items 4, 8, 9 and 10, and pipe resolution under shadowing, on `fix/v02-typecheck-followups`.
- The `Text<N>` method surface with fully defined results for malformed values, on
  `feat/v02-text-methods`.
- Item 5, nested and single-variant enum `let` patterns in the self-hosted stages, on
  `feat/v02-selfhost-let-patterns`.

## Next, in order

1. Right-size `auto_arena_capacity_for` for the virtual machine route again and add a function
   for the native route, on the operator's concern ruling, once V0.3.X states what a native-only
   host allocates. Its request is sequence 186.
2. Encoding F, the per-field schema hash, option B addressing, the field-based `DataLayout`, then
   packed private storage S1, under `ARRAY_WIRE_SLOT_DESIGN.md`.
3. The ring buffer, with the batch push and a required `overflow` arm where overflow is possible
   or guaranteed.
4. The C5 review of the self-hosted validator.
5. The release, which the operator takes up once this line is otherwise idle.

## Decisions waiting on the operator

None is open on this line. V0.3.X's native header figure was answered first-hand in that line's
session as its question 28, option (a), reported in sequence 184, so the item is dropped.

## Concerns

- Item 1 leaves the field-order contract to the host, which the operator accepted as a known
  property of contract violations. A reordered `Text` field read crosswise is
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
