# Reverse prompt

## V0.2.X state, 2026-10-30, after the consolidated rulings

The operator asked for an archaeology of every open item on `main` and on the V0.2.X line,
and answered the consolidated list first-hand in this session on 2026-10-30. The list
numbered the items A1 to A5, B1 to B10, C1 to C7, D1 to D4, E1 to E7, F1 and G1. The
B numbers are the questions of the previous reverse prompt.

## Rulings received, 2026-10-30, verbatim

> - A series. Address all open items appropriately.
> - B1. Adopt C. Then attempt to maintain guarantees and widen to A and B together.
> - B2. B now with C later. Give me more information to make a decision after work lands.
> - B5. `auto_arena_capacity_for` needs to return an autosized memory capacity based on
>   WCMU. The value should be suitable for allocation that will work. If capturing
>   composites is inappropriate, then we may need a separate function for that, and we may
>   need to update the cookbook. A function that returns arena plus composite capacity is
>   reasonable if this is the only realistic conceivable use case.
> - B6. Keep them private.
> - B7. A, restore operations. If posssible, it should be possible to conditionally include
>   them if we are looking at pulling text routines in or not.
> - B8. A, the case for literals with compile time refusals. If free is a function to free
>   memory, then we need to talk about this.
> - B9. I need more information.
> - B10. I need more information about the use case.
> - C1. A, refuse at compile time if the bound cannot be computed.
> - C2. Option A.
> - C3. Option B.
> - C4. Option A.
> - C5. Keep the Rust reference for now. Note that we need the review before V0.3.0.
> - C6. Option A.
> - C7. Option B.
> - D1. Option A.
> - D2. Option B.
> - D3. Option A. Message the V0.3.X line so it does the same for the V0.3.X line.
> - D4. Option B.
> - E-series. Send a message to the kaizen line, and I will handle these issues there.
> - F1. Option A.
> - G1. Option A. Message other lines about stale branches.

The lettered options the rulings select, as the list stated them.

- B1 (c) narrows the definite-initialization rule to what the current compiler already
  guarantees. (a) and (b) together are the compiler writing every path and initializers
  for composite private fields.
- B2 (b) is a documented host struct contract with a size check, recommending
  `#[derive(KeleusmaType)]`. (c) is field layouts on the wire, which needs authorization.
- B7 (a) restores the bundled text operations with `Text<N>` signatures.
- B8 (a) is an `as` cast for a literal, refused at compile time when the literal is longer
  than `N`, with free functions for the operations.
- C1 (a) refuses at compile time a module whose bound cannot be computed.
- C2 (a) refuses float opcodes in `verify()` when the `floats` feature is absent.
- C3 (b) designs one wire slot plus a length per array now. It needs authorization before
  any wire-format change lands.
- C4 (a) keeps version granularity continuous, with no tagged release for the milestone.
- C5 keeps the Rust reference, and an independent review of the self-hosted validator is
  required before V0.3.0.
- C6 (a) keeps the primitives in Keleusma as a later opt-in milestone gated on review.
- C7 (b) schedules backlog B42, first-class ring buffers, now.
- D1 (a) holds the release, under the ruling of 2026-10-20 that the version moves only on
  traction.
- D2 (b) pre-authorizes merging green security-only pull requests into `main`.
- D3 (a) requires `CI result` on pull requests into `v0.2.3`, with admins exempt.
- D4 (b) corrects now, on `main`, the four lines flagged under the Appendix B review.
- F1 (a) releases the proof branch, so the proofs line merges `v0.2.3` in, corrects its
  texts and opens its pull request.
- G1 (a) deletes this line's remote branches that are merged into `v0.2.3`.

## Done in this session

To be completed as the work lands.

## Information the operator asked for

To be completed.

## Concerns

- The peer message of sequence 154 relays the same definite-initialization choice from the
  V0.3.X session, "Proceed with option a", which there named the narrowed rule. It agrees
  with B1 and is recorded as a relay.
- Question 3 of the previous reverse prompt attributed the `brace-expansion` fix to pull
  request 630. The kaizen line corrected it in sequence 155. The fix was pull request 619,
  merged into `main` at 02:33 UTC on 2026-10-30.
