# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-09-03 (session 62) — `Text<N>` has a flat layout, and my own handoff was red

## MY SESSION-61 HANDOFF WAS RED AND ITS BANNER SAID IT WAS GREEN

The refresh I wrote at the close of session 61 rewrote the validity block and, in doing so, deleted
one of two required occurrences of the construct-support triple. A test derives that triple by
calling the boundary table and demands at least two occurrences in `HANDOFF.md`, precisely so that a
branch adding a case turns the document red instead of leaving it quietly wrong. Deleting one left
`found 1 occurrence(s)`.

**Two properties made it invisible, and both generalise.** The pre-push hook's routine tier excludes
the self-host binaries, so pushing would never have caught it. And "markdown is not compiled" is true
of nearly every file under `docs/` and false of exactly the one I edited, which is read by
`include_str!`. A true general rule applied to the single population it does not cover, which is the
species this line spent session 61 cataloguing, committed while writing the catalogue.

Repaired at `1826f3a4`, with a must-fire control: the test fails on the unfixed commit and passes on
the fixed one. I enumerated the coupling class rather than stopping at the first finding. Six files
mention these documents and four mention them only in prose.

## `Text<N>` HAS A FLAT LAYOUT

Increment 2, on `feat/text-capacity-layout` at `9ff3f345`, pushed. A positive literal capacity now
yields a two-element tuple, a word-sized length followed by an array of exactly `N` single-byte
elements, sized exactly one word plus `N`. It reuses the existing tuple and array descriptors rather
than adding a variant, so sizing, field offsets and the typed verifier's shape reconstruction need no
change, and no opcode is spent.

Three guards, each carrying its reason: a symbolic capacity surviving monomorphization is refused
rather than guessed, `Text<0>` is not a type, and the capacity converts with a checked conversion
rather than a cast, because a cast truncates silently on a 32-bit host and a truncated capacity
under-sizes the buffer without failing.

**Checked by mutation rather than by reading.** Adding a terminator byte to the content array fails
two of three shape tests, so "no terminator" is a tested claim rather than a comment. The size
formula is pinned at four word widths, because it is a claim about tuple layout having no padding and
one width does not establish it.

Scope: the layout only. `Text<N>` still does not compile end to end and the refusals above the layout
remain, each to be removed by a later increment. Verified green at the routine tier, 2212 tests plus
doctests, with both exit statuses read from the log.

## THE REFUSAL WAS SHORT BY THREE POSITIONS, AND IT HAD ALREADY BEEN WIDENED ONCE

Enumerating type positions BY CLASS rather than by example found three that compiled an unbuilt
type: a capacity nested inside an array in a `let` annotation, one nested inside a tuple, and a
trait method signature. All three now refuse, on `feat/text-capacity-layout` at `17aa1b22`.

**Two distinct mechanisms, and neither is a typo.**

The `let` cases failed because the body walk pattern-matched the annotation against the capacity
type instead of calling the recursive walk the signature positions already used, so it saw a
capacity only when it was outermost. The positions that REUSED the check never had the hole; the
one that reimplemented it did.

The trait case failed because the walk iterated functions and impl blocks, which is every
declaration that has a BODY. A trait signature has none, so it fell outside a loop whose shape had
been fixed by bodies rather than by the class being checked, which is types.

**The point is not that three more positions needed checking.** This guard had ALREADY been widened
once, after increment 1 shipped a refusal that four of five positions walked past. A guard widened
after a miss has a new reach, and its correctness on the cases that prompted the widening is no
evidence at all about that new reach. The reach question has to be asked again after every widening.

The test refuses to count a lex or parse failure as a pass, and asserts that every fixture actually
reaches the compiler. That caught two of my own fourteen fixtures being malformed -- an inherent
impl and a bare `None`, neither of which this parser accepts -- which would otherwise have sat there
as spurious refusals, testing nothing while reporting success.

## TWO DESIGN RECORDS CONTRADICTED THE RULINGS THAT SUPERSEDED THEM

`TEXT_CAPACITY_TYPE.md` still listed the overflow rule as an open question belonging to you, after
you had ruled on it. `TEXT_N_IMPLEMENTATION_BRIEF.md` still said the format fingerprint is derived
from the scalar size table, after your redirect made it a per-release random constant.

**The second is worse than stale, because it inverts a diagnostic.** It tells a session performing
the `ScalarKind::Text` collapse to expect the fingerprint to move and to treat a non-moving
fingerprint as a broken detector. The value derives from nothing, so it moves exactly never, and that
session would have chased a healthy component. Both corrected.

Both were a record that was correct when written, left standing beside the ruling that superseded it,
with nothing marking which was newer. **A superseded document does not announce itself.** Both were
found only by reading the source they described rather than the description.

## WHAT I GOT WRONG TODAY

- **I read a verdict off a truncated summary.** One of four new tests appeared absent from a run and
  I opened an investigation; the test had passed, and my own `tail -20` had hidden it.
- **I read clippy's status off a pipeline again.** A composite ending in `tail` reported success while
  clippy had failed with five errors. This file's own handoff warns about that in three variants, and
  I produced a fourth while working from it.
- **My contention restraint was inconsistent, and this one is for the other line to know.** I held two
  docs commits overnight rather than disturb their twelve-hour mutation sweep, then ran eight cargo
  invocations beside it today.

  **CORRECTED, and my correction was worse than the thing it warned about.** I first wrote here that
  the sweep was "contaminated by me" and that "today's figure is suspect". **There is no figure.**
  The sweep was killed at 12h51m on the fifth of twenty-five mutations of round one. Calling a
  result contaminated asserts that a result exists and was merely degraded, which is a stronger
  claim than the one I was cautioning against, made in a durable artifact, about another line's
  work.

  The `v0.3.0` line's own statement is sharper and survives the sweep dying, which mine does not:
  round one alone projects to roughly **sixty hours**, so the census is not merely stale but
  **expensive to un-stale**, and a claim that cannot be cheaply refreshed should not be leaned on as
  current.

  **And my either-or was wrong too.** Their framing is right: contention matters ASYMMETRICALLY, by
  check, not by machine. Load beside a build-dominated sweep is nearly free; load beside a
  wall-clock-scored performance canary corrupts it. The rule is not "keep the machine clear" but
  "know which run is scored on time, and yield to that one."

## THE OPEN DECISION IS STILL YOURS

Whose release gate is canonical at the back-merge. Unchanged from session 61 and still unanswered. I
ruled union with conditions so work could proceed; say so before the back-merge if you disagree.

## THE QUEUE, AS IT ACTUALLY STANDS AT SESSION END

**BOTH SUBSTANTIAL ITEMS ARE OPERATOR-BLOCKED, and both blocks were found by TRYING rather than by
planning.** That is the honest state: the productive ceiling without your input is close, and a
resuming session should not mistake either item for available work.

1. **`Text<N>` EMISSION -- blocked on a decision that is yours.** A spike removed both refusals and
   asked the compiler what happens. It said `let binding declared as Text<8> but value has type
   Text`, which is the distinct-nominal-type increment working AS DESIGNED: a literal is static
   text, `Text<8>` is dynamic text, and they deliberately do not unify. **Nothing can enter a
   `Text<N>` until there is a way to put it there**, and the silent path is closed by a language
   rule rather than by taste -- `GRAMMAR.md` states that no implicit type coercion exists.

   The surface form is already open question 2 in `../decisions/TEXT_CAPACITY_TYPE.md`. **Do not
   pick it unilaterally**: it appears in every program written with the type and is far more
   expensive to change than the layout beneath it.

   The encouraging half: exactly two match arms had to change to admit the type, and no other pass
   objected. The machinery below the surface is in place; only the way in is missing.

2. **The `ScalarKind::Text` collapse -- blocked BEHIND emission, not independently available.** That
   kind is two words precisely because it must still hold the dynamic case; its own comment says the
   one-address form becomes correct only once `Text<N>` removes that case. It is a wire change, so
   it is free while nothing has shipped at `BYTECODE_VERSION` 2 and costs a version afterwards.

3. **The width bundle -- blocked because it is a SEMVER BREAK, not a cleanup.** 33 signatures across
   5 files take `addr_bytes`, and **14 are public**, so it cannot land without breaking every
   embedder of a crate published at 0.2.2. An API decision, not a refactor to take on initiative.

4. Genuinely available, and small: the discard-arm reachability census, and `DATA_INIT` for the one
   stage that does not elide.

## WHAT `Text<N>` IS, SO NOBODY RE-PLANS IT

Four increments merged: the type surface refused everywhere below; the flat layout, a length word
plus exactly `N` content bytes, reusing existing descriptors with no new variant and no opcode; a
distinct nominal type in the checker; and a zero value cross-checked against the layout. Three
refusals remain and **all three are correct** -- nothing generates code for it yet.

## THE MEASUREMENT LESSON THIS SESSION KEPT RE-LEARNING

Three figures in this file were wrong in the same way, and each was corrected only by deriving it
from the thing itself:

- Sizing the type-system increment: "113 sites across nine modules" counted MENTIONS. Two static
  scans then disagreed at twenty-two and five. **The compiler answered in thirty seconds: five, all
  in one file.**
- The width bundle: recorded at 43 across seven modules, a quick grep said 11 because `addr_bytes`
  sits on continuation lines, and walking each signature gave **33, of which 14 public**.
- The refusal census: a fixed-window scan reported two refusing opcode arms; brace-matching the arms
  gave **four**, and the under-report was in the reassuring direction.

**A count of lines mentioning a thing is never a count of the thing.** When the data is reachable by
construction, arguing between two regexes over its source text is a choice to keep an instrument
that can be wrong.

---

## Session 61, retained because the release blocker is still live in fact

## A RELEASE BLOCKER, FOUND BY CENSUS, AND IT IS YOURS TO KNOW ABOUT

**`RELEASE_PROCESS.md` said five crates publish to crates.io. There are seven.**
`keleusma-wire` and `keleusma-wire-derive` appear nowhere in it -- not in the crate list, not in
the dry-run sequence, not in the publish sequence, not in the release-record template.

**Following the document as written loses money.** It publishes `keleusma-macros` and
`keleusma-arena`, both irreversible, and then FAILS on `keleusma`, because the registry has no
`keleusma-wire` to resolve. The failure lands after the point where the abort criteria still help.

Step 3 exists to catch exactly this. Its own text warns that the local gate and the audit gate both
pass while `cargo publish` fails at the registry-resolved verify build, and its dry-run list omitted
the only two crates that would have triggered it.

**Nothing was inconsistent; something was absent.** Both crates are marked publishable, carry a
description, licence and repository, have their own continuous-integration job, and are covered by
the release gate. Every artifact the tooling can inspect said they were ready. The one document the
tooling cannot inspect had never heard of them. **A missing entry has no line number.**

Corrected at all four enumeration sites. The census also came back clean on docs.rs configuration,
tarball excludes, publish metadata, and CI coverage, so this is one blocker rather than the first of
several -- and I checked those four classes rather than stopping at the first finding.

## NOTHING IS RED AND NOTHING OF MINE IS UNMERGED

`origin/v0.2.3` is at `27fcbd11`. Every branch this line created is merged. Worktrees are clean.
That is the first time today it has been true, and it is deliberate: the previous session stranded a
branch twice and I would rather hand you an empty queue than a short one.

The full release gate is green at 13 steps. Per-step, because a total across steps double-counts:
default 2739, no-default 297, signatures 2347, signatures+shell 2364, self-host 2518, wire 57 and 20,
detached compiler 86.

## WHAT LANDED

**`Text<N>` is STARTED.** The type parses, is distinct from bare `Text`, and carries its capacity
through monomorphization. **Nothing below the type surface is built** and every stage that cannot
handle it refuses with a named error, so each later increment removes one refusal rather than adding
a feature. Increment 2 is planned in full: the flat layout is `Tuple([Int, [Byte; N]])`, needing no
new descriptor variant, sized exactly `word_bytes + N` with no padding, and `Multiword<N, F>` is the
exact precedent for a nominally-distinct structurally-composite type.

**Two increments found defects in their own first drafts.** The `Text<N>` refusal did not work: the
infallible type conversion resolved it to static text, so four of five declaration positions
compiled. The float-width predicate was a DENYLIST in a default-deny codebase and claimed two-bit
and four-bit floats were implemented.



**The format fingerprint**, per your redirect. Random per release, in a constant beside
`BYTECODE_VERSION`, currently `0x4327_63E1`. `scripts/fingerprint.sh` reads this tree's value, reads
any commit's or tag's, and rolls a new one. **Release step 1b does the rolling**, with the reason
attached: skipping it produces no warning and no test failure, only two releases silently accepting
each other's bytecode.

**Your redirect was right and it answered the objection my version was built on.** I argued a
hand-written constant fails by being forgotten. True, but a derived value covers only what it hashes,
so a release changing an opcode's meaning would leave it unmoved while genuinely differing.

**Float arithmetic honours the declared width** at ten sites. This was a defect in every build, not
just under `narrow-float-32`, because the runtime admits narrower-than-declared bytecode on purpose.
The `v0.3.0` line's `f32` rung went green on absorption, which is independent confirmation from a
separate implementation — mine says the construction is self-consistent, theirs says it is right.

**A target may not claim floats at a width that is not a format.** `has_floats` with a zero width
compiled, loaded and returned 3.75, computing in `f64` while declaring zero bits.

**`Opaque` is sized by the address width**, finally. That branch had been red since yesterday.

## THE OPEN DECISION IS STILL YOURS, AND IT IS NOW ON THE CRITICAL PATH

**Whose release gate is canonical at the back-merge.** The `v0.3.0` line's `scripts/release-gate.sh`
differs from mine by 29 lines — a `native_codegen` step conditional on an LLVM install.

Everything either line has said today about "the gate is green" was said about a **different
instrument**. At the back-merge one definition wins and the losing line's recorded greens were
produced by a tool that no longer exists.

I ruled union rather than choice, since their step covers a package my gate is structurally blind to,
and conditioned it: the skip must be loud, `gate-summary.sh` must show it, and a skipped native step
is NO-GO for a publication shipping the native backend. That last is now in `RELEASE_PROCESS.md`. **If
you disagree with union, say so before the back-merge rather than during one.**

## WHAT I GOT WRONG, BECAUSE THE CORRECTIONS ARE THE USEFUL PART

- **A gate wait that measured its own existence.** `pgrep -f "release-gate.sh"` matches the shell
  running it. I reported a gate as running for nearly two hours after it went green.
- **A denylist for a safety predicate**, in a default-deny codebase, one commit after writing a
  release rule about instruments asserting more than they measured.
- **Two skips in an enumeration test that incremented nothing.** One dropped the exact widths the
  denylist had wrongly admitted.
- **A test named for a subject its body never reached**, which is worse than no test: it consumes the
  attention that would have written a real one.
- **Eleven passing tests covering four of ten sites**, found by mutation rather than by reading.

## THE QUEUE

1. **`Text<N>`.** Designed, authorized, unstarted, and the largest thing left. Its brief and
   completion condition are drafted. The `ScalarKind::Text` collapse to one address must land WITH it
   — that is a wire change, free before publication and unavailable after.
2. **The width bundle**, recorded as debt rather than paid. `addr_bytes` appears at 43 signatures
   across seven modules, several public. Cheaper before a publication than after one.
3. The earlier queue is unchanged: the discard-arm reachability census, and `DATA_INIT` for the one
   stage that does not elide.

## FOR YOU

The exchange with the `v0.3.0` line produced more than either line's code did. Six instances of one
error class in a day, catalogued jointly, each found by the other line rather than by the line that
made it. The general form: **a true measurement quoted as though it ranged wider than it did**, where
the scoped and unscoped statements are the same sentence minus a clause, so nothing looks missing.

Worth knowing when you weigh what either line reports: neither of us caught our own instances.

