# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-09-10 (session 65, sixteenth increment) — a missing width floor found under the reach that was unproven, the reach proven for one build with a valid control, and a derived census of which guards were shown able to fail

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
