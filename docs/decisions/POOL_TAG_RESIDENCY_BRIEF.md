# Brief — the shipping self-hosted compiler discards a field its own stage computes

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: in progress, opened 2026-08-20 (session 50).
**Scope**: `src/selfhost/mod.rs` host driver only. **No `.kel` stage change. No opcode. No
`BYTECODE_VERSION` change.**

## The finding, stated as a fact rather than a hypothesis

`src/selfhost/kel/codegen.kel` emits a constant pool as three streams: a count, then that many
raw values, then that many raw **tags**. It interns three tags, in three separate functions with
tag-aware deduplication:

| tag | meaning | interning function | value carried |
|---|---|---|---|
| 0 | `Int` | `intern_int` | the integer |
| 1 | `StaticStr` | `intern_str` | the **lexer intern id**, which the host must resolve to bytes |
| 2 | `Bool` | `intern_bool` | 0 or 1 |

`src/selfhost/mod.rs` reads the tag stream and **throws it away**:

```
for _ in 0..count { let _tag = next_word(&mut vm, &mut shared); }
```

Every pool entry is then built as `ConstValue::Int(v)` at all three call sites. So a `StaticStr`
becomes `Int(<intern id>)` and a `Bool` becomes `Int(0|1)`.

**This is not a stage defect.** The stage computes the right answer, dedupes it correctly, and
streams it. The host is the only thing that loses it.

## Why it survived

The comment sitting on the discard says it plainly, and is worth quoting because it is the
defect's own confession:

> *the stage sources are all-Int, so the tags are consumed and discarded here; the tagged protocol
> only matters for a program with a string literal.*

That is an accurate statement about the **corpus** and a false statement about the **contract**.
It is the seventh instance of the meta-defect this line keeps recording: *a property of the case
list mistaken for a property of the thing under test.* The byte-identity oracle compiles the twelve
stage sources; those sources contain no string literal and no struct equality, so the oracle cannot
see either tag. **Any construct the corpus does not contain is unverified by construction.**

## Both compilers read the same stage source, so the divergence is entirely host-side

`tests/common/mod.rs::stage_path` rewrites any `compiler/kel/<x>.kel` request to
`src/selfhost/kel/<x>.kel`, and `compiler/kel/` in fact holds only `prelude.kel`. So
`tests/selfhost_codegen.rs`'s copy and the library drive **the identical `codegen.kel`**. The copy
returns `Vec<(i64, i64)>` from `run_codegen` and maps all three tags; the library returns
`Vec<i64>`. That single type difference is the whole defect.

This matters for attribution: it rules out "the stages differ" and leaves exactly one cause.

## Proportionality, which must be stated every time

`self_hosted_compile` — the `--compiler self-hosted` CLI backend — cross-checks the constant pool
against the reference and **refuses on divergence**. A user compiling a string literal through the
CLI therefore gets a loud error today, not a wrong module. **The exposure is to direct callers of
`keleusma::selfhost::self_host_compile`, `_fused`, and `_scratch`.** Omitting this sentence
overstates the defect badly, and an earlier revision of the handoff did exactly that.

The fix converts a loud refusal into a correct compile. That is an improvement in reach, not a
change in safety.

## What "done" needs, and the part that is not the obvious part

Threading the tag through is mechanical at two of the three call sites. The third is not:

- `self_host_compile` (~1382) — `names` is in scope. Direct.
- `self_host_compile_scratch` (~3600) — `names` is in scope. Direct.
- **`self_host_compile_fused` (~1572) — `names` is NOT in scope.** The pool is consumed inside a
  `flush` closure taking `(group, name, module)`, while the name table arrives per-callback from
  `parse_functions_impl(src, false, &mut |names, f| ...)`. The table must reach the flush without
  being rebuilt. **Do not call `parse_functions_fused` a second time to get one** — that discards
  the streaming property the fused path exists for, and it is the sort of "fix" that passes every
  test while undoing the reason the code is shaped as it is.

## The wrong turns, named in advance

1. **DO NOT change both sides of the differential comparison in one increment.** This is the exact
   mechanism by which a `bool`/`Bool` regression shipped in `d1148e76` last session: the oracle was
   adjusted alongside the thing it judged, so it agreed with itself and stayed green. Here that
   means: **the library changes; `tests/selfhost_codegen.rs`'s copy does not.** The copy already
   agrees with the reference and is the control. If a step seems to require editing the copy, stop
   — that is the signal, not an obstacle.

2. **DO NOT delete `the_two_self_hosted_compilers_disagree_on_a_string_literal`.** Invert it. The
   convention on this line is that a pin recording an asymmetry becomes a pin recording the
   agreement; `block_form_statements` did precisely this when a trailing semicolon began to parse.
   A deleted test loses the fact that the divergence ever existed.

3. **DO NOT claim tag 2 coverage without a witness that actually fires.** `intern_bool` is called
   only from `push_struct_eq`, so the witness is a **struct equality comparison**, not a boolean
   literal — boolean literals lower to `PushImmediate` and never reach the pool. Before asserting
   anything about `Bool`, construct the source and confirm the reference bakes a
   `ConstValue::Bool`. **If no witness can be built, say so and pin only tag 1.** A guard that
   cannot fire is worse than none, and this line has written that sentence after building one.

4. **DO NOT assume `unescape_string` is reusable.** It exists only in `tests/selfhost_codegen.rs`.
   Escape handling is part of the contract — the reference bakes `\n`, `\t`, `\"`, `\\` — so the
   library needs its own, and a string literal containing an escape must be in the corpus or the
   escape path is the next thing unverified by construction.

5. **DO NOT touch the three resume channels on this branch.** `DESIGN_JOURNAL.md`,
   `REVERSE_PROMPT.md` and `TASKLOG.md` are prepended to by every increment and PR #211 already
   carries pending edits to all three. Two branches editing them conflict by construction. Findings
   go in this directory instead.

6. **DO NOT let the support table's verdicts drift silently.** `literal/string` is currently `SOk`
   because the table measures the test file's copy. Fixing the library does not change what the
   table measures. Whether any verdict moves must be **measured, not predicted.**

## What this does and does not do for the operator's `ParsedFn` decision

It closes exactly one of the four evidence bullets — "has measurably diverged from the shipping
compiler". **The decision still stands on the other three**: the duplicate blocks stage two of the
token-residency work, it forced the boolean-literal shared slots to be seeded twice, and it is
still the compiler the construct-support table actually measures. Reporting this as though it
resolved the decision would be a misreport.

## The transferable question, which outlives this fix

The class is: **the host discards a field the stage computes.** The tag stream was read and dropped
in a loop whose body was `let _tag = ...`. A discard is invisible to every test whose corpus never
exercises the discarded value. Sweeping the driver for other reads-then-drops is the general form
and is in scope here, bounded to `src/selfhost/mod.rs`.

---

# Findings, measured 2026-08-20 (session 50)

Everything below is measurement against `f091a668`, not prediction. The instrument was the
95-case construct-support table from `tests/selfhost_codegen.rs`, extracted mechanically and run
through `keleusma::selfhost::self_host_compile`, classified three ways against the reference.

## The census, before and after

| | baseline `f091a668` | after this work |
|---|---|---|
| byte-identical | 43 | **76** |
| differs | 21 | 11 |
| faults | 30 | 7 |
| reference rejects it | 1 | 1 |

**No case got worse.** Every construct that differs afterwards also differed in the baseline. The
baseline was taken by stashing the source change and re-running the same probe, because
attributing an improvement to oneself without a before-measurement is how three claims to the
other line last session turned out to be plausible rather than checked.

## What the tag discard actually cost — more than the string literal

The pool tag governs three `ConstValue` kinds, and dropping it wrongly rebuilt all three as
`Int`. The string literal was the only one anybody had noticed, but `Bool` is reached by **tuple,
array and enum equality**, none of which needs a `struct`:

- `intern_bool` is documented as serving `push_struct_eq`, which made the witness look expensive
  to build. It is not: `fn f(a: (Word,Word), b: (Word,Word)) -> bool { a == b }` reaches it.
- An all-unit enum equality bakes **all three tags in one pool** — names as `StaticStr`,
  discriminants as `Int`, results as `Bool`.

**The construct that reaches a branch is not always the construct the branch is named after.**
This brief's own wrong-turn 3 predicted a struct witness and was wrong in the cheap direction.

## THE LARGER FINDING: the driver dropped every struct, trait and impl DECLARATION

Looking for other discards found one immediately. `parse.kel` emits STRUCTSTART 18, TRAITSTART 19
and IMPLSTART 20 followed by the declaration's own parameter records. The shipping driver had no
state for them, so those records reached the function dispatch with nothing open and it panicked
by name.

**`tests/selfhost_codegen.rs`'s copy carried the skip all along**, in two places. So:

- 29 of the 95 boundary cases declare a struct. **The shipping compiler faulted on all 29.**
- **27 of those 29 are recorded `SOk`.**

That is a far stronger statement of the duplicate's cost than the string literal was. The boundary
did not merely drift on one construct; it reported `Ok` for twenty-seven constructs the shipping
compiler could not compile at all.

Mirroring the sibling driver's skip repaired 22 of the 29 to byte-identical. The remaining 7 fault
deeper, in scalar-kind decoding, which is a distinct and unfixed gap.

## THE RESIDUE, pinned rather than repaired

**Six constructs the boundary calls `Ok` that the shipping compiler miscompiles**, all pre-existing
and unrelated to the pool: the eager `and`/`or` operators and four precedence combinations. The
short-circuit `andalso` form is the control and agrees, so the divergence is attributable to the
eager operators specifically.

Not repaired here deliberately. The fix is in operator lowering, and making it in the same change
would leave the before/after census above unattributable — which is precisely how a `bool`/`Bool`
regression shipped last session, by changing both sides of a differential comparison at once.

## The class sweep (completion condition item 8)

Searched `src/selfhost/mod.rs` for reads bound to a discard, underscore-prefixed bindings,
ignored destructured fields, and unused parameters. Four sites beyond the pool tag:

| site | verdict |
|---|---|
| `_data_records` / `_enum_records` in `self_host_compile` | **By design.** That entry point splices chunk ops onto the reference scaffold; the layout is deliberately the reference's. `self_host_compile_scratch` does consume them. |
| `_valid` from `run_analyze_kel` in `assemble_resource_bounds` | **Sound.** `valid` reports a Stream chunk's transitive budget against the arena capacity, and that call passes `i64::MAX`, so the flag is trivially true. Discarding a constant is not discarding information. |
| `let _ = decode_op(tag)` | **Not a discard.** It is a test asserting decode does not panic; the value is genuinely irrelevant. |
| `let _ = names;` in `window_emit` | **FLAGGED, NOT RESOLVED.** A `names: usize` parameter is accepted and ignored. Either the caller's count should constrain the emit or the parameter should not exist. Out of scope here; recorded so the next reader does not have to find it again. |

So the pool tag was the only live instance, and the sweep is recorded including the negative
results, because "I looked and found nothing" is only useful when it says what was looked at.

## What this does for the operator's `ParsedFn` decision

**One of the four evidence bullets is closed and replaced by a much stronger one.** The string
literal no longer diverges. In its place: the boundary reports `Ok` for 27 struct-bearing
constructs the shipping compiler faulted on, and still reports `Ok` for 6 boolean constructs it
miscompiles. The decision is not resolved — it is better evidenced than before.

---

# THE BOUNDARY THIS MOVED, AND HOW TO REVERT JUST THAT HALF

**This change accepts programs the tree previously refused**, which is a boundary move and is
flagged here rather than left for the operator to discover.

`tests/selfhost_parse.rs` carried a test asserting that a `struct` declaration is REFUSED with a
named message. It now compiles, so that test is inverted rather than deleted, and it records all
three states the behaviour has had: a bare `unwrap()` panic, a named refusal, and the skip.

## Why this is not the deferred work, stated so the operator can disagree cheaply

The ruling of 2026-08-19 was **"Top-level struct support. Defer."** This change does not derive a
struct LAYOUT from the pipeline. A struct-using program compiles through `self_host_compile`
because the layout comes from the reference scaffold that entry point splices onto, and its chunk
ops now lower without faulting. Layout derivation remains deferred and unattempted.

**If the operator reads the ruling more broadly than that, the skip should come out.** The two
halves of this change are independent:

| half | where | how to revert |
|---|---|---|
| the constant-pool tag | `run_codegen`'s return type, `pool_to_constants`, three call sites | not recommended; it is a plain defect fix with no boundary move |
| **the declaration skip** | **three hunks in `parse_functions_impl`**: the `in_skip_decl` binding beside the other five `in_*` flags, the `else if in_skip_decl` arm in the state chain, and the `18..=20` match arm | drop those three hunks and restore `tests/selfhost_parse.rs`'s refusal assertion |

Reverting the skip costs the 22 boundary cases it repaired and restores the named refusal, which
is a coherent position; it does not affect the pool fix or any test in
`tests/selfhost_pool_tags.rs` other than `a_struct_declaration_compiles_rather_than_faulting`.

## A consequence worth knowing before deciding

With 18..=20 skipped, **no construct tried reaches `open_decl`'s named panic** — plain `struct`,
`trait`, `impl` and a const-generic `struct` all parse. Recorded as **not found**, not as
unreachable, matching the distinction drawn for `Op::IsStruct`. The message is retained because a
future record code arriving with nothing open is what it exists for.

---

# THE THIRD DEFECT, FOUND BY THE RESIDUE THIS BRIEF LEFT BEHIND (2026-08-21)

The census left six constructs the boundary calls `Ok` that the shipping compiler miscompiled.
They were deliberately not repaired in the same change, so the earlier census stayed attributable.
Repaired now, separately, and the diagnosis is the same shape as the other two.

## `a and b` compiled to `a`

Not a subtle divergence. The eager `and`/`or` operator and its **right operand** were dropped:

```text
  reference:  GetLocal(0), SetLocal(2), GetLocal(1), GetLocal(2), If, Else, PopN(1), Const(0), EndIf, Return
  shipping:   GetLocal(0), Return
```

So `true and false` returned `true`. `andalso`, `orelse`, `xor` and `not` were all correct
throughout, which is why nothing else in the suite noticed.

## The cause: the driver seeded neither id, and its comment said it did

`parse.kel` recognises the eager operators only when the host supplies their interned ids, guarded
`and_id > 0` so an unseeded host keeps the old behaviour. **The shipping driver seeded neither, at
either of its two token feeds. `tests/selfhost_codegen.rs` seeded both, at both of its own.**

The comment above the boolean-literal seeding in the shipping driver read *"seeded like the eager
`and`/`or` ids and for the same reason"* — a true statement about the sibling file, copied across
with the code it described. **A comment can be a false claim about its own file, and this one had
been for as long as it existed.**

## Both repairs are needed, and neither completes the construct alone

Seeding the ids made all six agree on **ops** and still differ in the pool — `Int(0)` where the
reference bakes `Bool(false)` — because the tag was still being discarded. The two fixes compose
exactly, which is why the pin covering them is one test rather than two.

## The census after all three repairs

| | baseline | + pool tag & declaration skip | + eager `and`/`or` |
|---|---|---|---|
| byte-identical | 43 | 76 | **82** |
| differs | 21 | 11 | **5** |
| faults | 30 | 7 | 7 |

**Every remaining difference is a case the boundary already labels `Diverges`.** No construct the
boundary calls `Ok` differs any more. The only `Ok` cases the shipping compiler still cannot handle
are the six that FAULT in scalar-kind decoding on a tuple whose element is a struct — a distinct,
unfixed gap, pinned in the failing direction.

## THE ONE-LINE VERSION, WHICH IS THE THING WORTH REMEMBERING

**Three separate silent miscompiles, one cause.** The shipping driver and the copy of it in the
test file are two implementations of the same thing, and **the construct-support boundary exercises
only the copy.** Every divergence found this session was a slot, a tag or a record the copy handled
and the shipping driver did not. That is the evidence for the accessor decision, and it is now
three findings deep rather than one.

---

# THE FOURTH DEFECT, AND THE GUARD THAT MAKES THE CLASS SELF-DETECTING (2026-08-21)

## `bad scalar kind tag 131080`, and the number is the whole diagnosis

Op tag 53 has **two** forms. The flat form packs `offset + kind_tag*65536`; the flat-nested form,
a nested composite tuple element extracted and re-wrapped, packs
`offset + size*65536 + variant*2^32`. They are disambiguated by operand magnitude.

The shipping driver decoded only the flat one — it had no `TupleField::FlatNested` arm at all,
though the type it constructs was already used elsewhere in the same file. So a struct-typed tuple
element's packed word was read as a scalar-kind tag. For `size = 8, variant = Struct(2)` the
operand is 8,590,458,880 and `operand / 65536` is **131,080**, which is the number in the fault.

`tests/selfhost_codegen.rs` has carried both arms all along, behind the same magnitude guard.

**Diagnosed entirely by reading**, while a gate held the build — the arithmetic was checked against
the fault message before any edit, rather than the fix being tried to see what happened.

## The final census

| | baseline | + pool tag & declaration skip | + eager `and`/`or` | + flat-nested tuple |
|---|---|---|---|---|
| byte-identical | 43 | 76 | 82 | **88** |
| differs | 21 | 11 | 5 | **5** |
| faults | 30 | 7 | 7 | **1** |
| reference rejects | 1 | 1 | 1 | 1 |

**The shipping compiler now reaches the same verdict as the boundary on all 95 cases.** Every
remaining non-identical case is one the table already labels `Diverges`, `Refuses` or
`RefRejects`. No construct recorded `Ok` differs or faults any more.

## FOUR DEFECTS, ONE CAUSE — and this is the finding, not the four fixes

| defect | symptom |
|---|---|
| the constant-pool tag was discarded | a string constant became the integer of its intern id |
| struct/trait/impl declarations had no skip state | the driver faulted on 29 cases |
| the eager `and`/`or` ids were never seeded | `a and b` compiled to `a` |
| op tag 53 had no flat-nested arm | a struct-typed tuple element faulted in kind decoding |

Every one was a slot, a tag, a record or an arm that `tests/selfhost_codegen.rs`'s copy of the
driver handled and the shipping driver did not. **The construct-support boundary exercises only
the copy**, so the project's own record of what self-hosting supports was describing a compiler
that is not the one that ships.

## The guard, which is the durable part

`the_shipping_compiler_matches_the_boundary_it_is_recorded_against` runs the SAME hoisted case
table through `keleusma::selfhost::self_host_compile` and asserts **per-case verdict agreement**,
not a count. A count is satisfiable by one construct regressing while another is repaired; per-case
agreement is not, and it names the construct when it breaks.

The table is hoisted into a function rather than copied, because a second copy of it would have
been the nine-copies defect a third time.

**This does not make the duplicate safe. It makes the drift visible.** The copy is still a second
implementation of the driver; the accessor decision that would let it be deleted is still open, and
is now four findings better evidenced than when it was raised.

---

# CLOSING THE CLASS BY STRUCTURE RATHER THAN BY CORPUS (2026-08-21)

The four repairs were each found by a corpus case. **A corpus is finite**, so the remaining
question is what the two drivers still differ on that no case exercises. Answered by deriving the
sets from the source rather than from the part of the system I happened to be thinking about,
which is this line's own recorded lesson.

## Three surfaces, all now closed

| surface | shipping driver | test-file copy | verdict |
|---|---|---|---|
| op-word decode arms | 63 tags, one guarded (53) | 63 tags, one guarded (53) | identical |
| shared slots seeded | 14 names, each on both feeds | 13 names | library is a superset |
| declaration record codes | 12 arms | 12 arms | identical |

The copy holds a **second** record dispatch, in `parse_function_records`, whose arm set
legitimately differs — it handles codes 2 and 3 rather than 6 and 7. Matching against that one
reports a divergence that is not one, so the extraction identifies the dispatch by an arm it must
contain rather than by position.

## `tests/selfhost_driver_parity.rs`, and its honest limits

The guard compares the three surfaces directly, so it does not depend on corpus coverage. **It
would have caught three of the four defects, not all four.** The pool-tag discard is invisible to
it: both files read the tag stream and differed in what they did with it afterwards, which is
semantics inside an arm rather than the presence of an arm. Stated in the test itself, in a table,
because a guard that overstates its reach is worse than a narrower one that does not.

It is also a **textual guard over source text**, the weakest shape of test, and this line has
argued against exactly that elsewhere. Two things earn it its place: it complements a corpus guard
whose reach is bounded by 95 cases, and every set it derives is asserted non-vacuous, so a broken
extraction fails rather than passes empty.

## THE INSTRUMENT WAS WRONG TWICE, AND BOTH TIMES IT SAID SO

Recorded because it is the cheap version of the failure the guard exists to prevent.

1. **A false positive on the first run.** The copy inlines its scalar-kind decoding where the
   library factors it into helpers, so a depth-blind scan collected the copy's nested
   `0 => ScalarKind::Unit` arms and reported a divergence that did not exist. Depth is
   load-bearing.
2. **A silent no-op after fixing that.** Anchoring on the function header rather than the `match`
   header handed the depth filter a body one level too shallow, and it found nothing. **The
   non-vacuity assertion caught it** — without it the test would have passed while comparing two
   empty sets.

## AND THE GUARD ITSELF FAILED ITS FIRST MUTATION TEST

Three mutations were run against the repaired tree, each removing one thing the guard claims to
detect. **The seeding mutation did not fire.**

The cause was real, not incidental. The library has two token feeds and seeds each slot once per
feed; the test compared **sets of names**, so removing one of the two seedings left the name
present via the other. **A slot seeded on one path and not the other is exactly this defect
class**, and the guard was blind to it.

Now counted rather than merely present, with the threshold calibrated against `BR_P_WORD_ID`'s own
count instead of a literal, so adding a third feed cannot silently weaken it. All three mutations
fire, and the message names the slot and both counts.

**A guard that has not been made to fail is a guess.** This one was a guess for about ten minutes.

---

# A PROCESS FINDING FROM SHIPPING THESE FOUR (2026-08-21)

**A pull request based on a feature branch gets no CI at all, and it looks like a slow queue.**

`ci.yml` filters `pull_request` on the **base** branch, `main` or `v*`. Two of this session's pull
requests were stacked on their predecessors, so their base matched neither pattern and **no
workflow ran** — not a failure, not a queue, an absence. `gh pr checks` said "no checks reported",
which reads the same as "just started".

Re-targeting the base did not provoke a run either: a base change emits `edited`, which is not one
of the default `pull_request` types. Closing and reopening does, because `reopened` is.

This is the same shape as the entry the handoff already carries about a CANCELLED run looking green
in a summary, and as the rule about never classifying a state as failure by exclusion: **the
absence of a signal is not the presence of a passing one**, and both are one line in a status
listing. Base future branches on the version branch and describe the stack in the body.

## The one sweep item left open is now closed, and it is NOT a defect

The class sweep flagged `window_emit`'s `names: usize`, accepted and read into a discard, as
"flagged, not resolved". Resolved 2026-08-21: **`wire.kel` derives the name count itself.**
`name_count()` is a pure function of the blob it was given and is returned by command 18, so a
host-supplied count would be a second answer to a question the stage already answers — the drift
this crate has now paid for four times over.

The parameter is annotated in place rather than removed, so the next reader does not repeat the
investigation, and the note says plainly that removing it would be a cleanup rather than a repair.

**A discarded value is not automatically a defect.** Three of the four this session were; this one
is not, and reporting it as a fifth would have been the more comfortable and less accurate answer.
