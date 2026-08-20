# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-19 (session 49, ruled refusals)

## Where things stand

| | |
|---|---|
| all twelve stages | `loop main(...)` coroutines |
| emit path | 11 of 11 stages; every emit-side cap removed |
| `lexer` into `parse` | FUSED, one-token window, byte-identical on four stages |
| architecture | one binary, selectable phases — documented, unbuilt |
| **`parse.kel` capacity diagnostics** | **four causes now NAMED; the rest still trap raw** |
| **the last cap** | **GONE. `wire.kel` PARSES, 486 functions** |
| **`parse` into `reconstruct`** | **FUSED at function granularity, 3.4x to 41.1x** |
| shared-slot layouts | **nine copies collapsed to two definitions** |
| `parse.kel` failure modes named | **THIRTEEN**; eleven counters guarded |
| **the type checker's INPUT** | **the DECLARED rows now come from the pipeline; the derived ones do not** |
| branch | `feat/typecheck-bindings-from-pipeline`, cut from `v0.2.3` at `fe2af14f` |

## THREE RULED REFUSALS IMPLEMENTED, AND ONE OF THEM WAS WORSE THAN A MISSING NUMBER

Batched on your approval. The trade is recorded rather than assumed: one gate cycle instead of three,
against a bisect that now lands on all three at once and a revert that takes all three.

### The nesting cap was not the finding. The silent drop was.

`verify_depth.kel`'s `push_frame` read `if df.sp > 127 { df.sp = df.sp; }` — a no-op branch,
documented as a deliberate drop. **In a verifier that is not defensible.** A dropped push means the
nested region is never walked, the parent folds in whatever `child_*` the PREVIOUS delivery left
behind, and `deliver` later decrements `sp` for a frame that was never pushed. The pass then
publishes a verdict over a program it did not traverse, and that verdict can be wrong in **either**
direction — it can miss a real underflow and it can invent one.

**It is not a hole in anything shipped, and I checked before saying anything.** The stage is reached
only through `depth_reject_chunk_via_kel` and its composition; it is not wired into
`self_hosted_compile`, and the shipping verifier is still the Rust `src/verify.rs`. This is a latent
defect in a stage being validated toward Order 2.

**128 was never a declared cap.** It was an array size with a silent-drop guard, which is what the
`v0.3.0` line warned against. Your 32 replaces a silent wrong answer with default-deny.

**Frames are nesting plus one**, because `run` pushes a root frame before any nested construct. The
arrays are sized 33 and the guard admits exactly 32 levels. Pinned from both sides and
mutation-verified — lowering the cap to 31 fails the accepting half by name.

**The verdict alone would have repeated the shared-message defect**, so `dv` gains `out_cause`
(appended) and the driver gains `DepthVerdict` with `Accept`, `Underflow` and `OverCap`. Only the
cause says whether raising the cap would change the answer.

### `-255` is ambiguous, and the test is sound by the case rather than by the code

It means two things in one call path. `mi_join_header` calls `mi_join`, which returns `-255` from a
pool overflow, and then returns `-255` itself for a missing header region. The test reaches the second
because the first cannot fire for its input, and a control proves the identical input joins cleanly
with the region restored. The test says all of this in its own doc comment.

**Its neighbours use `-233` and `-234` for exactly this reason**, and the comment above
`emit_name_records_from_nout` states the principle outright. The header check is the odd one out.
**Splitting it is one line and I held it for you**, because an error code is an observable.

### The reservations are free, and the collision that nearly made them look done is not

`CRYPTO_SIGNATURES`, `PROVENANCE` and `AUTH_TIER` at `0x0024..0x0026`, checked against every live kind
and against the parity-plane convention, pinned as unemitted with a vacuity guard. `AUTH_TIER` is a
region rather than a header field deliberately: a new header field changes every artifact's bytes and
a region changes nothing until emitted.

### Risks, stated because you asked for them in the pull request

- **Batching.** A bisect lands on all three; a revert takes all three.
- **The cap narrows the pass from 128 to 32.** That is the intent, not a side effect. The corpus is
  unaffected, and a chunk nesting 33 to 128 that the pass previously walked is now refused.
- **The frame arrays shrank from 128 to 33**, which changes `verify_depth.kel`'s private data size.
- **`-255` remains ambiguous.** The test is sound; the code is not yet.

### Two probe errors of my own, both caught by the compiler rather than by care

I reached for `Op::PushBool` and `Op::PushInt`, which do not exist — the encoding is `PushImmediate`
with a documented operand table. And my reserved-kind test parsed a framed module as a wire container
and got `BadMagic`; the fix was `parse_wire_sections`, the public accessor, rather than rebuilding a
`WireAuxBody` in the test, which would have been a second encoding free to drift from the one under
test.

## THE LIVE DECISION LIST IS EMPTY, AND I ASKED TWO OF THE QUESTIONS WRONG

Thirteen rulings recorded. The three standing forks are answered and so are ten further items; the
full record is in `HANDOFF.md` under "Open, held by the operator". This increment changes four
documents and nothing executable.

### Two of your rulings were taken against stale information I gave you

**The ECC plane is already exercised end to end.** You ruled that it seemed easy to add a test. It
exists. `SchemaBuilder::with_ecc` is at `src/wire_schema.rs:875`, `finish` calls `protect_all`, and
eight tests drive it on real compiler output across `tests/secded_end_to_end.rs` and
`tests/ecc_signature_ordering.rs`. Every corruption case is paired with the same corruption on an
unprotected artifact, asserted undetected, so a caught flip cannot be credited to the CRC. I read the
decision document's status field instead of the tree, and the document was stale. It is corrected in
place.

**Your token-array instinct was right, and better than my framing.** You said that ideally the tokens
stream so no large buffer is needed. They already do. Every `parse.kel` cursor move is plus or minus
one, `base` and `at` exist so a host slides the window with no protocol, and the fused driver slides
it at `FUSED_WINDOW = 8` where three would suffice. **What is left is the declaration, not the feed**
— `packed: [Word; 40960]` reserves the slots regardless. So shrinking the array is the lever, and it
REMOVES the input bound rather than widening it. Your ruling to leave the number alone stands and is
unaffected. Filed as its own increment.

**The common cause is this line's recurring defect in its sharpest form.** I derived a status from a
document rather than from the system. Previous instances cost a measurement. This one cost two of
your rulings, which is the scarcer resource.

### What was done here

- `V0_5_0_KELEUSMA_HOST.md` line 16, the probe-controller example, scrubbed. It was the only
  occurrence in any tracked document.
- `CHANGELOG.md` push order corrected. **The handoff cited line 340; it is at 571.** Verified against
  `src/vm.rs:6442`, which pushes low then high then flag — not against `GRAMMAR.md`, because
  correcting published text on a second document's authority is how the wrong one wins.
- `PIPELINE_THEN_MONOLITH.md` records the file-operand ruling and marks the sidecar fingerprint
  mandatory rather than conditional.
- `WIRE_FORMAT_V2_WORD_ORIENTED.md` item 5 corrected, with the staleness recorded rather than erased.

### What is authorised and NOT yet implemented

The file operand, a declared verifier nesting cap of **32**, the signature and provenance region
reservations with `AUTH_TIER`, and the `-255` negative test. Each is a separate increment.

**One trap recorded next to the reservation work**: `kind::SIGNATURES` at `0x0016` is per-chunk TYPE
descriptors, not cryptography, and the cryptographic signature lives in the framing header. A reader
checking whether a signature region is reserved will find that constant and wrongly stop.

## WHAT THE PIPELINE-BINDINGS INCREMENT DID

**Order 1 item 3, first slice.** The type checker's DECLARED binding rows -- a function's declared
return type and each parameter's declared type -- now come from `parse_functions`, the self-hosted
`lexer` into `parse` pipeline, through `binding_rows_from_pipeline`. They are compared against the
reference-AST extraction by NAME STRING rather than by id, because the two live in different
identifier spaces and comparing ids would compare the numbering rather than the content.

**Nothing was encoded to make this work.** The parameter's name was ALREADY in the record stream --
the header emits `4 + name * 64` and the driver discarded the payload because a count was all any
consumer needed -- and the `let` name is the record added in the previous increment under your
ruling.

### THE COMPARISON FOUND A DEFECT, AND IT WAS IN MY REFERENCE-SIDE EXTRACTION

**`Bool` does not parse as a `Prim`.** The reference parser yields `Named("Bool")`, so the harness's
`TypeExpr::Prim` match dropped every `Bool` annotation. `fn f(b: Bool) -> Word { 1 + b }` was
REJECTED by the reference compiler and ACCEPTED by the stage, because `b` had no binding row at all.
The pipeline extraction keys on the type NAME and reached a binding the AST walk did not. A second
extraction found a hole in the first, which is an argument for differential INPUTS and not only
differential outputs.

### What this slice does NOT establish

**Only the declared bindings.** A `let` bound to a literal or a call still produces no pipeline row:
the initialiser's shape is in the body record stream, so reading it means walking the forest rather
than the header. `the_pipeline_rows_are_the_declared_subset` pins that boundary from both sides --
it asserts the reference DOES produce the row, so it is non-vacuous -- and tells the next increment
to fold the case in rather than delete the pin.

### Recovered work, re-measured rather than trusted

These edits predate a laptop crash and were never committed. Verified on the recovered tree: 15/15
`selfhost_typecheck`, `fmt --check` 0, `clippy -D warnings` 0, and the four-entry feature-matrix
`cargo check --tests` sweep 0 on each. **The first verification attempt reported `FMT_EXIT=0` from a
`head` rather than from `cargo fmt`** -- the seventh constructed status this line has recorded.

## WHAT THE PREVIOUS INCREMENT DID (session 48)

`parse.kel` reported its capacity limits as raw virtual-machine traps. Measured by feeding the
stage malformed and oversized sources, not by reading it:

| input | reported | now |
|---|---|---|
| 65 local bindings | `IndexOutOfBounds(64, 64)` | names locals, the count, and the cap |
| 65 nested parentheses | `IndexOutOfBounds(64, 64)` | names expression nesting |
| 257 statements in a body | `IndexOutOfBounds(256, 256)` | names the statement table |
| an unmatched `]` | `IndexOutOfBounds(-1, 64)` | names the bracket and its token |
| an unterminated block | "did not reach DONE within its iteration budget" | names the likely cause |

**The first two are the finding.** `opstack` and `let_names` are both 64 entries, so two unrelated
limits produced a BYTE-IDENTICAL message. `the_two_sixty_four_caps_no_longer_give_the_same_message`
encodes that defect so it cannot return.

**The guard is on the pointer and each guarded array carries one spare slot.** The write precedes
the increment, so a guard on the increment alone fires one write too late; clamping at the last
usable slot would have REFUSED the exactly-full program that parses today, which is a unilateral
narrowing. Every boundary is pinned from both sides — 64 parses, 65 does not.

## WHAT I GOT WRONG, RECORDED AS CORRECTIONS

- **I widened two arrays of eight and the trap did not move.** Six more are written at the same
  local-binding counter. The test now DERIVES the array set by reading the stage, and is verified by
  mutation: reverting `let_enum` to 64 fails it by name. A hand-written list would have encoded the
  mistake I had just made.
- **A sixth constructed status, and it nearly landed.** The full suite reported `exited with code 0`
  with forty green lines. That was `grep`'s exit; `cargo test` had aborted at a failing binary and
  eighteen never ran. **The tell was the SHAPE, not the code** — `selfhost_parse` takes ninety-eight
  seconds and nothing in the list took that long. Now run with `--no-fail-fast` and the exit code
  captured outside the pipe.

## What this green suite does NOT establish

**Roughly a hundred and thirty fixed arrays remain in `parse.kel` and four causes are named.** The
rest still trap raw: the nesting stacks at 8 entries, the 32s, the struct-definition tables at 64,
and the remaining 256s and 512s. **None has been probed**, so none is known reachable or
unreachable. The chunk-table work is direct evidence that this matters: three of its walls were
unprobed arrays, and each reported a size rather than a cause.

**Separately, the probe found malformed inputs SILENTLY ACCEPTED**: a stray `)`, an unclosed `(`, a
binary operator with no right operand, and an empty index `a[]`. That is acceptance laxity rather
than a diagnostic defect, mitigated but not closed by the cross-check against the reference compiler.

**A question for you rather than a decision I took**: these refusals PANIC, matching the existing
failure mode of `parse_functions` and of the chunk-table guard. Turning them into a `Result` is
defensible and changes a signature many tests and both compile paths depend on. I did not widen the
scope to do it.

## Held for you, with rulings

- **`Op::cost()`**: 50 of 66 opcodes unmeasured. *Ruled: after Order 1.*
- **Derived operands in type rejection**: *Ruled: before publishing V0.3.0.*
- **Publication**: *held.*
- **The Japanese FAQ entry** renders as English. *Ruled: correct eventually.*
- **The input-re-readability fork** in `../decisions/PIPELINE_THEN_MONOLITH.md`: still open. It
  decides whether the monolith is one command or two.

## THE LAST CAP IS GONE, AND IT WAS NEVER ONE NUMBER

`wire.kel` parses at 486 functions. Raising `toks.chunks` from 256 to 1024 was three edits and the
first two did not work: the wall moved to `LoopLimitExceeded` (two `limit 256` loops over the chunk
count) and then to `IndexOutOfBounds(388, 256)` (the six chunk-indexed `chunkret.ret_*` arrays).

**A cap is a FAMILY, and that is the second family in two increments.** The eight local-binding
arrays were the first. Both times I widened what I could find by name and the trap did not move.

**THEN SIXTY-EIGHT TESTS FAILED AND NOT ONE NAMED A SLOT.** The shared layout was restated in FOUR
places — the driver and three harnesses — so moving the block left them seeding the type ids at the
old slots, and `parse.kel` sized every field as one byte. **My derivation test proved the DRIVER
agreed with the stage and said nothing about harnesses that never consult the driver.** Now: public
chained constants, harnesses aliased, and a guard that WALKS the tree rather than checking a list.

**Two vacuity guards fired in one run** — the family test found zero arrays (a bug in my own walk),
and the no-copies guard flagged itself. Both now verified by mutation.

## `parse` INTO `reconstruct` IS FUSED, AND THE PREDICTED COST DID NOT EXIST

Cut at FUNCTION granularity. `self_host_compile_fused` holds one GROUP -- consecutive same-named
heads, which are one chunk -- where `self_host_compile` holds every function's records for the whole
program. Byte-identical modules, mutation-verified: flushing per function instead of per group fails
the equivalence test by naming the multihead chunk.

**Measured 3.4x to 41.1x**, against a recorded estimate of 3x to 13x. `wire` is the 41x case, so the
largest stage benefits most.

**THE FOURTH SIDECAR FACT DID NOT MATERIALISE.** A group ends when the next function's NAME differs,
so a completed function waits for the following HEADER -- a bounded one-function lookahead, not a
whole-input dependency. The name table is available before the drive. That predicted cost was the
reason this increment ranked below the diagnostics work; it was not real.

## I SHIPPED A DEFECT MY OWN GUARD WAS WRITTEN TO CATCH

Raising the chunk table moved the parser's shared block, and a FIFTH copy of the layout in
`compiler/src/main.rs` actively seeds the parser. That binary was reading the keyword and type ids
from inside the chunk array. Nothing caught it: `run_parse_pipeline` is reachable only from `main`,
so its constants are compiled by continuous integration and never executed.

**The guard I wrote to prevent this walked `src/` and `tests/`.** A guard with a scope narrower than
the class it guards is the same defect it was written to prevent. It now walks the repository and
asserts that `compiler/` was actually reached.

**The lexer's block was restated in four places too** and had failed nothing, because it has not
moved -- exactly the state the parser's five copies were in the day before. Both layouts are now
published and chained, all nine copies alias them, and both derivation tests are mutation-verified.

**Two corrections on my own reporting**: I said `compiler/` has zero tests; it has 86, and my check
was scoped to `compiler/src/`. And root `cargo fmt --all` does not reach `compiler/`, which declares
its own workspace -- a local gate touching it needs a `cd compiler` pass.

## FIVE MORE CAPS, FOUND BY SWEEPING RATHER THAN BY TRIPPING OVER THEM

Parameters (32), `if` nesting (32), `for` nesting (8), array-literal nesting (8), and enum variants
(256, a WHOLE-PROGRAM total). **Two more pairs shared a message**, one array-size down from the pair
fixed the morning before -- fixing the instances I had measured left the class, and sweeping found
the rest.

**The enum bound's size does not say what it counts**: 128 enums of two variants refuse at the same
point as one enum of 257. No message naming an array size could convey that.

**The family lesson was applied rather than relearned.** `ps.pcount` alone indexes twelve arrays;
the widening derived thirty-one arrays across five counters from the stage. Fourth consecutive
increment where a hand-written list would have been wrong, and the first where I did not find out by
failing.

**Corrected from my own probe**: call arguments are NOT a separate cap. A call cannot exceed its
callee's arity, so the parameter cap fires first. A probe that varies two quantities measures neither.

**Naming a cause has a measured price**: 645 to 660 names, 34,148 to 34,785 blob bytes. The
diagnostics programme has spent 33 of the 1,024-name budget across two increments, leaving 64%
margin.

## THE LAST TWO UNNAMED FAILURE MODES ARE NAMED

**The token array had TWO failures**, and which one a caller got depended on how far over they were:
`IndexOutOfBounds(40960, 40960)` from the stage, or a shared-slot range error from the driver's own
seeding loop. One refusal now fires before any seeding. **This is the bound the corpus is closest
to** -- `parse.kel` is 32,907 tokens, 80% of it.

**Six bare `unwrap()`s became one diagnostic.** A top-level `struct` declaration was the measured
cause; `parse.kel` has no struct handling at all. **It does not decide whether `struct` should be
supported** -- that is yours, and the test says so.

**Both of my own mistakes here were the session's recurring one.** My test generated against the
REFERENCE tokenizer while the cap governs the STAGE's lexer -- measuring the wrong quantity, so
`lex_token_count` is now public and documented as the count the cap uses. And an insertion detached
`#[allow(clippy::type_complexity)]` from its function, because I anchored on the signature rather
than the item. I restored from `HEAD` and reapplied rather than stack a third correction.

## THE SWEEP CONVERGES, AND THE PROGRAMME NOW HAS A UNIT PRICE

Two more caps: call nesting (8) and data-block fields (512, a WHOLE-PROGRAM total like the enum
bound). **`IndexOutOfBounds(8, 8)` had THREE sharers**, not two -- call nesting sat behind a construct
I had not generated. All three are now held distinct by test.

**A distinction that is this session's trap in miniature**: array-literal ELEMENTS have no wall
through 1,025; array-literal NESTING caps at 8.

**The sweep is converging**: two caps this round against five last, and four constructs came back
clear (data blocks and `use` through 64, tuple elements through 32, array-literal elements through
1,025).

**The margin pin has moved SIX times and now yields a rate**: roughly three names per cause named --
an error code, a capacity, a guard. 39 of the 1,024-name budget spent, 65% margin left. It has not
once moved for a reason its author was thinking about, which is why it is pinned rather than computed.

## THE SWEEP IS DONE, AND IT CAUGHT A STALE DIAGNOSTIC OF MINE

A final round found **no new reachable caps**. It found something better: the chunk-table guard's
message and comment were **stale in four ways and I made them so** when I raised the cap -- it told a
caller with 1,025 functions about a *257th* entry, cited a 256-entry array that is now 1,024, and said
raising the array "is NOT done here" after it had been done. Both copies now derive from
`PARSE_CHUNK_CAP`.

**Five of my probes this session measured something other than what I intended.** The rule that came
out of it: when a generated program fails, confirm the REFERENCE accepts it before concluding anything
about the stage. It caught three of the five.

**`HANDOFF.md` is rewritten** against `3ffd5a4c` with every value re-measured, and its own check block
was run as a resuming session would.

## DERIVED OPERANDS IN TYPE REJECTION ARE CLOSED

Your ruling was "before publishing V0.3.0", so this needed no new decision. `let a = 1 + 2` left `a`
UNKNOWN and `a + b` was accepted; the stage now proves `a` from its operands and rejects.

**It needed a fixpoint, as recorded.** A binding may take form 2 -- "takes whatever node N yields" --
and the stage proves a tag only for an operator node whose operands agree. **The host supplies only
WHICH NODE**, which is verified by mutation: neutering the stage's join fails the test.

**The cap I almost documented was not the bound.** I nearly wrote "reaches a chain of four" from
`tyb_rounds() = 4`. Setting it to 1 rejects every depth through six: scoping forces `let` bindings
into dependency order, so one pass proves the chain. The cap is insurance for out-of-order rows.

**The new edge is pinned**: a `let` bound to a FIELD READ or an INDEX is still unreached, and the
test says so as a measurement rather than an aspiration.

## IDENTITY NOW TRAVELS WITH THE STRUCTURE (your fork, option 1)

Order 1 said the type checker's input should come from `parse.kel` plus `reconstruct.kel` because
"structure is available". **Measured, that was half true**: a `Local` record carries a SLOT and no
body record mentioned a name, while the type channel is keyed by interned NAMES. You ruled that a
`let` record should carry its name id.

Built. The statement table emits in the PACKED form (kinds capped at 63), so the name goes out on the
migrated path with tag 90 -- a full word, no packing, no radix. The driver pairs it with the
following `LetIn` and diverts it, leaving the node stream unchanged.

**I claimed the blast radius before measuring it and was wrong.** I said nothing else was touched,
having run one suite; eight tests then failed because a THIRD decoder -- the Rust reconstruction that
checks `reconstruct.kel` -- panicked on kind 90. Three decoders now consume the record stream, and
only the TAG is shared, which is correct: their skip sets legitimately differ.

**The margin pin moved a seventh time, and this is the first move predicted in advance**: 669 names,
35,154 blob bytes.

## Next intended increment

**Nothing is queued that does not need a decision from you**, and the sweep that needed no ruling is
now exhausted. The three live decisions are in the handoff's operator-held list and repeated below.



**`parse.kel` is 32,907 tokens against its own 40,960-token array, at 80%** -- newly measured,
unowned, and nothing reports it when it binds. I would NOT widen it unilaterally: raising a capacity
widens what is admitted, and the chunk-table raise was widened only because you had named it. A
NAMED REFUSAL costs nothing and widens nothing, which is what I would do absent direction.

Beyond that, the remaining structural work is the phase-selection architecture, which is blocked on
the input-re-readability fork below.
