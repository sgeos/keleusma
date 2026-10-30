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

- **A series.** Pull requests 642, 643 and 644 merged. Pull request 641 closed, because it
  collapsed the 1.85, 1.88 and nightly toolchain pins again and failed Miri. The Order 5 and
  Order 6 audits no longer read as unruled.
- **D3.** `v0.2.3` requires `CI result` on pull requests, with admins exempt and force pushes
  and deletions refused, as `main` does. V0.3.X applied the same to `v0.3.0` on its own
  operator's confirmation, sequence 161.
- **D4.** Pull request 650 merged into `main` after `main`'s full CI. The private term list
  matches no line of `main`.
- **G1.** Nine merged remote branches of this line deleted. V0.3.X deleted its thirteen,
  tagging the four unmerged ones first. Kaizen and proofs were told of theirs.
- **E series, F1, D2 and D3.** Relayed to kaizen, proofs and V0.3.X, as relays.
- **C1, C2 and stage 0 of the C3 design.** Merged in pull request 652. `compile` refuses a
  module whose memory bound cannot be computed. `verify()` refuses a float opcode without
  `floats`. A private composite slot beyond 65,535, whose placement record the compiler had
  dropped without a diagnostic, is refused at compile time.
- **In pull request 658.** B2's composite input check and the host contract. B5's arena
  sizing for composite inputs, with a new `unsafe` compaction that Miri passes under Tree
  Borrows. B1's narrowed rule and its self-hosted twin. The first `Text<N>` increment, values
  and the literal cast refused at compile time when too long. The kaizen K7 fuzz targets,
  which found a loader panic on a header width exponent of 32 or more, repaired there.
- **Kaizen K3 and K4a.** The slow tests are split per stage and per word type, on branch
  `test/v02-split-slow-tests`, to follow pull request 658.
- **C3 and C7.** `docs/decisions/ARRAY_WIRE_SLOT_DESIGN.md` and
  `docs/decisions/RING_BUFFER_DESIGN.md`. The array design measured that the August work
  already removed most of the 16 MB growth, `lexer.kel` now giving a 28,212-byte artifact.
- **In progress.** The B1 widening, the compiler writing every path with initializers for
  composite private fields, is being attempted.

## Information the operator asked for

### B9, the boundary of the flat Text proof

What is measured or read from the code, as of `v0.2.3` on 2026-10-30.

- A flat `Text` field is two words, a pointer and a length. `read_flat_scalar` in `src/vm.rs`
  rebuilds a string from them with `KString::from_raw_parts`, which is `unsafe`. Its safety
  contract requires that the two words describe a live region that holds valid UTF-8.
- When the script built the body, item 5's proof establishes that contract statically.
- When the body came from outside the program, nothing establishes it at the read. The five
  sources are an entry argument, a resume value, a native's result, a shared slot and a hot-swap
  seed.
- Shared data refuses `Text` at compile time, so the shared slot source does not arise today.
- The other four are packed by the VM itself from a host value tree, in the host's field order.
  The words are a real pointer and length whenever the host's layout matches the script's.

The failure is therefore exactly the host struct defect of B2. Suppose the host passes
`(Word, Text)` where the script declares `(Text, Word)`. Both are three words, so the size check
landed for B2 does not catch it. The script then rebuilds a string from the integer, and in Rust
terms that is undefined behaviour, a read through an arbitrary address. Inference: a host that
follows the contract never reaches it, and a malicious or mistaken host can.

What each option does.

- **(a) A constant-cost range check at the read.** It confirms the region lies inside the arena
  or the bytecode image before rebuilding the string. That keeps every read inside memory the VM
  owns. It does not establish UTF-8, because validating UTF-8 costs time linear in the length.
  An in-range region of arbitrary bytes still breaks the `str` contract. Range plus UTF-8 is
  sound, but it makes each such read cost up to the arena's size, which puts a large term into
  the time bound.
- **(b) Refuse such reads.** That refuses every program that reads a `Text` field of a struct
  parameter, resume value or native result, which is ordinary code.
- **(c) Record a trust assumption.** This is the B2 host contract, already documented in
  `LANGUAGE_DESIGN.md`. It is honest, but the cost of a violation is undefined behaviour.
- **(d), new, field layouts on the wire, which is B2's option (c).** The VM packs these bodies
  itself. With each field's kind on the wire, it could refuse a host value whose field kinds do not
  match at the moment it packs it, in time linear in the number of fields and independent of string
  lengths. That closes all four live sources soundly, with no cost at the read. It needs a wire
  change and your authorization.

My recommendation, an inference to weigh: (d), with (a) as a stopgap until it lands, and (c)
recorded in the meantime, which is now done. If (d) is authorized, B2's remaining reorder case
closes with it, so B2 (c) and B9 become one decision.

### B10, a halting error for hosts

The request came from pykeleusma. I asked that line, in sequence 159, for the concrete use case,
the host exception that must stop the VM rather than be caught, and what the host must observe
afterwards. What is known now:

- Every error a native returns is a soft error that the native-error construct may catch, by
  design, so a script can recover from a host failure.
- A host that wants a failure the script cannot catch has no variant for it.
- The present routes are two. A native can return `InvalidBytecode`, which the construct does not
  catch but which misreports the cause as a malformed artifact. Or the host can fail outside the
  VM call.

I will bring pykeleusma's answer to you when it arrives. The decision then is between a new
variant, made non-exhaustive first so that adding it does not break a host's exhaustive match,
and documenting the existing route.

### B8, "free functions"

"Free functions" meant functions that are not methods, `text::concat(a, b)` rather than
`a.concat(b)`. It has nothing to do with freeing memory. A `Text<N>` is a flat value of exactly
`word_bytes + N` bytes. A local is reclaimed with the iteration's arena at `Reset` like every
other value, and a data field lives in its region. There is no `free`, and none is proposed,
because an explicit deallocation would make memory use depend on the program's run-time path,
which the worst-case memory bound exists to exclude.

### B7, a decision the restored operations need

Native declarations cannot be generic today. `NativeSignature` in `src/ast.rs` carries concrete
parameter and return types, and a native's memory cost is attested per native, not per call
site. A concatenation typed `(Text<A>, Text<B>) -> Text<A + B>` therefore has no way to be
declared.

- **(a) Generic native declarations over const parameters.** For example,
  `use text::concat<const A: Word, const B: Word>(Text<A>, Text<B>) -> Text<A + B>`. The type
  checker instantiates it at each call, and the result size, known at the call site, enters the
  memory bound there. The Rust implementation reads sizes from the bodies, so one host function
  serves every capacity. No opcode is needed, and the native table records each instantiation's
  return shape. This is a language extension, and I recommend it.
- **(b) Compiler intrinsics.** The operations lower to existing opcodes. Since no opcode writes
  one byte of a flat body, each result is built whole with `NewComposite`, so code size grows
  with `N`. It needs no native and no library.
- **(c) One native per capacity.** For example, `text::concat_8_8`, declared and registered per
  instantiation. It is simple and verbose.

For conditional inclusion, I propose that the operations form a bundled `text` library behind a
cargo feature, as `math`, `audio` and `shell` already are. A script imports them with
`use text::...`, and a build without the feature refuses the import at compile time.

## Decisions waiting on the operator

Each has lettered options with my recommendation first.

1. **B9 and B2 (c) together.** (d) Field layouts on the wire, which closes the reorder case
   and the flat `Text` boundary for every live source and needs a wire change. (a) A range
   check at the read as a stopgap. (c) The documented trust assumption alone, as now.
2. **B7, typing the restored operations.** (a) Generic native declarations over const
   parameters. (b) Compiler intrinsics. (c) One native per capacity.
3. **B10.** Waits on pykeleusma's use case, requested in sequence 159.
4. **The `Text<N>` brief's wire item.** `TEXT_N_IMPLEMENTATION_BRIEF.md` records that shrinking
   a flat `Text` field to one address should land with `Text<N>` or wait for a new version.
   (a) Defer it with the version. (b) Authorize it now.
5. **The C3 design's eight decisions**, in `ARRAY_WIRE_SLOT_DESIGN.md`. Decision 8, stage 0,
   is done as its option (a).
6. **The C7 design's seven decisions**, in `RING_BUFFER_DESIGN.md`. D1 should follow the
   answer to item 2 above.

## Concerns

- The peer message of sequence 154 relays the same definite-initialization choice from the
  V0.3.X session, "Proceed with option a", which there named the narrowed rule. It agrees
  with B1 and is recorded as a relay.
- Question 3 of the previous reverse prompt attributed the `brace-expansion` fix to pull
  request 630. The kaizen line corrected it in sequence 155. The fix was pull request 619,
  merged into `main` at 02:33 UTC on 2026-10-30.
- Two compiler defects were found and not fixed. `let (a, b) = 5` compiles and faults at run
  time, and `let r = yield ...` without an annotation faults when a field of `r` is read.
- The fuzz package's own `Cargo.lock` is guarded by neither CI nor Dependabot, which is the
  kaizen line's to decide.
- The CLI test `shipping_validator_reaches_every_current_stage` failed once and passed on a
  rerun in the twin agent's run, with its output not captured.
- GitHub refused the job-log API with an account egress limit on 2026-10-30, so a CI failure
  of pull request 652 was diagnosed by reproducing it locally.
