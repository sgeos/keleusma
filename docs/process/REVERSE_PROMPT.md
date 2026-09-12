# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

# CURRENT STATE — READ THIS BLOCK, THEN STOP

**2026-09-12, session 65, through the eighty-third increment.**

**Where the work is.** The type-rejection input path now carries **ten of twelve** real `.kel` stage
sources, up from two, at **1.6x** the shared data it uses today rather than 7.3x — a growth of
**+33 KiB** against a 16 MB ceiling, across six verdict-preserving reductions.

**What is next, and it is YOURS.** Closing the last two sources is a **capacity decision**, not
another reduction. `parse` needs about 192 where the caps are 128; `wire` needs roughly four times,
dominated by declaration-indexed tables no reduction reaches. The four standing operator decisions
are unchanged and still block the language-surface work they name.

**What is next that is NOT yours.** **Four** self-hosted-parser gaps remain, all traced and all
feature work: the variable and struct match patterns need new arm semantics; `assert` needs a token
code, a lexer arm, statement parsing and emission through two more stages; a qualified call
expression needs the same kind of work. A fifth — the bare enum unit-variant pattern the grammar
documents — was a missing branch and **is fixed**.

**None of the four blocks self-hosting**, and the reason is structural: all twelve stages compile
through the pipeline byte-identically, so no stage can contain a construct the pipeline cannot
parse. Each gap blocks a USER program, not the stages.

**All four used to fail by producing a malformed record stream rather than a refusal naming the
construct. THAT IS FIXED.** Measured first: every one reported only a `reconstruct.kel` work-stack
underflow or an unreduced record range, with a note about `reconstruct_range` reading slot zero —
text addressed to a stage author, never mentioning the `v =>`, the `P { x }`, the `assert` or the
`audio::` the user wrote. The driver now names the construct and the line, and keeps the stage's own
report after it, since the two halves serve different readers.

**The naming runs only on the failure path, so it cannot cause a false rejection** — it changes what
a refusal says and never whether one happens. It is Rust-side, so it is CAPACITY-NEUTRAL: no stage
source was touched and the pinned worst-case blob does not move. A guard asserts the scan names
nothing in any of the eleven driver-read stage sources, which all compile through the subset; it is
mutation-tested, and asserts each source parses first, since an unparseable source would make the
scan return nothing for an unrelated reason.

**THE FLOAT BOUNDARY IS THE LITERAL, NOT THE TYPE**, and measuring it stopped a wrong entry going
into that list. `fn f(a: Float) -> Float { a }` compiles and matches the reference byte for byte; a
float literal is refused. Both halves are pinned. **The stage-source guard would NOT have caught the
mistake**, because no stage source uses a float type — it covers constructs the stages happen to
use and is not a general check that the list is right.

**The divergence refusal was measured and found ADEQUATE**: it already names the offending chunk,
and the op-level message names the chunk, the op index and both ops. No code changed; it is pinned,
including that it assigns no fault, since the reference has been the divergent side before.

**THE ERROR TYPE'S THREE VARIANTS ARE NOW ALL MEASURED, AND THIS VEIN IS EXHAUSTED.** A non-host
target says it supports only the host width and keeps the retry hint; a reference-rejected program
surfaces the reference's own error and SUPPRESSES the hint, correctly, since the reference would
fail identically — and that suppression was already tested in `tests/self_hosted_backend.rs`. Only
`Unsupported` needed work, and it got it. **Nothing further here is available without either
touching a stage source, which prejudges the capacity decision, or an operator ruling.**

**MEASURED SINCE, ON THE PATH THAT MATTERS: all four are refused with an `Err` by
`self_hosted_compile`, the entry point behind `--compiler self-hosted`, and an ordinary program
still compiles.** So the subset is SAFE — nothing on the gap list mis-compiles — and the remaining
obligation is message quality, not soundness.

Two claims were made and corrected getting here, both worth the warning. First, that the obligation
was "considerably smaller than implementing": unverified, and `parse.kel` has no refusal channel at
all — 54 node kinds plus `DONE`, not one an error — so a refusal record kind would touch the
parser, the driver and every consumer. Second, the censuses drive a TEST HARNESS that unwraps, so
gaps surface there as panics; **that is not what a user meets**, and reasoning about refusal quality
from the harness measured the wrong thing.

**The pipeline also diverges from the reference on three binder forms** — a `for` variable, a match
payload binding, a const parameter used as a value — where it reports no occurrence at all. Pinned,
and bounded to that one channel.

**A risk stated here about that divergence has been checked and DISMISSED.** It claimed the pipeline
is safe only by omission, and would reproduce the reference side's false rejections if it began
reporting those binders. It has no separate locals set: a local read emits `local = 1`
unconditionally and only when the slot has a name, so an occurrence-present-but-not-local condition
is not expressible. Reporting and collecting are one lookup, not two walks that can disagree.

**A second hedge was replaced by a stronger fact.** The comment explaining why the branch-pair row
is withheld said a tag-based heuristic "could not be shown safe". It cannot work at all: a written
`else { }` and an implicit arm produce the SAME parse record stream, while the reference separates
them (one pair row against zero, measured). The distinguishing information never reaches this side,
so the question of which tag a case yields is moot. Witnessed in `tests/selfhost_parse.rs`.

**A census of the reference syntax tree's eighteen optional fields found no second instance of that
class in an active channel.** The node rows are extracted from the reference tree, which reads the
optionality directly; only pipeline-derived channels are exposed to it. That is a negative result,
recorded rather than tested.

**One guard the census did find worth pinning is now pinned and mutation-tested**: a body with no
tail expression must contribute no declared-versus-actual row, since manufacturing one is a
comparison the source never wrote and therefore a false rejection. Unguarding the extraction fails
the pin; reverting is green.

**Before trusting any green run**, read *"How a green local run has actually lied"* in
[`CLAUDE.md`](../../CLAUDE.md): six observed ways a verification run reported success while covering
less than assumed. Two of the six were paid for in this session's last two increments.

**Detail per increment** is in [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md), newest first. It is the
source of truth for reasoning; this block is the source of truth for state.

---

## EVERYTHING BELOW THIS LINE IS SUPERSEDED HISTORY

**It is retained for provenance and is NOT current.** Sections below describe the state as it was
when each was written, and several have been corrected since — by name, in the journal.

**This file is specified as the BOUNDED latest-state channel and has re-accreted to nearly 1,800
lines.** It was split once before, when it reached about 362 KB, for the same reason: each session
prepends a section and keeps the rest, so "bounded" becomes nominal while the file still claims it.
The block above exists so a resuming reader can stop at the line rather than reconstruct currency
from a stack of dated sections. **Nothing below was deleted**, because it is other sessions' record
and trimming it is not this session's call.

---

## Last Updated

**Date**: 2026-09-12 (session 65, through the sixty-fourth increment) — the type-rejection input path went from carrying two of twelve real stage sources to TEN, and from 7.3x the shared data to 1.6x, across six verdict-preserving reductions; what is left is a CAPACITY DECISION rather than another reduction

## THE FOUR DECISIONS ARE STILL YOURS AND NONE HAS MOVED

They are the reason the large work is blocked, and nothing below decides any of them.

1. **How does a value ENTER a `Text<N>`?** It appears in every program anyone writes with the
   type. Open question 2 in [`TEXT_CAPACITY_TYPE.md`](../decisions/TEXT_CAPACITY_TYPE.md).
2. **Is the width bundle worth a breaking change?** 33 signatures, 14 public, published crate.
3. **Should `verify()` refuse float opcodes when the `floats` feature is absent?** **Evidence
   COMPLETE**: ten lines, prototyped, **zero new failures**, and the semantic worry is moot because
   the lexer refuses float literals in that build. Unlanded only because I said it was your call in
   a merged document, and a deferral is worth something only if honoured. **This is the cheap one.**
4. **Does any build configuration earn a continuous-integration job?** Cheaper than it looked on
   the WIDTH axis, unchanged on the FEATURE axis.

## SINCE THE LAST REFRESH (INCREMENTS 55 TO 64), IN ONE BLOCK

**This channel is BOUNDED and had drifted ten increments.** The per-increment reasoning is in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md); this is the latest state and the next step.

**A process note on the drift itself**: the reverse prompt stopped being updated exactly when the
increments got dense, which is when it carries the most. The append-only journal kept pace because
appending is cheap; the bounded channel did not, because it requires deciding what the current state
IS.

### THE HEADLINE: THE INPUT PATH IS NO LONGER THE OBSTACLE

The type-rejection stage is fed by tables with fixed capacities. Measured against the twelve real
`.kel` stage sources — the strongest available evidence, and nothing had ever run the stage on them:

| | first measured | now |
|---|---|---|
| real sources whose tables fit | 2 of 12 | **10 of 12** |
| shared data to carry the whole corpus | 54,301 words, 7.3x | 11,690 words, **1.6x** |
| growth over what it uses today | +366 KiB | **+33 KiB** |

Six reductions, each verdict-preserving under a differential that runs it against its own absence:
inert-row elision, then distinct-facts deduplication of the occurrence, operand-pair, call-site,
expression and binding channels.

**I predicted ZERO sources would fit.** Every step was larger than expected, which is the argument
for measuring rather than reasoning, made six times.

### THE NEXT STEP IS YOURS, AND IT IS A FIFTH DECISION

**Closing the last two is a CAPACITY DECISION, not another reduction.**

- `parse` is over by a little on four channels, the worst 162 against 128. **A cap of 192 admits
  it.**
- `wire` is over about fourfold on six, dominated by DECLARATION-INDEXED tables that no
  deduplication reaches: it declares 492 functions and 499 top-level names against caps of 128.

Raising capacities is a worst-case-memory change, which is this project's value proposition, so it
is not mine to take. The price is measured and pinned: **+33 KiB of shared data against a 16 MB
ceiling.**

### THREE FALSE REJECTIONS, ALL FOUND ON REAL CODE OR BY ASKING THE SYNTAX TREE

Every one made the stage refuse a program the reference accepts — the direction a type checker may
not take, since rejecting a valid program is a language change.

1. **A `match` arm binding.** Every program using one was refused. Found by a well-typed control.
2. **A `for` loop variable.** Every program containing a loop was refused. Found by running the
   stage against its own sources — no hand-written control in the file contained a loop.
3. **A const parameter used as a value**, and **an imported native called through its module path**.
   Found by censusing the binding forms instead of waiting for the next accident.

**The lesson that generalises**: a corpus of rejections cannot detect over-rejection by
construction, and every well-typed control was a snippet I wrote. Real code was three lines of
`include_str!` away the whole time.

### TWO INSTRUMENTS WORTH KNOWING ABOUT

- **The rule-shape census** crosses rule SHAPES against the syntactic FORMS each should govern. It
  found eight gaps on its first run where the inventory said the rules were complete — including two
  rules that existed and *could not fire*. **A rule inventory counts shapes, not the forms each
  shape reaches.**
- **The channel-withholding census** empties one input channel at a time and requires some verdict
  to change. All eleven are depended on. Its first, one-directional version got one channel wrong: a
  channel whose absence trips a fail-closed guard is invisible to an instrument watching only for
  acceptance.

### ONE SAVING IS DELIBERATELY REFUSED, AND PINNED SO IT STAYS THAT WAY

Occurrence rows that cannot reject are **still sent**, at a measured cost of 39% of that channel.
Eliding them would have the host withhold a row *because it knows the rule's answer* — the
marshalling objection running backwards, invisible in every verdict. The pin fails if the rows stop
arriving, so the argument must be met rather than bypassed. **If the trade becomes worth making,
delete that test deliberately and say why.**

### A PROCESS RULE THAT COST FIVE RUNS

**A push cancels the running check for that pull request.** Five consecutive runs were cancelled and
none ever completed, because a self-paced loop woke every twenty minutes against a run taking forty.
Once a branch is ready, stop pushing and let it finish; work that cannot wait goes on a separate
branch. Recorded in [GIT_STRATEGY.md](./GIT_STRATEGY.md).

**And a fourth way a local check under-reports**, joining the three on record: a cached clippy run
prints nothing whether or not warnings exist. Force a rebuild before believing a zero.

## FIFTY-FOURTH INCREMENT: WHICH INPUT CHANNELS ANY VERDICT DEPENDS ON

**The claim being measured is the central one**: the host supplies SYNTAX and the STAGE performs the
join. Two channels had a withholding proof; the other nine were credited with work nothing checked.

**Result: all eleven channels are depended on.** Each has at least one NAMED program whose verdict
changes when that channel alone is withheld — named rather than counted, because a tally cannot be
checked and an attribution can.

### The first run was one-directional and got a channel wrong

Measuring only "does an ill-typed program flip to ACCEPT" reported the declared-parameter-counts
channel as depended on by nothing. **It is not unused**: withholding it puts every call-site index
out of range, and the stage REFUSES an out-of-range index rather than skipping it, so the program is
rejected either way for different reasons. **A channel whose absence trips a fail-closed guard is
invisible to a one-directional instrument.** A well-typed corpus and change-in-either-direction made
it observable.

**My prediction of which channels would be inert was wrong** — I guessed the operand pairs and the
call sites; it was neither.

### What it establishes, and what it does not

**Establishes**: the stage reads all eleven. **Does not establish**: that it DERIVES its conclusion
from each — a channel can be read and still be redundant with a conclusion arriving elsewhere.
Dependence is necessary evidence for the claim about where the join lives, not sufficient. And a
channel that flips nothing is not thereby inert; it may be a corpus gap, and this instrument cannot
tell those apart.

### An assertion that replaced itself

The test first pinned the set of channels nothing depended on, expecting it non-empty, with a
failure message saying an empty set is the good outcome and the assertion should be replaced. It
became empty and the message was followed. **A non-vacuity assertion that says what to do when it
stops holding is worth more than one that merely fails.**

## FIFTY-THIRD INCREMENT: A KIND THAT WAS ALREADY GENERAL, UNDER A NAME THAT HID IT

The census left five gaps. **Three of them said the same thing**: "an agreement between a DECLARED
type and an ACTUAL one" — which is the only thing node kind 8 does. The constant was called
`TAIL_VS_RETURN`, after the function tail, its first and for a long time only caller, and **I wrote
that mechanism down three times without seeing the kind was already there.**

**A constant named after its first caller reads as a special case even when it is a general rule.**
That is the previous finding one level up: there a rule was present and unreachable; here a kind was
present and unrecognised.

**Four cells closed with no new kind** — array index against `Word`, a `let` against its annotation,
an assignment against its declared field, and a tuple index through the existing projection kind.
**Census: 15 covered, 1 gap**, from 8 and 8.

### Measured before writing, not after

A **Byte** index is rejected by the reference, so the requirement is `Word` exactly rather than "some
integer". And `let q: P = p` with a named type is ACCEPTED, so a named annotation must require
NOTHING — safe by measurement rather than by luck.

### The consequence of the reuse, which a test found

Once three more constructs use kind 8, **"kind 8" no longer means "a function tail"**, and the
pipeline differential that selects rows by kind compared four uses against a subset. **Narrowing that
test's filter would have been the wrong repair** — it would still have compared two populations while
reading as if it did not. The new rows are OPT-IN instead, the pattern the field-read index set.

### The one gap left, and why it stays

"Must not be bool", for the negation operand. Every kind here states a POSITIVE requirement. A
negative one needs its own kind, and one operator does not justify it. Left open as a decision.

## FIFTY-SECOND INCREMENT: THE RULE-SHAPE CENSUS

The previous finding was that **a rule inventory counts SHAPES, not the syntactic FORMS each shape
reaches**. One accidental hit is a reason to enumerate the class, so this crosses the shapes against
the forms and measures every cell.

**First run: 8 covered, 8 gaps. After closing what needed no stage change: 11 covered, 5 gaps.**

### The two surprises, which justify the census by themselves

**"A scalar cannot be projected" was a gap for BOTH its forms.** The rule exists and its node kinds
exist — but the set of names it could fire on held only `let`s carrying a primitive ANNOTATION. Not a
declared PARAMETER, not a `let` bound to a literal. **A rule that is present and unreachable looks
identical, from any inventory, to a rule that is present and working.**

**"Logical operator operands must be bool" was absent**, and agreement cannot substitute for it:
`n andalso m` with two `Word`s AGREES. **Two operands can agree and still both be wrong.**

### Closed, and with what

Three rules newly applied with the EXISTING condition kind and no stage change — the `when` guard,
the `not` operand, each logical operand. Two widened by enlarging the scalar set. The literal source
is restricted to four literal kinds, because `Literal::Fixed` yields an indexable `Multiword` and
calling it scalar would reject a valid `m[0]`; I could not write that program with the syntax I
tried, which is a reason to avoid the hazard rather than assume it away.

### The five that remain, each with its mechanism

Let annotation against initialiser, and assignment target against value — both want an agreement
between a DECLARED and an ACTUAL type, which the existing claims channel carries, except that it
carries TAGS and so reaches a literal but not a name. Tuple index on a scalar — a third expression
variant with the same rule. Array index must be a word — "must be T" for T other than bool has no
kind. Negation operand must not be bool — a NEGATIVE requirement, the weakest case in the table.

**The census is not exhaustive and says so.** These are the forms I thought of, which is the same
kind of list that missed match arms.

### A process failure, repeated

An edit script aborted on an assertion after `cargo fmt` reformatted what it was matching, so nothing
was written, and the test run that followed reported the unchanged result. I read that as the fixes
having no effect. **Second occurrence in two increments, and the first was already recorded** —
recording a failure mode did not prevent it. Scripts now exit naming the replacement that failed, and
the diff is checked before a test run is believed. A related miss: the compiler emitted `unreachable
pattern` for a guarded arm placed after an unguarded one, and my build grep filtered warnings out.

## FIFTY-FIRST INCREMENT: A FALSE CLAIM OF MINE, AND THE TWO GAPS CHECKING IT CLOSED

### The correction comes first because it is the point

For two increments I wrote that the remaining field-read cases **"need a type the source states
nowhere"**. **That was false for both**, and reading the abstract syntax tree settled it in one step:
an enum declaration lists each variant's payload types in order and a pattern says which variant and
position a name binds at; an array type expression carries its element type directly.

**It had been reasoned about rather than checked**, and it was the premise that would have justified
stopping. It reached the roadmap, the tasklog, this file, the design journal and a test doc comment
before anyone looked at the data types it was about.

### What that bought

**The match-binding case, for two tables and one scan.** Its base is a plain NAME, so the field-read
row and the operand form already handled it; what was missing was a third source for resolving a
name to a struct type. The host reports where a pattern binds (use site) and what the declaration
says is there (definition); the stage matches three coordinates. Withholding the declaration side
makes the same program accepted.

### A second gap, found by a test written for something else

A case was added to prove the stage discriminates between two VARIANTS of one enum. It failed,
because **the expression walk emitted no node for a match at all** — match arms were never compared,
and every program whose arms disagree was accepted.

**This was invisible from the rule list**, which records the fifteen enumerated shapes as complete.
The match-arms rule is the SAME SHAPE as the `if`-branches rule, and the shape had been implemented
while one of its two syntactic forms had not. A rule inventory counts shapes; it does not count the
syntactic forms each shape reaches. Worth carrying to any future "the rules are complete" claim.

Closed with the existing branch-pair kind and no stage change.

### A process note on the edit itself

One scripted edit aborted on a sanity assertion for a snippet I did not intend to change, so the
write never happened — and the test run that followed reported the OLD failure. I read that as the
fix not working before noticing the traceback above it. **An edit script that can abort before its
write, followed immediately by a test run, produces a result attributable to neither tree.** Same
class as the run-edited-while-in-flight finding already on record.

### What is left

One case: a field of an ARRAY ELEMENT. Its base is an index expression rather than a name, so no
field-read row can address it. The element type IS written down; what is missing is a base FORM on
the field-read row.

## FIFTIETH INCREMENT: THE DIRECT-OPERAND FORM, AND A NAME THAT ENCODED A TALLY

**One arm on each side.** `operand_form` gains form 2, whose value is a field-read ROW INDEX; the
stage resolves it through the binding case's own join behind a range check. `p.x + true` now types
where only `let a = p.x; a + true` did. **No new source of type information was needed** — the
struct-binding and field-tag tables already existed and the stage already searched them. What was
missing was a way for an operand to point at a read.

**The shortcut was refused and the refusal is recorded where the form is defined.** A synthetic name
would have made this work with NO stage change: register the read under an invented name, emit the
existing form-3 binding row, report form 1. Every test would pass. It is wrong because the invented
name IS the join — nothing in any source spells it, so the host would be asserting that this operand
and that binding are the same thing, hidden behind an identifier no reader can look up.

**The form reaches five node kinds and each applies a different rule**: binary operator, condition,
array element, branch pair, and function tail against its declared return. Both halves are pinned
per kind, because a wrong tag shows as a missed rejection while a tag where none belongs shows as a
REJECTED valid program — the error the previous increment demonstrated a rejection corpus cannot
detect.

### A process finding worth more than one increment

The pin that recorded what the channel does not reach carried its tallies IN ITS NAME -- three base
forms and two unreached. It was ONE INCREMENT OLD and already wrong, and correcting it rippled a
rename through five documents plus the citation guard's own commentary. The dead name is not quoted
here, because this file is one of the two the guard checks for exactly that. **A name that encodes a tally needs renaming every time the tally moves**, and every
citation of it goes stale at that moment. Renamed to a count-free pin; the tallies live in the body.

### What is left, and it is a different kind of gap

A field of an ARRAY ELEMENT and a field of a MATCH BINDING. Both need a type **the source states
nowhere** — an element type projected out of an array, a variant payload's type. The case just
closed was a missing CHANNEL, which is why it cost one arm on each side.

## FORTY-NINTH INCREMENT: THE CHANNEL LANDS, AND A WELL-TYPED CONTROL FINDS A FALSE REJECTION

### The finding first, because it matters more than the feature

**The stage REJECTED well-typed programs that bind a name in a `match` arm.** The occurrence
channel collected locals from parameters and `let` statements only, so an arm's `p` resolved to
neither a local nor a declaration and the classification rule refused it.

- **This is the unsound direction.** The sibling `verify_*` stages may over-approximate, because an
  over-approximation defers to a runtime guard. A type checker may not: rejecting a valid program
  is a LANGUAGE CHANGE.
- **It predates this increment.** Confirmed by stashing the working tree and running the probe at
  `HEAD`, on a program with no struct and no field read, so no new code could fire.
- **It is the SECOND binder this channel has missed**, after the `for` loop variable already pinned
  in the same file.
- **A rejection corpus could not have found it.** A checker that rejects everything scores
  perfectly against one. It was found by a **well-typed control** added for the field-read work.

Fixed by collecting pattern binders recursively, shorthand struct fields included, and pinned with a
must-fire control so the fix cannot have been "call every name a local".

### The channel

`let a = p.x` now proves a tag, through two joins the STAGE performs: the base name to a struct
type, then that type and the field name to the field's declared tag. The host reports that `p` is
written `P`, that `a` is written `= p.x`, and that `P` declares `x` as `Word`. None of those is the
conclusion, and withholding the declared field sets makes the same program ACCEPTED, which is what
tells a join from a marshalled answer.

**Struct identity is deliberately not a tag.** Folding struct indices into the scalar space 1..4
would put struct operands into every channel the disagreement predicate feeds, where a mismatch this
slice never reasoned about would reject a valid program.

**No new fold phase, no change to the declared step bound, no new opcode, no `BYTECODE_VERSION`
change.**

### What remains, and one item is a limit the sizing did not model

| unreached | why |
|---|---|
| base is an array element | the base is not a plain name, and the `let` states an ARRAY |
| **the field read is a DIRECT OPERAND** | the channel binds a NAME to a field read; an operand row has no form for one, so `p.x + true` types nothing even with `p` declared |
| base is a match binding | both of the above at once |

**"Three of five" and "three base forms of five cases" are not the same statement.** The sizing
spike placed every field read directly as an operand and measured a host-side lookup, so the
direct-operand limit could not appear in it. The second statement is the true one.

## FORTY-EIGHTH INCREMENT: THE FIELD-READ STEP IS PARTIALLY CHEAP, AND NOW MEASURED

The previous increment found that the existing sizing spike measures a step already taken, leaving
the field-read edge **unsized**. This sizes it, with its own cases.

**Result: declaration lookup types 3 of 5.** The two it does not reach are a field of an ARRAY
ELEMENT and a field of a MATCH BINDING.

| case | reached by lookup |
|---|---|
| field of a struct literal | yes |
| field of a field | yes |
| field of a call result | yes |
| field of an array element | **no** -- the `let` states an ARRAY; the element type must be projected out of it |
| field of a match binding | **no** -- the binding's type comes from the VARIANT PAYLOAD, which no `let` states |

**The mechanism is two lookups and no unification**: a `let` whose initialiser is a struct literal
or a call states its type outright, and a struct declaration states each field's. Nested access
repeats the pair. Nothing is inferred.

**So the next increment can be scoped rather than feared.** The cheap majority can land as a tagger
extension over declarations the pipeline already has, with the two projection cases recorded as
still unreached -- instead of the whole edge waiting on inference it may not need.

**Non-vacuity runs both ways, deliberately.** The spike fails if it types NONE, which would mean the
lookups are broken, and it fails if it types EVERY case, which would mean the corpus no longer
contains the edge. Each case also asserts the REFERENCE rejects the program, so a case that stopped
being a missed rejection cannot sit in the corpus unnoticed.

**The two unreached cases were predicted and then measured, not asserted.** Writing the prediction
into the corpus labels and letting the run decide is the difference between a sizing and a guess --
and this session has already recorded what happens when a plausible prediction goes in unchecked.

## FORTY-SEVENTH INCREMENT: THE CITATION GUARD CAUGHT ME NAMING A RETIRED TEST

A documentation-only pull request failed two continuous-integration jobs.
`the_current_claim_documents_cite_nothing_that_does_not_exist` fired on this very file, which
named the RETIRED test while explaining that it is retired.

**The guard is right and the fix is the claim, not the allowlist.** A current-claim document that
names an identifier existing nowhere asserts something no reader can check, and the guard cannot
distinguish a deliberate negative from a stale citation. This is the second time this session -- the
first was a begin command the slot stream did not have -- and both times the temptation was to
widen an exemption rather than reword.

**IT FAILED IN TWO CONFIGURATIONS I DID NOT RUN.** I ran the guards under default features and
under `self-host`; it failed under `--no-default-features` and `--features signatures`. That is the
feature-set lesson **that I had already written down in the handoff**, arriving in a new place: not
a gated test absent from a run, but a guard whose verdict differs by configuration.

**So I ran both document guards in every configuration continuous integration uses**, rather than
fixing the one that failed and assuming. All four pass for `comment_citations`; `claimed_counts`
reports ZERO tests under `--no-default-features`, which is itself worth knowing -- that guard does
not exist in that configuration, so a claim it protects is unprotected there.

**AND I DID IT A THIRD TIME WHILE WRITING THIS ENTRY.** The first draft named the phantom begin
command as an example of naming a phantom, and the guard failed again on the very paragraph
describing the rule. **Knowing the failure does not prevent it; running the check does** -- which is
the same sentence `HANDOFF.md` already carries about numbering its own validity list, arrived at
independently in a different file.

**The transferable rule is now four items long and this is the fourth**: before believing a green
guard, know which CONFIGURATIONS it ran in, not only which binaries and not only whether it stopped
early.

## FORTY-SIXTH INCREMENT: THE SIZING SPIKE SIZES WORK THAT IS DONE

Having found that type rejection's edge has moved to a FIELD READ, the obvious next step was to read
the sizing spike that measures what reaching further costs. It reports **"local propagation reaches
5 of 5"**, which invites the reading that the remaining step is small.

**It is a measurement of work already completed.** Every one of its five cases is a let-bound
literal, a call return, or a composition of the two -- and the stage now reaches all of them. The
spike was written when the edge WAS the literal operand; local resolution and the bounded arithmetic
fixpoint moved that edge afterwards.

**Its corpus contains no field read**, which is where the edge actually sits. So "5 of 5" sizes the
step behind us, not the one in front, and **the field-read step is unsized**.

**Kept rather than deleted**, with the limitation recorded in the spike itself. Its result is still
the reason the literal-to-local step was known to be cheap before it was taken, which is why it was
taken at all. What it cannot do is size what comes next.

**This is the third artifact in two increments whose answer was true when written and is now about a
different question** -- after the retired literal-only test and the Order 1 cell that cited it. The
pattern is not staleness of FACTS but staleness of SUBJECT: the instrument still works, and the
question moved out from under it.

## FORTY-FIFTH INCREMENT: THE OTHER HALF OF ORDER 1 WAS STALE TOO

With every region kind routed, the remaining Order 1 obligation was "source types before type
rejection reaches beyond literals". **Reading before acting, that is stale as well.**

The test the roadmap cites as pinning that limitation is **RETIRED**. Its name is deliberately not
repeated here: a current-claim document that names an identifier existing nowhere in the tree
asserts something no reader can check, and `tests/comment_citations.rs` refuses it -- which is how
this sentence was caught. The file that retired it says why: local resolution reaches a `let` bound to a
literal and a call taking a declared return type, so those programs are ordinary members of the
rejection corpus now. **A bounded fixpoint reaches an ARITHMETIC result too**, with no depth limit
on the chain -- the hop bound is a decision rather than a limit of the approach.

**The limit MOVED rather than vanished.** Its next edge was a FIELD READ, and the
forty-ninth increment above moved it again: what remains is pinned by
`the_field_read_channel_records_what_it_does_not_reach`. The field-read pin this
paragraph used to name no longer exists and is deliberately not quoted, since a claim
document that names a missing test asserts something no reader can check.

**Both halves of that sentence are now corrected in the roadmap cell itself**, not only in the
channels, because that cell is where the stale figure would be copied FROM -- which is exactly how
a stale capacity limit reached the handoff four increments ago. **This is the second time the Order
1 cell has been stale on these same two subjects**, so the correction says to derive the state from
the tests rather than from the cell.

**Worth noting about the coverage figure**: the cell now states that 100% of the region BYTES pass
through the stage while the share it DERIVES is unchanged. A reader taking "100%" as self-hosting
would be badly wrong, and the four provenance standings are what prevent it.

## A SMALL REPORTING ERROR OF MY OWN

I reported the trunk run green on the strength of a `--limit 1` row whose commit I did not compare
against the branch tip. It named a different commit; the run for the actual tip was still in
progress. **Checking what a result is ABOUT is the same discipline as checking what a command
covered**, and this session has now met that failure on both sides.

## FORTY-FOURTH INCREMENT: THE LAST REGION KIND, AND IT WAS THE WEAKEST

**Every region kind the corpus emits is now routed and byte-identical. The skipped set is EMPTY.**

`PARAM_TYPES` turned out to be the weakest rather than the hardest. It is a byte POOL, not a record
table -- `wire_schema` says why: a type tag is one byte, so a whole-word record per tag would waste
seven eighths of the region. The stage already had a pool path, and what it does there is COPY
bytes the host supplies, deciding nothing, because a pool has no offsets, widths or endianness to
decide.

**So it is routed and recorded at that standing rather than allowed to inflate a figure.** The
provenance table gains a fourth row, **copied, not encoded** -- weaker even than `HEADER`, which at
least decides a record's layout. Closing the set must not launder a memcpy into coverage.

**Two tests changed SHAPE rather than value.** The skipped-kinds guard asserted the set was
non-empty and bounded its size, with a message asking whoever emptied it to *"replace this test with
one asserting completeness"*. That day came. It now asserts EMPTY, so a kind reappearing reads as a
regression in the driver rather than an unrecorded gap, and the sequence stays in the comment
because it is the evidence: eight on 2026-08-22, six on 08-31, five on 09-04, zero on 09-11. The
share test's upper bound existed to catch an unrecorded advance; **there is no advance past
completeness**, so it becomes an equality between covered and total bytes.

**The distinction the coverage tests exist to protect is now at its widest, and is stated that
way**: 100% of the BYTES pass through the stage, and the share the stage DERIVES is unchanged.
Three of the four kinds that closed the gap supply only their name from the interner -- the
`CHUNKS` standing, not the `NAMES` one -- and the fourth supplies nothing at all.

**Four kinds, four shapes**: index, walk, step-and-accumulate, copy. None was a copy of the one
before it, and the plan that said two of them were the same shape was corrected before it produced
an emitter.

## FORTY-THIRD INCREMENT: `ENUM_LAYOUTS` ROUTED, AND A THIRD DERIVATION I MISSED

The third and last ROUTABLE region kind. `PARAM_TYPES` alone remains, and it needs an emitter
WRITTEN rather than routed.

**This one computes TWO of its four fields.** The type name comes from the interner, as the other
two kinds' do, and `variants_first` is ACCUMULATED in the stage rather than relayed -- the same
accumulate-then-advance `ck_stream_step` performs for its three ranges. The host supplies only the
variant count and the minimum payload, and the count does double duty: a record field AND the
distance the cursor travels to the next enum's type name.

**Three kinds, three shapes, none a copy.** The slot section indexes; the variant section
interleaves and walks with a boundary flag; the layout section steps a whole enum at a time while
accumulating a range.

**Verified it actually routed rather than emitting an empty region** -- the `STRUCT_AUX` trap, where
a byte identity on an empty region passes while emitting nothing. Skipped set now `[1e]`.
Mutation-checked ONCE PER STAGE-COMPUTED FIELD: advancing the cursor by one fails it, freezing the
accumulator fails it.

## THREE WAYS A LOCAL RUN CAN SAY LESS THAN IT APPEARS TO, AND I MET ALL THREE TODAY

`wire.kel`'s chunk count has **THREE** independent derivations. My enumeration found two and missed
`tests/wire_self_compile_status.rs`; continuous integration failed on it. All three now read 492,
and the third says so in its own message.

**The run that was supposed to confirm the enumeration could not have.**

| how the run under-reported | which one |
|---|---|
| guards executed BEFORE the last edit | recorded earlier this session |
| `-p keleusma --test X` never enables `self-host`; `--workspace` unifies it on | found by continuous integration |
| **`cargo test` stops at the FIRST failing binary**; nextest runs them all | found by continuous integration |

**A fourth was self-inflicted in the same hour.** A workspace run was still in flight while its
subject was edited, and it then reported NO failures -- which cannot be true, since the third site
was still stale when it began. Its result belongs to no tree. **Discarded rather than read as a
pass**, which is the same rule this session already recorded when two gates shared one log file.

**The shape common to all four is that the run did less than I believed it did**, and only the
first and the last were written down beforehand. Believing a green result requires knowing what the
command actually covered.

## FORTY-SECOND INCREMENT: THE HANDOFF REFRESHED AFTER SIXTEEN INCREMENTS

It was refreshed at the twenty-fourth increment and the session is at the forty-first. In between,
the premise it carried -- that the large work is blocked -- was corrected, and two region kinds were
routed. **A resuming agent would have read that Order 1 was blocked and that no name-carrying region
kind was reachable**, both of which are now false.

The banner leads with what changed and what it cost:

- **81% to 99%**, two kinds routed, `highest_command` 181 to 185, and the two kinds that remain need
  an emitter WRITTEN rather than routed.
- **The five increments of reading that preceded one line of behaviour**, four of which corrected
  something that would otherwise have been built on, none of which reached code.
- **The feature-set trap**: `cargo test -p keleusma --test X` does not enable `self-host` and CI's
  `--workspace` unifies it on, so a gated test can be absent locally and run in continuous
  integration.

**Three validity items added**, for the host-contract observation, the node budget, and the pair of
tests that pin `wire.kel`'s chunk count from two independent derivations. The list reads 1 to 21
with no inversion, **checked by rendering it** rather than by writing the next number -- the
distinction this file has paid for three times.

Every check was run rather than copied: fingerprint `0x4327_63E1`, the newer guards green under
`self-host`, the citation and count guards, and the boundary pin at 169 seconds actually executed.
Ancestry anchor moved to `fad3fe11`.

## FORTY-FIRST INCREMENT: `ENUM_VARIANTS` ROUTED, AND TWO FIGURES THE STAGE GROWTH MOVED

The cursor design landed as commands 184 and 185. **`no_region_the_driver_routes_disagrees_with_the_reference`
passed on the first run again** -- the region is byte-identical for every corpus stage, and the only
failure was the share figure asking to be told the new number.

| | before | after |
|---|---|---|
| self-hosted share of corpus region bytes | 98% | **99%** |
| skipped region kinds | three | **two** -- `ENUM_LAYOUTS`, `PARAM_TYPES`, both needing an emitter WRITTEN |
| computed share | -- | **unchanged** |

**Driven and mutation-checked**: three records across an enum boundary match the reference byte for
byte, removing the type-name skip fails it, and a cursor walked past its section is refused.

## THE PART WORTH KEEPING: A GUARD CAUGHT WHAT MY LOCAL RUN COULD NOT

Continuous integration failed on `selfhost_chunk_names.rs`, which pins `wire.kel`'s chunk count.
Four new functions moved it from **486 to 490** -- a function is a chunk, so the figure moved by
exactly four.

**My local check could not have caught it.** `cargo test -p keleusma --test selfhost_chunk_names`
reported *"0 passed; 0 filtered out"*: the test is `self-host`-gated and that invocation does not
enable the feature. CI runs `--workspace`, where cargo's feature UNIFICATION turns `self-host` on
because another member requires it. **`-p keleusma --test X` and `--workspace` are different feature
sets**, and a test can be silently absent from the first while running in the second.

That is the rule *a run that executed no tests is not a pass* arriving through a channel I had not
considered: not a filter, but a feature set that omits the test entirely.

**I predicted this class and under-enumerated it.** The plan said `wire.kel` is itself a measured
stage and growing it perturbs the corpus's figures. I guarded the NODE count with a new test and
never thought of the CHUNK count.

**So I enumerated instead of fixing the instance, and found a second live site**:
`tests/selfhost_parse.rs` pins the same 486, derived from the PARSED source rather than the compiled
module. Two independent derivations of one figure, which moved together as such a pair should. The
workspace run then confirmed those two were the only failures.

**Had I fixed only the failing one, CI would have caught the other and I would have called it a
surprise.** It is the one-of-two-sites shape this session has now met eight times.

## FORTIETH INCREMENT: `ENUM_VARIANTS` IS NOT THE SAME SHAPE, AND MY PLAN SAID IT WAS

With `DATA_SLOTS` routed, the plan's own "not in scope" section named `ENUM_VARIANTS` as "the same
shape with the enum base". **Checking before copying, it is not.**

`mi_enum_names` INTERLEAVES: for each enum it interns the type name, then that enum's variants,
then the next type name. So a flat variant index `k` does not sit at `ebase + k` -- the type names
are in the way, one per enum, at no fixed stride because enums have different variant counts. The
counters cannot supply the offset either: `vcnt` is the CURRENT enum's variant count and is
overwritten each iteration. **That is the same fact that defeated the slot base, biting a second
time in a different place.**

**The sound shape is a CURSOR, not an offset.** A begin sets the cursor to `ebase`; each step emits
one variant and advances by one, except that the host tells it when a record is the FIRST variant
of its enum and the stage then advances one extra to step over the type name. The host supplies
structure it legitimately knows -- the boundary -- and never a name index it cannot check.

**The sentence was written three increments before the walk was read closely**, which is how it came
to describe a shape the code does not have. It cost nothing because it was checked before being
acted on, and it would have cost a wrong emitter had it not been.

**The slot slice's transferable value is the METHOD, not the shape**: read the walk, capture state
from the walk rather than deriving it, let the host supply only what it decides, and say in advance
which coverage figure should move.

## THIRTY-NINTH INCREMENT: `DATA_SLOTS` IS ROUTED, AND THE SHARE WENT 81% TO 98%

The driver half landed. `slot_run_fields` groups consecutive slots sharing a name and visibility
into runs -- mirroring the encoder including its `u16::MAX` chunking, so a chunked run emits several
records all carrying the same run index -- and `window_emit_slots` drives commands 182 and 183 on
one virtual machine and ONE shared buffer.

**`no_region_the_driver_routes_disagrees_with_the_reference` passed on the first run.** The region
is byte-identical for every corpus stage. The only failure was the share figure, which is the test
asking to be told the new number rather than a defect.

| | before | after |
|---|---|---|
| self-hosted share of corpus region bytes | 81% | **98%** |
| skipped region kinds | four | **three** |
| computed share | -- | **unchanged** |

**The computed share not moving is the part worth checking, and it was predicted in advance.** The
stage supplies this region's NAME from its own interner and the host decides every other field --
the `CHUNKS` standing, not the `NAMES` one. `DATA_SLOTS` joins `CHUNKS` as **mixed** in the
provenance table, and the test that exists to stop the headline figure being over-read still holds.
Saying beforehand which number should move, and which should not, is what makes the one that did
mean something.

**`DATA_SLOTS` is the first routed kind whose record carries a NAME.** Every kind routed before it
carried none, which is precisely what let the host supply every field. That is why the interner
route had to exist before the routing, and why five increments went into reading before one line
changed.

**Verified**: 180 wire tests, 147 codegen tests including the boundary pin and the byte-identical
scaffold, all five region-coverage tests with updated figures, the node budget at 1,202 of 1,365,
`fmt`, and clippy with `-D warnings` across the full feature set.

## THIRTY-EIGHTH INCREMENT: THE MARGIN THE SLICE IS SIZED AGAINST WAS STALE BY A QUARTER

The plan's first step is to measure `wire.kel`'s node budget before editing it, so a later cap
failure is attributed to stage growth rather than to routing.
`tests/module_input_node_budget.rs` does that, parsing the count out of the blob the stage itself
reads.

**Measured: 1,194 nodes against a 1,365 cap, a margin of 171.** The figure quoted around the tree
is **1,148**, stale by 46 nodes, and the margin the plan assumed was 217. **A quarter of the assumed
headroom was already gone**, in the one number the slice is sized against.

**This is the fourth stale figure in this arc**, after the two capacity limits and the retained
state, and it is the one that would have mattered most: a slice sized against 217 that actually has
171 is a slice planned with a margin it does not have.

**The plan now cites the test rather than a number**, which is this project's own convention for a
figure that moves. The test asserts its walk has not drifted from the writer -- every length bounded
by the remaining blob, every section count cross-checked against the module -- because a parse that
drifted would report a wrong figure rather than failing, and a wrong budget is worse than no budget.

**The measure-first instruction paid on its first use.** It was written two increments ago as
process caution; it caught a real error the first time it was followed.

## THIRTY-SEVENTH INCREMENT: THE SLICE IS BUDGETED, NOT CASUAL, AND TWO CONSTRAINTS SAY WHY

Going to implement, two constraints surfaced that change the shape of the work rather than its
conclusion.

**Command 178 is already driven.** `the_four_record_formatters_lay_out_a_record_the_reference_agrees_with`
feeds `ds_stream_step` a name index from the reference's own record, which is legitimate for the
claim it makes -- whether the stage lays a record out as the format specifies. Changing 178 to read
the interner would break that for no gain, so **the name-aware step must be ADDITIVE**. The slice
adds TWO commands, a begin and a name-aware step, and `highest_command` moves 181 to 183.

**`wire.kel` is itself one of the eleven measured stages.** It carries **1,148 constant-forest nodes
against a node table holding 1,365** -- a margin of 217 -- and 475 chunks. Two new functions add
chunks and constants to the very stage the corpus measures, and that stage must still emit its own
regions afterwards.

**That is why this has not been done casually, and it belongs in the sizing rather than being
discovered mid-change.** The plan now says to re-measure the node count after the stage edit and
before the driver edit, so a cap failure is attributed to stage growth rather than to routing.

**This arc has now corrected itself four times**, each time by reading one level deeper: a dispatch
table, then a field list, then an assumption's consequence, now a contract and a budget. **Every
correction made the slice larger and better specified, and none of them reached code.** The plan is
executable now in a way it was not three increments ago, and the cost of getting there was entirely
in reading.

## THIRTY-SIXTH INCREMENT: THE ASSUMPTION HOLDS, AND THE SLICE STILL NEEDS A BEGIN

The plan named one assumption to check before anything else: that `wire.nmap` survives between the
interner call and the step calls under the driver's buffer handling.

**It holds.** `window_emit_chunks` creates ONE `shared` buffer and passes `&mut shared` to every
`enter_wire` call, begin and steps alike. The driver re-seeds only the slots it writes, and
`wire.nmap` is never among them, so the interner's result survives for as long as the same buffer
comes back -- which that function guarantees by construction. A `DATA_SLOTS` driver written the same
way inherits it.

**The question it leaves is sharper and smaller, and it changes the answer.** The slot stream had
no begin of its own, and the two commands that DO call `mi_window_prepare()` each do something else
as well: 174 zeroes the chunk range cursors, and 170 emits the `NAMES` records into the window. Either
would run the interner; both are misuses, one chunk-specific and the other writing bytes the driver
would discard.

**So the slice needs a begin after all -- for a different reason than the plan guessed.** Not
because the interner's result fails to survive, but because nothing currently runs the interner
WITHOUT also doing something a slot pass does not want. A begin whose whole body is
`mi_window_prepare()` is the smallest honest answer, and it moves `highest_command`.

**The plan was right to name the assumption and wrong about what would follow from it.** Checking it
still paid: the conclusion "needs a begin" is the same, the REASON is different, and a reason that
is wrong is how a design gets built against the wrong constraint.

## THIRTY-FIFTH INCREMENT: THE FIELD LIST MADE THE STATE LOOK SUFFICIENT, AND IT IS NOT

Going to route `DATA_SLOTS`, I checked the one thing the previous increment asserted without
reading: that the section bases are "recoverable from state that already exists", naming `ecnt`,
`vcnt`, `scnt` and the running `cnt`.

**`nm.vcnt` is assigned INSIDE the enum loop.** It holds the variant count of the *current* enum
and is overwritten on each iteration. `ecnt` and `scnt` are totals; there is no running total of
variant names anywhere. **The slot section's base cannot be computed from what the walk retains**,
and the previous increment said it could.

**Caught by reading the assignment site rather than the field list** -- the third surface-reading
failure in this arc, and the first of the three caught BEFORE it reached a line of code. That is
the whole value of writing the design down before executing it.

**The design is now recorded** in `docs/decisions/DATA_SLOTS_ROUTING_PLAN.md`: capture each
section's base at the moment that section starts, from the walk itself, rather than computing it
from counts. Two fields, two assignments, and the base cannot drift from the sequence it indexes.
`ds_stream_step` then takes the run index and reads `wire.nmap[slot_base + k]`, so no host
arithmetic touches a name.

**The assumption most worth checking first is named in the plan**: that `wire.nmap` survives
between the interner call and the step calls under the driver's buffer handling. `ck_stream_begin`
documents and relies on that property, but relying on someone else's documented reliance is not the
same as checking it. If it does not hold, `DATA_SLOTS` needs a begin after all and the shape
changes.

**Deliberately not implemented.** A half-finished change in the self-hosted emitter, against an
hour of continuous integration and a byte-identical oracle, would be worth less than a design the
next session can execute without re-deriving three increments of reading.

## THIRTY-FOURTH INCREMENT: THE MEASUREMENT I ASKED FOR WAS ALREADY IN THE TREE

The previous increment flagged two things as unmeasured before any driver change: whether the
interning ORDER for the enum and data-slot sections matches the reference's `SchemaBuilder`, and
whether `nmap` is indexed by walk position. **Both are settled by a test that already exists and
passes**, one inference away.

**The chain.** `NAMES` is emitted by `emit_name_records_from_nout(0, nm.cnt)` -- one record per
walk position, in walk order, straight out of the walk's own output. `NAMES` is ROUTED by the
driver at command 170. `no_region_the_driver_routes_disagrees_with_the_reference` asserts that no
routed region DIFFERS, across every corpus stage, with a non-vacuity floor of four identical
regions per stage.

**So `NAMES` is byte-identical for every stage, and it is the walk's output.** A byte-identical
table of name records in walk order is precisely the statement that the walk produced the same
names in the same order as the reference. The order is proven; it was not an open question.

**`mi_pair(k, len, mode)` writes `nin[k*2]` and returns `k + 1`**, so `k` is the walk position and
the pair sequence is indexed by it. Dedup reuses a POOL OFFSET rather than collapsing a record, so
walk position is the name index -- which is why `ck_stream_step` can read `wire.nmap[ck.j]` with
`ck.j` a plain chunk index.

**What remains is arithmetic.** Walk positions run sequentially across the three sections, so the
base for slot runs is the chunk count plus the enum name count, and the walk retains `ecnt`, `vcnt`
and `scnt`.

**Stated as an inference, because this session has over-claimed twice from a surface.** The chain
above is a reading plus an existing byte identity, not a direct measurement of the routed result.
**The definitive check is to route one kind and compare bytes**, and that is the next increment
rather than this one. What this increment establishes is that the check is now expected to pass for
a stated reason, instead of being attempted blind.

**The lesson is about where I looked.** I wrote down "this needs measuring" and the measurement was
sitting in a passing test the whole time -- the third time today that the tree already contained
what I was about to go and get. The cheap move, before sizing any measurement, is to ask which
existing green test would have to fail if the property were false.

## THIRTY-THIRD INCREMENT: THE NAME-INTERNING ROUTE ALREADY RUNS, AND THE GAP IS THE SECTION BASE

Having established that all four skipped region kinds wait on the name-interning route, I read what
that route actually does rather than sizing it from its name.

**The walk covers all three name sections, in one call.** `mi_window_prepare` calls
`mi_chunk_names()`, which TAILS into `mi_enum_names()` (line 3017), which tails into
`mi_slot_names()` (line 3051). The module-input blob carries chunk names, enum type and variant
names, and data-slot RUN names -- one per run, because interning per slot is what once produced the
395,804-name figure -- and `nm.cnt` advances through all of them, with `nm_mode_fresh()` used for
variant names exactly as `SchemaBuilder::intern_fresh` does.

**So `wire.nmap` already holds an interned index for every data-slot run name and every enum
name.** The route is not missing. What a router needs is the SECTION BASE: `ds_stream_step` would
take `wire.nmap[slot_base + k]` where it currently takes `wire.fin[0]`, and the interner retains
`ecnt`, `vcnt`, `scnt` and `ccnt` alongside the running `cnt`, so the bases are recoverable from
state that already exists.

**WHAT THIS DOES NOT ESTABLISH, and I am stating it because the previous increment over-claimed
from a surface.** That the interning ORDER for those sections matches the reference's
`SchemaBuilder` is NOT verified here -- the roadmap records slices 14b and 14c producing the enum
sequence with both modes, which makes it likely and not certain. Nor is it established that
`nmap` is indexed by walk position in the way a base offset would assume. **Both are measurements,
and the byte-identical oracle is what would settle them.** This increment is a reading, and the
next one should be a measurement before a line of the driver changes.

**The method that produced this is the one that failed last time, applied correctly**: read the
function bodies and follow the tail calls, rather than reading a name, a dispatch table, or a
count. `mi_chunk_names` is a misnomer for a walk that covers three sections, and sizing the work
from its name would have repeated the error exactly.

## THIRTY-SECOND INCREMENT: I OVER-CORRECTED A CLAIM THAT WAS ALREADY RIGHT

The previous increment "sharpened" `selfhost_region_coverage.rs` from *"all four wait on the
name-interning route"* to *"two are INTEGRATION and two are INVENTION"*, on the evidence that
`DATA_SLOTS` and `ENUM_VARIANTS` are dispatchable at commands 178 and 181, beside the routed 179
and 180.

**That was wrong, and the original sentence was right.** Dispatchable is not routable. Reading what
each formatter READS rather than which command dispatches it:

| formatter | first fields | routed |
|---|---|---|
| `sh_stream_step` (SHAPES) | tag, kind, reserved, size | yes |
| `sg_stream_step` (SIGNATURES) | params_first, params_count, ret, resume | yes |
| `ds_stream_step` (DATA_SLOTS) | **`dslot_off_name`** | no |
| `ev_stream_step` (ENUM_VARIANTS) | **`evar_off_name`** | no |

**The criterion is whether the record carries a NAME INDEX**, and it explains the whole set at
once. `SHARED_LAYOUT` and `DATA_INIT` were routed earlier on exactly that ground, and the driver
says so in its own comment. `ENUM_LAYOUTS` has `elay_off_type_name`, so it sits with the unrouted
two even though it has no emitter yet. A host-supplied name index could disagree with the interner
that produced `NAMES` -- the hazard the chunk path avoids by taking its index from its own
interner -- so the name route is a SOUNDNESS requirement, not a convenience.

**That criterion is the genuine sharpening, and it is the opposite of what I wrote.** Restored,
with the over-correction recorded in place rather than quietly replaced.

**The failure has a name: I read a dispatch table instead of the function bodies.** It is the same
shape as the message-based census classification two increments ago, which also read a surface
that looked like a taxonomy and was not. Twice in one session, the cheap signal was the wrong
signal, and the expensive one -- reading the code -- was the only one that settled it.

## THIRTY-FIRST INCREMENT: "ALL FOUR WAITING ON ONE THING" WAS TWO DIFFERENT THINGS

With Order 1 established as available, the next slice is one of the four region kinds the driver
still skips. `tests/selfhost_region_coverage.rs` said all four were "waiting on the name-interning
route" -- one sentence over two different states. Measured:

| kind | state |
|---|---|
| `DATA_SLOTS` | emitter `ds_stream_step` exists and is **DISPATCHABLE at command 178** |
| `ENUM_VARIANTS` | emitter `ev_stream_step` exists and is **DISPATCHABLE at command 181** |
| `ENUM_LAYOUTS` | READERS only (`elay_*`); no emitter written |
| `PARAM_TYPES` | no emitter written |

**The driver routes commands 179 and 180 -- `SHAPES` and `SIGNATURES`, the immediate NEIGHBOURS of
the two unrouted ones -- and everything else falls into its `_ => continue`.** So two of the four
are INTEGRATION and two are still INVENTION, which is exactly the distinction the roadmap's Order 1
cell draws and this comment collapsed.

**This is the third capability in one day that already existed and was not wired**, after the
streaming chunk emitter and the removed walk cap. The pattern is worth naming: on this line, a
stated blocker is as likely to be an unrouted capability as a missing one, and the cheap check is
to look for the dispatch entry before sizing the work.

**Deliberately not implemented in this increment.** Routing them is a driver change against a
byte-identical oracle, and the honest deliverable here is the sizing: a named, dispatchable slice
rather than a vague blocker. What it would buy is also stated in advance -- the PRODUCED share
rises and the COMPUTED share does not, because these records format fields the host decides, and
`the_computed_share_is_smaller_than_the_produced_share` exists so that cannot be misread.

## THIRTIETH INCREMENT: I COPIED A STALE FIGURE WHILE CORRECTING A STALENESS

The previous increment corrected the handoff's "the large work is blocked" premise and listed what
Order 1 actually needs, taking the detail from the roadmap cell. **Two of those details were
already false**, and I had not checked them.

**Both capacity limits are REMOVED, and the windowed path reaches all eleven stages.**

| limit as I restated it | actual state |
|---|---|
| `parse`, 94 chunks against a 90-record batch | **gone** -- the chunk region became a STREAM, one record per call, the coroutine carrying the three range cursors in private data across the loop's RESET |
| `wire.kel`, 1,148 nodes against a 1,024-node walk cap | **gone** -- the guard was comparing against `nm_max_names()`, a bound on the NAME arrays, where it should have checked the node table's 1,365; every constant in that stage is `Int`, so the walk interned nothing |

**The tree already said so, in the body of the test that proves it.**
`the_windowed_path_reaches_every_stage_it_can_walk` explains both removals and says its `Expect`
enum is gone because neither exclusion survives -- while its OWN DOC COMMENT listed both as live.
A doc comment contradicting the code beneath it is worse than an absent one, because the comment is
what a reader quotes: the roadmap cell carried both figures, and I copied them into the handoff
from there.

Three places corrected: the test's doc comment, the roadmap cell, and the handoff bullet. **The
roadmap is corrected as well as the handoff because it is where the figure was copied FROM** --
fixing only the copy is the one-of-two-sites failure this session has now met seven times.

**A search lesson worth keeping.** I concluded the streaming emitter was unreferenced outside the
stage, because `grep ck_stream` found nothing in Rust. The driver addresses the stage by COMMAND
NUMBER -- `CMD_BEGIN = 174`, `CMD_STEP = 175` -- so a name search across the language boundary
could not have found it. **A cross-language call site is invisible to a single-language grep**, and
the conclusion was wrong until reading corrected it.

**What remains true for Order 1** is what the previous increment said it was, minus the capacity
claims: four region kinds of twenty, with `HEADER` encoded but not derived and `CHUNKS` mixed per
field, and source types before type rejection reaches past literal, direct occurrences.

## TWENTY-NINTH INCREMENT: THE PREMISE THIS SESSION RAN ON WAS TOO STRONG

**I re-derived what the roadmap says is outstanding instead of continuing on momentum, and the
framing that kept this session in one lane for twenty-eight increments does not hold.**

`HANDOFF.md` has said, since before this session, that *"the large remaining work is blocked on
[the four decisions] and the small remaining work is not worth choosing over them."* The four
decisions block the LANGUAGE-SURFACE work they name -- `Text<N>` programs, the width API, the
float-verify semantics, a build's continuous-integration cost. **They do not block Order 1**, which
`docs/roadmap/V0_2_X_ROADMAP.md` identifies as the largest remaining workstream and whose own cell
says what stands in the way is *"integration, not invention"*:

- **The remaining region kinds.** The module-driven emit path covers FOUR of twenty, and unequally:
  `NAMES` and `STRING_POOL` are COMPUTED, `HEADER` is encoded but NOT derived, `CHUNKS` is mixed
  per field with ten fields per record host-supplied. Two capacity limits are named with numbers --
  `parse` at 94 chunks against a 90-record batch, `wire.kel` at 1,148 constant-forest nodes against
  a 1,024-node walk cap.
- **Source types.** Type rejection reaches only literal, direct occurrences, because no stage
  computes source types and `parse.kel` says so in its own comment. A missing pipeline capability,
  not a missing rule.

**The roadmap carries four open decisions of its OWN** -- cryptography locus, meta-circular bound
composition, version granularity, reference retirement -- and they are a different four. None
blocks Order 1 either. **Two sets of four decisions, neither of which blocks the largest
workstream**, and the resemblance is probably why the conflation went unnoticed.

**The verification work stands**: it found a real runtime defect, corrected several claims, and
strengthened the instruments. What does not stand is the inference that nothing larger was
available.

**No guard catches this.** It is a judgement rather than a figure, so it survived a refresh of the
very file that carries it -- four increments ago, by me, while I was checking eighteen other things
against the tree.

## TWENTY-EIGHTH INCREMENT: A HOST'S MISTAKE REPORTED AS THE ARTEFACT'S

Census groups F and J are the host-contract surfaces, judged lower value on the grounds that a host
supplying a mis-sized buffer or an unregistered native has broken a stated contract. True, and not
the whole question.

**`InvalidBytecode` means *this artefact should never have been produced*.** When a HOST's mistake
carries it, the message directs the reader to distrust the bytecode -- the one thing that is not
wrong. This file already records that for ONE hot-swap site, as an open API observation.

**Measured: it is not one site.**

| host mistake | variant |
|---|---|
| hot swap whose data vector length mismatches the new module's private slot count | `InvalidBytecode`, naming the mismatch |
| calling a native the host never registered | `InvalidBytecode`, naming the native |
| calling an entry point with an argument it does not take | **NOT `InvalidBytecode`** |

**The third row is the control, and it is what makes the first two mean anything.** A runtime with
one error variant could not be said to choose it wrongly. This one distinguishes -- an
argument-count mistake gets a different variant, as does a late read under read-before-resume,
measured two increments ago as a `TypeError`.

So the observation generalises from a single site to **both host-contract groups**. Every refusal
is CORRECT, each names the actual mismatch, and none of this is a defect report. **Which variant
carries them is the operator's call**, and it is a breaking change either way.

## TWENTY-SEVENTH INCREMENT: THE CLASSIFICATION METHOD FAILED, AND THAT IS THE RESULT

Naming the unprobed members of census groups E and I needs a per-line classification of the
forty-seven sites, which the document has never published. **It was attempted and the method
failed**, and the failure is recorded rather than the guess.

**Assigning each site by what its MESSAGE says** produced group sizes disagreeing with the table in
four places, with two assignments demonstrably wrong on inspection:

- `"no entry point"` and `"empty call stack"` would not place. Neither is an index out of range, a
  composite operand form, a data-segment layout, or any other group's stated subject -- yet both
  are in the population and the table's parts sum to its whole.
- Group H tallied at four; its own section names exactly three. The fourth was a message that
  READS like a should-never-have-been-emitted case and is not one.

**A classification with two known errors is worse than none**, because this document's value is
that every site carries a verdict, and a wrong row silently moves a verdict onto a site it was
never made about. The messages were written to help a reader diagnose a fault, not to encode a
taxonomy, and several are equally plausible under two group definitions.

**What a sound derivation needs is recorded**: reading each site's surrounding code against the
group's stated subject. Until then the unprobed members can be counted and not named -- the state
the document has been in since it was written, now with the reason written down instead of assumed.

**This is the third time this session a measurement's output was a corrected or refused claim
rather than a new fact**, and the second time the honest result was to ship nothing but the
reasoning.

## TWENTY-SIXTH INCREMENT: THIS LINE OF WORK ADDED A CENSUS SITE AND DID NOT TELL THE CENSUS

Deriving the `InvalidBytecode` population from source to name the unprobed members of groups E and
I turned up something else first: **the source has 47 construction sites and the census says 46.**

**The site is ours.** The opaque-width repair of 2026-09-08, part of this same line of work, added
`"flat opaque field read out of bounds"` to `src/vm.rs`. Confirmed by counting at the census's own
commit -- 50 raw matches then, 51 now -- and by diffing the message multisets, which names exactly
that string as the addition and nothing as removed.

**A source-derived guard existed and did not fire, BY DESIGN.**
`the_invalid_bytecode_census_still_describes_the_tree` scans the runtime sources and compares
against the stated figure with a tolerance of plus or minus four. Its own message explains why:
*"the comparison carries a small tolerance rather than pretending to exactness the scan cannot
deliver."* A drift of one sits well inside it. **The tolerance that makes the guard robust also
makes it blind to exactly the movement that happened** -- a property of the instrument, not a
mistake in it.

**My first account of this said "nothing noticed and every guard was green", and that was wrong.**
I found the tolerant guard only when my edit broke its extraction. Corrected before it entered the
tree, and both versions are visible in this increment's history.

**An exact counter now sits beside the tolerant one.** It can be exact because it reproduces the
document's exclusions mechanically -- comments by a strip, the match arm by its `(_)` pattern
position, the unit tests by truncating at the test module -- which is the distinction the tolerant
guard declines to make. Both are kept deliberately: a tolerant alarm that survives refactoring, and
an exact one that notices a single site. Mutation-checked: dropping the pattern-position exclusion
fails it.

**The new site got a ROW, not a reclassification.** Its sibling -- the identical bounds guard on a
flat `Text` field -- was already in the population, so the two plausibly belong together. But this
census publishes no per-line classification, and **guessing which group the sibling occupies would
be worse than leaving the new site in a row of its own.** Group K holds it, verdict "not examined",
and folds in if a future pass derives the classification. Population 47, examined 37, remainder ten.

## TWENTY-FIFTH INCREMENT: THE HANDOFF REFRESHED AFTER NINE INCREMENTS

`docs/process/HANDOFF.md` was last refreshed at the fifteenth increment and the session is at the
twenty-fourth. Its banner described three lines of work that predate the width floor, both sweeps,
the runtime grid, the float differential, the census closure, the indirect-site enumeration and
group G. **A resuming agent would have got a picture nine increments out of date**, and that file
is the designated resume anchor.

**Every validity check was RUN, not copied forward.** Fingerprint `0x4327_63E1`, SEVEN crates,
fifteen named guards green under `self-host`, the floats-absent test green under
`--no-default-features`, the boundary triple pinned, and the two long ones -- region coverage at 66
seconds and the boundary pin at 169 -- actually executed rather than assumed.

**One check nearly reported a false pass.** Running item 12's test by name gave *"0 passed; 38
filtered out"*, because it lives in `claimed_counts.rs` rather than the binary I guessed. **A run
that executed no tests is not a pass**, and the only reason that did not enter the record is the
rule already being written down.

**The list reads 1 to 18 with no inversion, checked by RENDERING it** rather than by writing the
next number -- the distinction this file has paid for three times. Items 16 to 18 cover the width
floor, the descriptor and runtime sweep, and the guard census with its two companion pins.

**The banner carries what is NOT established, not only what is.** Fourteen shapes is not every
construct; reach is proven at `narrow-word-16` and no other narrow selector; the census's
population remains a lower bound; groups F and J and one member each of E and I remain, none
individually named. The four corrections are kept at the same prominence as the findings, because
three of the nine increments produced a corrected claim rather than a new result.

## TWENTY-FOURTH INCREMENT: GROUP G'S OTHER TWO SITES, AND A GUARD THAT REFUSED MY ARITHMETIC

Group G's entry read "no witness found (1 of 3 probed)", with the other two described as
"host-supplied opaque handles going stale, which is a different question and untested here". Both
routes are now probed in `tests/opaque_across_reset.rs`, and **neither reaches an
`InvalidBytecode`.**

**Route one is closed at COMPILE time.** A persistent `data` slot's body survives RESET, so a
composite bearing an opaque stored there would carry a registry index across a reset. It cannot be
stored there at all: *"opaque types are not yet admissible in data segment fields"*. The probe
admitted three outcomes -- resolve, fault, or refusal -- and the answer was the third; **it was not
guessed, and the first draft of the test said so by failing with the refusal message rather than
asserting a resolve.** The guard now asserts that MESSAGE, because a refusal for an unrelated
reason would leave the route open for every shape that reason does not cover.

**Route two produces a `TypeError`, not an `InvalidBytecode`.** `src/vm.rs` documents that a
yielded value stays arena-resident and must be decoded before the next `resume()`, "a read
afterward resolves to a clean stale error" -- a claim about a host-facing contract that nothing
checked. Doing the forbidden thing gives a `TypeError` naming read-before-resume. **The VARIANT is
the census-relevant part**, since group G is a group of `InvalidBytecode` sites, so the test
asserts it is not that variant rather than merely that the read failed.

**Then a guard I did not know existed refused the edit.** Removing group G's probe count moved the
examined total from thirty-five to thirty-seven, because this census's own convention is that a
verdict without a probe count extends to every member.
`the_census_group_table_adds_up_to_its_stated_totals` compared the table against the prose and
failed. **Re-derived in both places rather than adjusted in one**, which is precisely the failure
its message names: *"adjusting the total instead is how group G went missing from every remainder
list."* The remaining count falls from eleven to nine.

## TWENTY-THIRD INCREMENT: THE SITES THE INSTRUMENT CANNOT SEE, COUNTED

`INVALID_BYTECODE_CENSUS.md` derives its population by grepping for the variant where it is
CONSTRUCTED, and says plainly what that misses: a site propagating the error from a helper, or
mapping another kind into it, does not appear. It also notes that one such conversion exists and is
included only because the grep happened to see it.

**That conversion is `impl From<ScalarError> for VmError`, and the grep counts it as ONE site.** It
is one construction and many reaching paths: a malformed artefact arrives through every call that
can raise a `ScalarError`, and the table attributes all of them to a single group-A row.

**Enumerated: six call sites in `src/vm.rs` and four in `src/marshall.rs`**, each converting
through `?` in a `VmError`-returning function or an explicit `map_err(VmError::from)`. All ten were
read individually rather than assumed from the pattern. So "the population is a lower bound" now
has a number against it -- 46 constructed sites plus ten paths collapsed into one of them --
and `tests/invalid_bytecode_indirect_sites.rs` keeps it current. **A failure there is not a defect;
it means the census's figure has gone stale**, which is exactly what a lower bound cannot tell you
on its own.

**An overclaim caught by measuring it.** The guard strips comments, and the natural justification --
both files' documentation names these functions, so an unstripped count would include prose -- is
FALSE today. Raw and stripped counts are both 6 and 4, because every prose mention omits the
opening parenthesis the pattern requires. The strip is defensive, not load-bearing; the file says
so, and its decoy carries the offending shape deliberately so the guard still fails without it.

## THE CURRENCY GUARD FIRED, WHICH IS THE FIRST TIME THIS SESSION

Adding that file pushed the integration-test count past the tolerance in `tests/claimed_counts.rs`,
which reported that `CLAUDE.md` states 101 files against a tree holding 112.

**Re-derived rather than adjusted**: 1282 lib tests under `self-host`, 1275 under default features,
1327 integration `#[test]` functions across 112 files. Both occurrences in `CLAUDE.md` updated.

**The attribution was made truthful rather than convenient.** The line has always read "Measured
<date> at <hash>", and the obvious move was to write the current HEAD. That hash names a tree with
111 files, not 112, because the measurement includes the file the same commit adds. **No hash can
be written there truthfully**, and the line now says that instead of naming one that is wrong by
one.

## TWENTY-SECOND INCREMENT: THE LAST TWO CENSUS ENTRIES RESOLVED, AND THEY RESOLVED OPPOSITELY

`GUARD_REACH_CENSUS.md` had two entries reading "cited, not demonstrated". Both are now settled,
and **the useful part is that asking the same question of each produced OPPOSITE answers.**

**`forest_child_channels.rs`: repaired, strip shown load-bearing.** Its extraction splits on
`": "`, which a COMMENT satisfies as readily as a field. A line reading `// channel: Vec<u32>`
becomes the pair `("// channel", "Vec<u32>")` and enters the field list as a seventh channel that
does not exist -- and if a real field were removed in the same edit, a phantom would stand in for
it and the count would still pass. Measured: with the strip removed the new guard fails, naming
the phantom.

**`composite_escape_routes.rs`: safe by construction, established by MEASUREMENT.** Removing
`code_only` entirely leaves all ten tests in that file passing. A comment cannot contribute an
opcode name, because a line beginning `//` is skipped and the anchor line itself fails the
uppercase-identifier test -- two independent filters, either of which suffices.

**My first guard for that file was VACUOUS, and I reverted it rather than shipping it.** The decoy
produced the identical opcode list with the strip removed, so it demonstrated nothing. Retargeting
it at the per-line filters did no better: removing either filter still leaves the phantom rejected
by the other. **A test that cannot fail is worse than no test, because it reads as coverage.** The
brief for this increment listed that exact trap, and the measurement still had to tell me.

**A reversed judgement, with its reason recorded.** The census had argued that a fourth
near-identical test might be worth less than stating the gap. That was reversed on new
information -- the guard added to `forward_data_reference.rs` under the same argument was
mutation-checked and fails with its strip removed -- and the census says so rather than presenting
the new verdict as though it had always held.

## TWENTY-FIRST INCREMENT: A GAP I NAMED WAS MOSTLY NOT A GAP, AND THE PART THAT WAS IS CLOSED

The twentieth increment ended by naming "float arithmetic across a width-mismatched pair" as
unswept. **That sentence was nearly a mis-reading of the tree.**
`tests/float_arith_width.rs` covers exactly that property -- arithmetic honours the module's
DECLARED float width, not the runtime's -- with every test declaring a 32-bit float on a 64-bit
runtime, eight of ten narrowing sites established by MUTATION, and the other two argued
witness-free for a reason rather than merely unwitnessed.

**A limitation of the sweep is not a limitation of the tree**, and writing the first as though it
were the second is the quiet way a document overstates what is missing. Corrected in place.

**The part that WAS genuinely absent is now present.** That file runs every case on ONE runtime, so
it establishes the declared width governs THERE -- not that the answer is independent of the
runtime, which is what the phrase "honours the declared width" actually claims. The two authorities
are the point. The same declared-`f32` module now runs on an `f32` runtime and an `f64` one and
must agree BIT-FOR-BIT, on the same four witnesses that file already established as
width-discriminating, with the vacuity check re-asserted rather than inherited so a witness that
stopped discriminating fails loudly instead of agreeing trivially.

**Mutation-checked**: removing the `Op::Add` narrowing fails the new differential as well as the
existing test.

## TWENTIETH INCREMENT: THE THIRD WIDTH, FROM BOTH SIDES

The nineteenth increment left the runtime's float at `f64`, so the float authority was swept from
the module side only. It is now swept from both, and the corpus USES a float rather than only
declaring one.

**`f32` runtimes.** All sixteen word-address pairs now run on both float runtimes, so a module
declaring a sixty-four-bit float meets a runtime that cannot host it and is refused at load -- the
same asymmetry already swept on the other two widths.

**A float in a body.** A `Float` field between the opaque and the word after it puts the float
width into a computed OFFSET, exactly as the address width enters through the opaque. **The value
read back is the integer field, never the float**, because a shape whose expected value were a
computed float would report an `f32`-versus-`f64` rounding difference as a wrong answer, and that
is a width difference the sweep is not entitled to call a defect.

**21504 cells: 6800 ran and were correct, 14192 refused at load, 512 refused at compile, nothing
else.** Green at the default build, three narrow selectors, and a build with `floats` ABSENT, where
the float shape compiles out and the rest is unaffected.

**Both refusal counts land on their own closed forms.** The 512 are the float shape against the
sixteen no-float descriptors on each of thirty-two runtimes. The 6800 match the per-cell prediction
exactly.

**The prediction had to get BETTER to accommodate the float shape, which is the useful part.** It
previously assumed every shape runs under every admissible descriptor, so it could be a product. A
shape needing floats cannot run where the descriptor declares none, and that refusal comes from the
COMPILER rather than the loader. It now models each shape's own requirement, which is a truer
statement of what the sweep claims than the version it replaced.

**The control gives 200 findings**, against 120 on the sixteen-runtime grid and twelve on the
single-runtime sweep, still on exactly one shape -- and the count matches its own closed form:
eighty on no-float descriptors, eighty at `f5`, forty at `f6`, the last halved because only `f64`
runtimes admit a sixty-four-bit float.

**Three analytic agreements now stand** between a measured count and an independently derived one:
the cells that load, the cells the compiler refuses, and the findings the control produces. Each
tests the HARNESS rather than the runtime, and a sweep that skipped a runtime or refused for the
wrong reason would break them.

## NINETEENTH INCREMENT: THE SECOND AUTHORITY, AND A CHECK THAT THE NARROW BUILDS CAUGHT

**Every width is carried TWICE** -- by the module header and by the runtime type parameters of
`GenericVm<W, A, F>`. The load check refuses a module WIDER than the runtime and admits one
NARROWER, and that asymmetry is where the second authority becomes visible. It is where the
original opaque-width defect lived. The descriptor sweep varied only the module.

`Word` and `Address` are implemented for four types each, unconditionally, so **sixteen runtime
pairs are constructible in the DEFAULT build**. The grid generalises `composite_width_skew.rs`'s
two hand-picked runtimes to all sixteen.

**9984 cells: 3900 ran and returned the expected value, 6084 were refused at load, nothing else.**
Every refusal is a module wider than its runtime, which is the guarantee working. Green at the
default build and all four narrow selectors.

**The harness is checked against an independent path.** The loader's documented rule, evaluated per
cell from the descriptor and the runtime's own trait constants, predicts which cells load without
the loader's involvement. The measured 3900 agrees exactly. A sweep that skipped a runtime, or a
refusal from some check other than the width one, would break the agreement.

**The first version of that check was WRONG and the narrow builds caught it.** It used a closed
form -- the product of two triangular numbers -- which assumes the runtime grid and the descriptor
set span the same widths. They do not: the grid is over concrete Rust types and is identical in
every build, while the descriptor set shrinks with the build's maxima. Right at the default build,
wrong at all four narrow selectors: 2730 ran against 1170 predicted under `narrow-word-16`.

**The control says something about the defect, not only about the harness.** Reintroducing the
sub-floor address produces **120 findings** against twelve on the single-runtime sweep, still on
exactly one shape, and they appear on EVERY runtime that admits the module. The defect is a
property of the module's declared width; the second authority neither masks it nor creates it.

**A tooling note worth keeping.** Writing Rust source through a non-raw Python triple-quoted string
silently reinterprets backslash sequences: a Rust line continuation is consumed as a PYTHON
continuation and `\n` becomes a real newline. Three assertion messages reached the tree mangled but
still compiling, so nothing failed. Use a raw string.

## EIGHTEENTH INCREMENT: A WIDER CORPUS FOUND NOTHING, AND CORRECTED A CLAIM ANYWAY

The seventeenth increment wrote down that six shapes is not every construct. **Seven were added,
each for a width-derived layout property the first six do not stress** -- a `Fixed<4>` whose default
fraction count is derived from the word width, a `Byte` whose offset contribution is
descriptor-invariant while its neighbours' are not, stride COMPOSED with a field offset, stride
NESTED inside another array, a composite behind an enum discriminant, a const parameter erased to a
literal that feeds a size, and a `Multiword<2>` limb index.

**624 cells, every one ran and returned the expected value**, at the default build and at all four
narrow selectors including both eight-bit ones. The sweep now NAMES any cell that does not run, so a
shape refused everywhere cannot be mistaken for coverage.

**The value is not the negative result.** Re-running the control against the larger corpus produces
**exactly the same twelve findings on exactly the same one shape**. Two of the seven additions also
stride, and NEITHER reaches the defect. So the characterisation written the same day -- *"the array
stride multiplies an element size, so a zero-byte scalar surfaces there"* -- **was incomplete**.
Striding is not the discriminating property. The element must itself CONTAIN the address-sized
scalar. An array of words, or of arrays, strides just as much and reaches nothing.

**A widened corpus that finds no new defect can still correct a claim.** That is the transferable
part, and it is the second time this session a measurement's chief value was overturning a sentence
rather than finding a fault.

## THE INSTRUMENT DEFECT THAT COST THIS SESSION AN HOUR

I started a second verification gate while the first was still running, having deleted the status
file they both append to. The record interleaved two runs -- `CLIPPY=0` from one, the `ALLDONE` from
the other, which had FAILED clippy -- and **no line could be attributed to a run**. Then I edited
the gate script while it was executing.

That is the shape `NARROW_WIDTH_FAILURE_CLASSIFICATION.md` already records, *"a measurement taken
while its subject is being edited measures neither state"*, applied to my own log two increments
after writing it down.

**The repository already solves this and I did not use it.** `scripts/gate-in-worktree.sh` names its
log per gate and per commit and pins the run to an immutable commit in a detached worktree, and its
header states the reasoning verbatim: the rule that a gate result is valid only for the tip it ran
against "stops being a discipline anyone has to remember and becomes a property of the mechanism."

Its warning about STOPPING a gate was also correct in detail: a path-scoped kill of the driver left
`cargo test --features self-host` reparented and still running, exactly as its header says, and the
second target-scoped kill is not optional. The ad-hoc gate now writes one status file per run.

## SEVENTEENTH INCREMENT: THE AXIS THE CENSUS NEVER VARIED, SWEPT AND CLEAN

The lead the sixteenth increment opened is now measured. `tests/target_descriptor_axis.rs` sweeps
**every target descriptor the compiler accepts** -- word and address from the narrowest implemented
width to the runtime's maximum, each with no floats and with every float format the runtime
implements -- against six shapes chosen for the constructs whose layout is WIDTH-DERIVED.

**48 descriptors by 6 shapes, 288 cells, and every cell RAN and returned the expected value.**
Nothing refused, nothing faulted, no wrong answers. Green as well under all four narrow selectors,
including the two eight-bit ones, where the descriptor space shrinks with the runtime's maxima.

**The control is what makes that mean anything.** Removing the address floor and admitting sub-floor
widths produces TWELVE findings, each named by descriptor and shape, reproducing the sixteenth
increment's defect through this harness -- and the same run classifies ninety compile-time refusals
correctly, so the refusal path is exercised too.

**Only ONE shape of six reaches it.** The array stride multiplies an element size, so a zero-byte
scalar surfaces there and is absorbed everywhere else. A corpus of five ordinary programs could have
missed the defect entirely. That is an argument about how thin any small corpus's evidence is, not a
claim that this one suffices.

**The first draft of the sweep was wrong, and the sweep said so on its first run.** It required at
least one cell to be REFUSED, assuming some admissible descriptor would be rejected. None is. The
check asserted a property that had not been measured, inside a test written to measure properties.
It now constrains the descriptor set's SPREAD, which is what non-vacuity actually needs.

**What this does not establish** is written into the census beside the result: the population is
still a lower bound, six shapes is not every construct, and no group carrying a probe count is
closed by this.

## SIXTEENTH INCREMENT: A DEFECT UNDER THE UNPROVEN REACH, AND THE ARGUMENT APPLIED TO ONE WIDTH OF THREE

The session's own residue was the subject: two guard-reach claims it had NOT established, and the
question of how many more there were. Closing the first found a defect in the runtime.

**`Target::validate_against_runtime` had no floor.** It checked that the word, address and float
widths did not EXCEED the runtime's and never checked the other end. A target declaring
`addr_bits_log2 = 2` compiles. The layout sizes an opaque by the ADDRESS width, four bits is zero
bytes, and the fault surfaces at run time as `InvalidBytecode("NewComposite flat operand on
non-flat values")` -- a message naming neither the width nor the target, pointing its reader at
composite construction, which is the one place there is nothing wrong.

**The argument was already in the tree, twice, applied to the float width only.**
`validate_program_for_target` refuses `float_bits_log2` below 5 because such widths "are not
formats ... so a target declaring one produces bytecode nothing will run", and
`tests/float_arith_width.rs` records where that came from: the V0.3.0 line observed that widths 0,
1 and 2 collapse to ZERO BYTES. Neither sentence is about floats. `1 << bits_log2` over eight is
zero below 3 whichever field it names.

**It was found by a derivation that produced one, and the derivation was mine.**
`composite_width_skew.rs` clamps an address one step below the build's word, with a floor of 2.
Under `narrow-word-8` the word is 3 and the clamp handed back a four-bit address. **That is the
sixth time this session the class under repair has appeared inside the repair**, and it is recorded
because the frequency is now evidence about the work rather than about any one edit.

**The premise guard is the transferable part.** Every test in that file is about widths that
DIFFER, both targets are derived, and a derivation can collapse. If the address came out equal to
the word, all ten tests would pass while exercising nothing. Nothing would have reported it. The
file now asserts its own premise and fails loudly under `narrow-word-8`, where the word is already
at the narrowest implemented width and no narrower address exists. **A build the file cannot cover
is a different thing from a defect it has found**, and the message says which.

## THE REACH THAT WAS UNPROVEN IS PROVEN, FOR ONE BUILD, WITH A VALID CONTROL

Two of the four runtime sites that ask the layout for the opaque width were reverted to asking for
a word. **Each failed at the DEFAULT build** -- the control that makes the probe mean anything --
**and each also failed under `narrow-word-16`**. One failed STRICTLY MORE tests at the narrow width
than at the default, so that build is not a degraded copy of the default. The earlier attempt
failed at neither width and was nearly reported as evidence the corpus had gone vacuous.

No reach claim is made for any other selector. The eight-bit ones were run over this file and are
NOT clean: two losses at `narrow-word-8` and eight at `narrow-address-8`, every one inadmissible by
construction, enumerated with its reason. The clamp repair removed a third that was the zero-byte
opaque rather than a property of the corpus.

## THE PARITY GUARD'S SILENT DIRECTION, AND THE CONTROL THAT MATTERS MORE THAN THE RESULT

Only the false-FAILURE direction had been checked. Measured now: a real seeding call deleted from
the shipping driver with the identical text left in a comment. The guard FAILED, naming the slot
and both counts. **Then the control: with the comment strip disabled and the same mutation in
place, the guard reported `ok`.** The strip is load-bearing, which is a stronger statement than the
guard merely passing.

## AND THE CENSUS THAT CLAIMS TO ENUMERATE THAT CLASS DOES NOT NAME THE SITE

`docs/decisions/INVALID_BYTECODE_CENSUS.md` asks, at each site, whether a module a SUPPORTED
PRODUCER emitted and `verify()` ACCEPTED can reach it. One did, and no group names that message.

**The axis is the reason.** The census enumerated by the error constructed at each site and reasoned
about what a PROGRAM can express. A degenerate TARGET DESCRIPTOR is a different axis: the program is
ordinary and the module is malformed by the width it declares. The population derivation does not
reach that axis, and the assumption that the target is well-formed was never stated.

The route is closed, so this is a record of the census's REACH, not a live hole. **What is NOT
established is whether other sites are reachable the same way**, and the addendum says the next pass
should ask each group's question with the target descriptor as a variable rather than only the
program.

## THE CENSUS: SIXTEEN DEMONSTRATED, TWO CITED-BUT-NOT

`docs/decisions/GUARD_REACH_CENSUS.md`. The population is DERIVED -- the test files added or
modified between `639108fd` and `b74380a2` -- not recalled. Eighteen files. Sixteen record a
demonstration of their own guard failing. **Two do not**: `composite_escape_routes.rs`, whose
measured statements are about the tree rather than about the guard, and
`forest_child_channels.rs`, which cites a measurement made on a DIFFERENT guard. Both are named
with the cost of closing them, and neither is repaired. Inventing a fourth near-identical test is
not obviously worth more than saying plainly that two files rest on a property nothing checks.

**Three were repaired during the census.** `narrow_vm.rs`'s widened predicate said in prose that it
"deliberately does NOT accept any error at all" -- a claim, with nothing checking it; a later edit
relaxing it would have made three tests vacuous with all three still green. `forward_data_reference.rs`
documents that a comment naming an anchor can make its ordering assertion report the WRONG STRUCTURE
as a pass, and nothing checked that either. Both now have negative cases, and both were shown to
fail when the mechanism they depend on is removed.

## NINTH INCREMENT: THE CLASS CLOSED, AND A TRIPWIRE CHOSEN OVER NINE PARSERS

**The class is closed with a population and a verdict per file**, not abandoned when the obvious
cases ran out. `docs/decisions/COMMENT_MATCHING_GUARD_SWEEP.md` lists thirteen files: **nine
repaired, three safe by construction, one not in the class.** A line-prefix search is safe because a
comment line begins with `//` — a property of the search, not a judgement about the file.

**The last three.** `tests/selfhost_bare_for.rs` asserts the **absence** of a removed refusal in raw
`parse.kel`, and a historical note naming it makes the guard report that the stage *"still defines or
raises"* it — **a false failure that names a cause which does not exist**, sending its reader after a
definition that is a comment. `tests/selfhost_driver_parity.rs` counts seeding calls against a calibration, so
a comment adds a phantom. The false-failure direction was verified then; **the silent-false-pass
direction was measured on 2026-09-10 and the strip shown load-bearing** (see the sixteenth increment
above).

**The block-comment gap: measured, then tripwired rather than parsed.** All nine strips handle `//`
and none handles `/* … */`. **Exposure today is zero** — the only `/*` in the stage sources is inside
a line comment about the `+`, `-` and `*` operators, and every `src/*.rs` occurrence is in a doc
comment or test string. But Keleusma supports block comments, so the risk is latent.

Teaching nine helpers cross-line state is complexity bought for no current exposure, so
`tests/block_comment_tripwire.rs` fails if one ever appears and names the document. **That trades
certainty for proportionality, and is the judgement most worth reviewing**: the guards remain unable
to handle a block comment; the check only ensures nobody introduces one unnoticed.

Its detector deliberately does not fire on a `/*` inside a line comment or a string literal, both of
which this tree contains, and a second test pins those shapes — without it, tightening the detector
until it reported nothing would look like a fix.

**The transferable rule**: only an **absence** assertion loses silently to an early truncation.
Everything else fails loudly, so the naive strip is correct in eight of the nine.

## EIGHTH INCREMENT: FOUR MORE, AND THE DEFECT COMMITTED INSIDE ITS OWN FIX

**A third silent false pass.** `tests/stage_command_reach.rs` has a helper that strips comments and
whose doc cites *"four recorded instances of a guard firing on the prose that explains it"*, while
three **presence** assertions beside it searched the raw driver. Change `CMD_STEP`'s value, leave the
original text in a comment, and the test reports **2 passed, 0 failed**.

**Three files now**, each documenting the hazard in its own prose while guarding one of two readers.

**Three anchor-locates repaired.** One historical note failed three tests in
`composite_escape_routes.rs` with nothing wrong in the source it read.
**`forward_data_reference.rs` is the subtle one**: its positions feed an `at_fn < at_blk` ordering
assertion, so a comment can change which declaration appears first and make the test assert the wrong
thing about the stage rather than fail loudly.

**I committed the same defect inside its own fix** — computing one offset from the stripped copy
while the slice beside it indexed the raw string. Caught by running the tests, not by reading the
edit. **That is the most useful result of the increment**: it shows the class is not carelessness
that attention prevents, which is why a mechanical sweep found instances that four documented prior
incidents did not.

**Two controls measured nothing** because they did not compile — renaming a constant used elsewhere,
and adding an opcode to an exhaustively matched enum. Both replaced by controls that build; the
second shows the extraction panics loudly rather than returning an empty list.

**Seven guards examined.** Only the radix guard needs the string-aware strip, its assertion being an
**absence** one; the other six fail loudly and the naive form is correct. Deliberately not unified —
sharing a helper would add cost to six and remove a needed guard from one.

## SEVENTH INCREMENT: THE SWEEP, AND A FALSE PASS ON THE HISTORICAL DEFECT

Two instances of the comment-matching class were found by **reading**. Asking the class question
**mechanically** — which test files search source for a code-shaped literal without stripping
comments — found **twelve**. That is the difference between finding instances and finding a class.

**The one that mattered runs the opposite direction from the first two.** Those are **absence**
assertions, where a comment causes a noisy false *failure*. `wire_self_compile_status.rs` asserts the
**presence** of `forst.forin_count = 0;` — the exact line whose absence *was* the historical `wire.kel`
defect — and a comment satisfying a presence assertion is a **silent false pass**.

**Measured by isolating the test**: with the real reset deleted and the identical text left in a
comment, it reported **ok**.

**The file was not fooled.** A sibling *behavioural* test failed, because deleting the reset really
does break the stage. **But that backstop is incidental**: narrow the behavioural test, or change the
stage so deletion no longer breaks byte identity, and this assertion is the only defence and does not
hold. A guard named for the historical repair should not depend on a different test to be right.

**I nearly reported this wrongly.** The first run showed the *file* failing, which reads as "not
fooled". Only isolating the single test separated "the file fails" from "this assertion holds".

**Three comment-strippers now exist with three risk profiles** — string-aware for the absence guard,
naive for the two whose truncation fails loudly — and each says why it is not shared.

## SIXTH INCREMENT: THE SAME CLASS AGAIN, IN THE FILE THAT DOCUMENTS THE CLASS

Scoping the comment-matching defect **by class rather than by where I looked** found a second
instance — in `tests/op_tag_tables.rs`, **whose own doc cites a divergence detector broken by a
commented-out `for k in 0..3`.**

It has two source extractions. `decoder_arms` strips comments and **then** locates its anchor.
`stage_tag_table` did the reverse — `find` on the raw source, stripping only the block it found — so
the anchor search itself was comment-blind. **Measured: one comment line mentioning the anchor
failed four tests in that file**, with nothing wrong in the stage.

**The shape is the one this tree keeps meeting**: a case handled for one construct and not for the
one beside it, as with `rewrite_pattern_enum_name`, `check_pattern_against_type`, and `forin_count`.
Here both siblings sit in one file, one of them already correct, under a doc warning about the exact
hazard. **Knowing a hazard and guarding one of two sites is the recurring failure, not ignorance of
it.**

**The two comment-strippers are deliberately not unified, and both now say why.** The radix guard's
is string-aware because its assertion is an **absence** one, where an early truncation means a
missed offender passes silently. Here an early truncation makes an anchor or a field go missing and
fails loudly. The naive form is correct in one and wrong in the other, so tidying them together
would either add unneeded complexity or remove a needed guard.

**Both directions mutation-tested**: the comment that broke four tests now passes, and a duplicate
tag number still fails three. The second matters as much as the first — an extraction can be made
comment-proof by making it find nothing at all.

## FIFTH INCREMENT: A GUARD THAT COULD NOT COEXIST WITH A COMMENT ABOUT WHAT IT GUARDS

This repository records **four** instances of a guard matching prose it was never meant to read. One
guard is still exposed. `every_site_in_the_call_packing_family_agrees_on_the_radix` asserts that
**no** site splits a `Call` record on the old eight-bit radix, and it searched raw source lines.

**Measured**: adding a plain historical note — *"the Call record once split its chunk field as
`count * 256`"* — to a source file **fails the test**, with nothing wrong in the tree.

**The remedy already in the file is the part that worried me.** It records flagging itself once,
"the third time a guard in this repository has done that", and the fix applied was to skip the whole
file. That costs reach, and it is the fix a later reader copies.

**The obvious fix introduces the opposite defect.** Truncating at the first `//` cuts inside a string
literal like `"http://a"` and drops a **real** occurrence — and for an absence assertion that is the
dangerous direction, since a missed offender passes silently where a matched comment merely fails
loudly. The strip is therefore string-aware. **Block comments are not handled and the code says so.**

Three mutations: the historical comment now passes, a real code site still fails, and a real site
after a string containing `//` still fails. **The third is what justifies the complexity** — a naive
strip would have missed it.

**A run that executed no tests is not a pass.** The first demonstration reported
`0 passed; 0 failed; 0 filtered out` because the binary is gated on `self-host`, and I nearly read
that as the comment being harmless.

## THE NARROW-WIDTH LINE REACHED ITS FLOOR: 36 -> 13, NONE EXCLUDED

The standing claim was *"the whole suite at a narrow width is unverified — not shown broken, not
shown working."* **Twenty-nine failures repaired, nothing newly broken, and no test excluded.**

| stage | distinct failures | binaries green |
|---|---|---|
| start | 36 | 98 |
| after deriving `tests/narrow_vm.rs` and my own two files | 33 | 100 |
| after `tests/float_arith_width.rs` and `tests/composite_width_skew.rs` | **13** | **102** |

Every figure came from **diffing the failing sets**, never subtraction — a rule that earned its keep
twice here, once when a reduction concealed three new failures of my own, and once when a total
coincidentally matched a stale recorded figure.

**The thirteen that remain are real wide-word dependencies, checked rather than assumed.** Seven are
programs declaring `require word >= 32` — the **self-hosted stage sources**, fourteen of which
declare it, so the refusal is the directive working. One pins 64-bit semantics with a constant that
does not exist at sixteen bits. Two are Q-format fractions inadmissible at the width by
construction.

**Making any of them pass would mean weakening a program's stated requirement**, which is coverage
hiding rather than repair.

**The claim is now sharper, not finished**: the narrow width runs everything that can run there, and
what cannot is enumerated with a reason. That is not "the narrow widths are verified" — one
corpus's narrow-width REACH is still unproven, and the probe that would have shown it was invalid.

## WHAT I GOT WRONG THIS SESSION, KEPT BECAUSE THE CORRECTIONS COST SOMETHING

| claim | outcome |
|---|---|
| a red CI job was the **feature-set** trap | **wrong** — the same test fails under default features. The real cause: I ran the guards BEFORE the last edit and only `cargo fmt --check` after it |
| "every guard I wrote had a first-draft defect" | **overstated** — four of seven. Two were got right before shipping; one guard was fine and my characterisation of its output was wrong |
| two CI jobs looked **stuck** at 90 minutes | **wrong baseline** — those two take 60 minutes each and ~52 had elapsed. I nearly escalated a non-problem |

**And I fixed what the guard caught, not the class.** The citation guard scans two documents, flagged
two bare file names in one, and I corrected exactly those — leaving the identical names in the task
log's newest note because nothing pointed at them. **Same one-of-two-sites shape, committed while
cataloguing it.**

**A measured recommendation left as the operator's call.** Should the task log be scanned by the
citation guard? Not whole: sixty of its citations name identifiers that no longer exist, and
inspection shows most are legitimate — pins deleted as the work moved on. **A currency note is a
dated record, so its names going stale is the file working correctly.** Scanning only the newest note
would be bounded and meaningful, but it needs a rule for where a note begins and puts a recurring
cost on whoever writes the next one. Recorded, not adopted.

**A figure confirmed rather than trusted.** This file's CI timing — about 61 minutes with a
~59-minute critical-path job, measured 2026-09-08 — was independently reproduced today: `Test` and
`Test (self-host feature)` each take about 60 minutes and start together. **Three pushes to one
branch therefore cost three full cycles**, which is the argument for batching fixes rather than
pushing them as found.

## FOURTH INCREMENT: A DEFECT SHAPE TURNED INTO A ONE-SECOND CHECK

The first four increments produced measured negatives. This one took a defect that **actually
happened** and asked whether its shape is mechanical.

The `wire.kel` self-compilation failure's last cause was one line: `forin_count` was never added to
the per-function reset that already cleared its own documented analogue `forlimit_count`, and it
indexes an emitted record as `7 * forin_count`, so every function after the first emitted a record
pointing past its own parts. **Finding it took prefix bisection, a rebuilt dependency chain, delta
debugging and a five-line synthetic, with two of the four causes first diagnosed wrongly.**

The shape is a grep, and `tests/selfhost_counter_reset.rs` now is one.

**The class is clean** — three members, all in `parse.kel`. `aq_k` matched the dangerous shape on
the first pass and reading its assignments cleared it: it is reset at both of its construct entry
points, a **stricter** scope than per-function rather than a weaker one. Reporting it without
reading those two lines would have been a false finding.

**It deliberately does not check that a reset dominates its use.** That needs control-flow analysis
it has no business doing, and the sound resets sit at two different scopes, so demanding either
would flag the other.

**Mutation-tested against the historical defect**: deleting the 2026-08-27 repair reproduces it and
the guard names the field. A guard tested against a real past defect is a different object from one
tested against an invented mutation — the invented one asks whether it *can* fail, the historical
one whether it would have earned its cost.

**Its own reach was checked**, because all three members sitting in one file reads like a broken
scan. The accumulator half fires in all twelve stages; being multiplied into an index is what is
rare, and `parse.kel` is the stage that emits records with packed arguments.

## THIRD INCREMENT: ONE PROPERTY, THREE AXES, AND A REFUTED HYPOTHESIS

The float result was not a finding about floats. **Three widths are carried independently — word,
float, address — and each has two possible authorities**: the module header the compiler baked every
offset from, and the runtime type parameter of `GenericVm`. The load check does not force agreement.
It refuses a module whose width is **wider** than the runtime and admits one that is **narrower**, so
a module compiled for a small target and run on a large host is supported — and is where a second
authority becomes visible. **That is how the opaque defect presented.**

**The existing skew tests cannot see it.** Every configuration in `composite_width_skew.rs` is
matched; a matched pair cannot expose a second authority because both readings coincide.

**Both remaining axes are clean**, including the address axis the original defect lived on, which had
no coverage in this form. Word: 3 of 6 catch the mutation. Address: 2 of 3. **Each mutation fails
only its own axis**, so the two tests are independent rather than one guard firing twice.

**A probable hypothesis was tested and refuted.** Two word cases survive the mutation. Boxing was the
likely explanation — a sibling test records that a boxed composite agrees on both runtimes — and it
is **excluded**: all four shapes are flat at both module widths. Constant folding is excluded too.
The cause is unestablished and the test says so, recording what was ruled out. Had I not checked, a
plausible wrong cause would have entered the tree, which is how two of the four `wire.kel` causes
were first diagnosed wrongly.

## SECOND INCREMENT: AN AUDIT'S SCOPE ARGUMENT WAS AN INSTANCE OF THE ERROR IT AUDITED

`FLAT_FIELD_WIDTH_AUDIT.md` justified auditing only the OPAQUE flat field with one sentence: every
other kind is a function of the word or the float width, **"so a site assuming a word is correct for
them."** That is **false for `Float`** — a float field is sized by the float width, selected
independently of the word, and a word assumption is correct only where the two happen to be equal.
**That is the exact coincidence that hid the opaque defect.** The scope was right; the argument for
it repeated the mistake being audited. Corrected in place.

**The excluded class was then measured, and it is clean** — six flat-composite shapes, in the
configuration capable of exposing the defect: a module declaring a **narrower** float than the
runtime provides, which the load check admits since it refuses only a wider one.

**Two earlier attempts proved nothing, and the second is worth more than the result.** Mis-sizing
the LAYOUT is an **equivalent mutation** — invisible in all six cases, because the compiler's
offsets and the runtime's strides both derive from it and move together. That coherence is precisely
what the opaque field lacked. The defect needs **two authorities**: taking the VM's float width from
the runtime type rather than the module header supplies one, and three of six cases then catch it
with a **silently wrong value** rather than a fault — the opaque defect's own signature.

The three that catch it are the three that read a field **positioned after** a float; the others
read the float itself and land in the same place either way. `tests/flat_float_field_width.rs`
asserts that property by name rather than inventorying constructs.

**"No defect found", never "no defect exists."** Six shapes are not the class.

## WHAT THIS SESSION DID: THREE VERDICTS, NO KINDS MOVED, AND THAT IS THE OUTCOME

The type channel's last extraction has four of eight kinds on the pipeline. Of the four that are
not, the branch pair was already withheld with a recorded reason. The **three composite kinds** —
field access, index access, struct literal — carried only a note saying the two representations
"disagree about what a node IS", which is a reason to look rather than a finding.

Each now has a **measured verdict and a witness test**. None moves.

| kind | verdict | why, in one line |
|---|---|---|
| 5, field on a value | **WITHHELD** | reconstruction refuses the program; there is no forest to extract from |
| 6, index on a value | **WITHHELD** | same |
| 7, struct literal | **WITHHELD** | the record does not say which struct is built, and the size it does carry is not a substitute |

**Kind 7's witness is a proof rather than a survey.** A struct of one `Word` and a struct of eight
`Byte`s are both eight bytes wide. Supplying one field to each produces a **byte-identical**
struct-literal record while the reference accepts the first and rejects the second with exactly the
field-count error. No function of that record can reproduce the verdict, so emitting a row would
either reject a correct program or lose the check. **Both directions unsound** — the branch pair's
shape.

## THE PREDICTION WAS WRITTEN FIRST AND WAS WRONG IN BOTH HALVES

| predicted | measured |
|---|---|
| kind 7 **moves**, the declared count is already on the wire | **wrong**; the count is not on the wire at all |
| kinds 5 and 6 do not move, blocked on a missing **annotation** record | verdict right, **mechanism wrong**: they are blocked on the pipeline refusing the whole program |

Recorded as missed rather than revised. The kind-7 miss came from reasoning that a sibling
extraction already returns declared field sets — true, for a struct you can NAME.

## A FINDING BESIDE THE SLICE: A PUBLIC EXTRACTION PANICS, AND IT IS BOUNDED

`expression_rows_from_pipeline` **panics** rather than returning on programs the reference merely
rejects. Measured over eight ill-typed programs: four panic, four return rows.

**Proportionality, bounded twice over.** `self_hosted_compile` compiles with the reference FIRST
and surfaces the reference's own error, so such a program never reaches the pipeline; and it wraps
the pipeline in `catch_unwind` besides. **The shipping compiler is not exposed.** Exposure is to
direct callers of the extraction API, which today are tests. Recorded on the function, not repaired
— making a public API total is a contract change and there is no caller needing it.

## WHAT WOULD UNBLOCK KIND 7, AND WHY I DID NOT START IT

A record naming the literal's struct. `parse.kel` **already resolves the identity** — the field
records preceding a literal carry INDICES, and only a resolved declaration yields an index — and
then discards it. That is a **record-stream change**, which is your call on this fork, and the same
class of decision as the `let`-binding-name record you ruled on earlier. Not begun.

## VERIFICATION, STATED AS RUN RATHER THAN AS INTENDED

- `selfhost_typecheck` **41 passed, 0 failed** with `--features self-host`.
- `comment_citations` and `claimed_counts` green.
- `cargo fmt --check`, and `clippy --tests --features signatures,shell,self-host -D warnings` clean.
- The three feature sets **without** `self-host` compile: `--features signatures`,
  `--features signatures,shell`, and `--no-default-features --features compile,verify`. Default
  features build clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` clean.
- **Both new guards were mutation-tested** and each fails on the assertion it was written to make.

**Not run**: the full workspace suite and Miri. CI is the gate for the merge.
