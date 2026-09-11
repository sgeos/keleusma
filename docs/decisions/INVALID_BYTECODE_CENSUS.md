# The `InvalidBytecode` class, enumerated

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: First pass, measured against the tree. Written 2026-09-04.

## Why this census exists

`VmError::InvalidBytecode` means *this artefact should never have been produced*. It is **the class
`verify()` exists to exclude**. A module that verifies, loads, and then raises it is a hole in the
load-time guarantee rather than a bad program.

On 2026-09-04 exactly such a hole was found **by accident**, while removing an unrelated fallback:
checked indexing over a `Multiword` compiled, verified, took a memory bound, loaded, and trapped. It
was not predicted, not covered by any test, and reachable that day with nothing holding it shut.

**One hole found by accident implies nothing about how many remain.** The honest response to an
accidental find in a class nobody has enumerated is to enumerate the class.

## The question asked at each site

Not *can this fire* -- a corrupt or hand-built module can reach almost any of them, and the runtime
is right to keep every check. The question is narrower:

> **Can a module that a supported producer emitted, and that `verify()` accepted, reach this site?**

## The population, and the instrument's reach

Derived from source, not chosen by hand:

```sh
grep -rn "VmError::InvalidBytecode" src/ --include=*.rs
```

**51 matches, of which 47 are construction sites in production code.** The four excluded are named
rather than silently dropped: one doc comment referring to the variant, one match arm listing it
among unrecoverable errors, and two inside unit tests. They live in `src/vm.rs` (45) and
`src/marshall.rs` (2).

**The figure was 50 and 46 until 2026-09-10.** The opaque-width repair of 2026-09-08 -- part of
the same line of work that wrote this census -- added `"flat opaque field read out of bounds"` to
`src/vm.rs`, taking the population to 47.

**A source-derived guard already existed and did not fire, by design.**
`the_invalid_bytecode_census_still_describes_the_tree` compares a scan of the runtime sources
against the figure stated here with a tolerance of plus or minus four, chosen deliberately because
its scan cannot distinguish a unit-test occurrence by line shape and its own message says so:
*"the comparison carries a small tolerance rather than pretending to exactness the scan cannot
deliver."* A drift of one sits well inside that. **The tolerance that makes the guard robust also
makes it blind to exactly the movement that happened**, and that is a property of the instrument
rather than a mistake in it.

`tests/invalid_bytecode_indirect_sites.rs` now carries an EXACT count beside it. It can be exact
because it reproduces this document's exclusions mechanically -- comments by a strip, the match arm
by its `(_)` pattern position, and the unit tests by truncating at the test module -- which is the
distinction the tolerant guard declines to make. The two are kept separately on purpose: a tolerant
alarm that survives refactoring, and an exact one that notices a single site.

**A first draft of this table said 48, and mis-sized three groups.** The grep counts TEXT, and a doc
comment and a match arm read exactly like a construction site to it. Re-derived by classifying every
match rather than by adjusting the total, which is the same discipline this line applies to every
other figure it publishes.

**What this instrument would miss.** It matches the error CONSTRUCTED at the site. A site that
returns a pre-built error value, propagates one from a helper with `?`, or maps another error kind
into this one would not appear. One such conversion DOES exist and is included because the grep
happened to see it, which is evidence the class has members this scan cannot enumerate. The
population is a lower bound.

## Verdicts by group

Sites are grouped where they share one defect class. Probing one member of a class and saying so is
honest; probing every member individually is not a better use of the same effort.

| # | group | sites | verdict |
|---|---|---|---|
| A | flat scalar decode failure, converted from the codec error | 1 | **REACHABLE, witnessed** — see below |
| B | float opcode without the `floats` feature | 2 | **REACHABLE -- see below** |
| C | `Fixed` fraction bits exceeding the word width | 5 | **defended**, by two checks that compose |
| D | composite operand form mismatch | 7 | **defended**, by boundary canonicalization |
| E | structural indices out of range | 9 | **defended at load** (8 of 9 probed) |
| F | shared and private data-segment layout | 7 | **host-contract, confirmed** (2 of 7 probed) — but see below |
| G | arena staleness after reset | 3 | **no witness found**, all three now addressed — see below and the seventh addendum |
| H | the three "should never have been emitted" | 3 | **closed 2026-09-04** |
| I | operand-range and constant-kind checks | 6 | **mixed** — see below (5 of 6 probed) |
| J | unregistered or invalid native index | 3 | **mixed** — the index is admitted at load (1 of 3 probed) |
| K | the retained runtime bounds guard on a flat opaque field | 1 | **not examined** — added 2026-09-08, see below |

The group sizes sum to 47, which is the population above; a table whose parts do not add to its
stated whole has been the tell for a miscount here before.

**Group K is a row rather than a reclassification, deliberately.** The site it holds is a bounds
guard on a flat opaque field read, and its sibling -- the identical guard on a flat `Text` field --
was already in the population when this census was written. They plausibly belong to the same
group, but **this document does not publish a per-line classification, and guessing which group
the sibling occupies would be worse than leaving the new site in a row of its own.** If a future
pass derives the classification and the two belong together, K folds in and the arithmetic moves
with it.

**What "examined" counts, stated because it was not.** The parenthetical in a verdict is the signal:

- A group whose verdict carries **no** probe count — `defended`, `REACHABLE`, `closed` — extends its
  argument to every member, and all of them count.
- A group whose verdict says **"(k of n probed)"** deliberately WITHHOLDS that extension: only the
  `k` count, and the remaining `n - k` are open. Groups E, F, G, I and J are in this state, which is
  why the closing section names F and J as remaining despite their verdict reading "confirmed".
- `not examined` counts none.

The per-group figures are the authority; any total elsewhere is derived from them and is wrong if it
disagrees.

**A first attempt at this paragraph got it backwards on the same day it was written.** It said a site
counts when "a stated argument covers it as a member of a class one probe reached", which would make
all seven of F examined — contradicting both the table it declared authoritative and the closing
section that names F as remaining. **A convention invented to settle a count must be checked against
the count it settles**, and this one was not until it was re-read.

**The F row above read "1 of 7 probed" until 2026-09-08 and disagreed with the prose beneath it**,
which said "two of F's seven" and was right: the hot-swap site was probed after that row was
written and the row was not updated. The totals were re-derived from the prose and are unaffected.
**A count in a table and the same count in a sentence are two places to go stale**, and this
document has now been the tell for its own miscount twice.

**Thirty-seven of forty-seven sites carry an examined verdict**, group by group: A's one, both
of B, all five of C, all seven of D, eight of E's nine, two of F's seven, all three of G, all three
of H, five of I's six, and one of J's three.

**The remaining ten** are one in E, five in F, one in I, two in J, and the one in K.

**Group G moved from one of three to all three on 2026-09-10**, when its other two sites were
probed. See the seventh addendum. By this document's own convention a verdict carrying no probe
count extends to every member, so removing G's parenthetical moved the examined total by two, and
the guard in `tests/claimed_counts.rs` refused the document until this sentence moved with it.

**Two corrections are folded into that tally, and both are the same defect.** Earlier revisions of
this line said fifteen remaining and then eleven, and **both omitted group G entirely** — the
arena-staleness sites, never examined, silently absent from a list that purported to name what was
left. The figure was also re-derived by summing the per-group column rather than by adjusting the
previous number, which is how the omission surfaced at all.

**One caveat applies to every count here.** Probes map to sites by MESSAGE CLASS, not one-to-one: a
mutation tripping `GetData` exercises the site that message comes from, and sibling sites emitting
the same message are credited with it. The groups were formed the same way. Read the figure as
"message classes examined", not as lines of source visited — a weaker claim than the bare number
suggests, and it was overdue. A census whose entries are unexamined opinions is worse than a
short one that says which sites were looked at.

## Group A is reachable, and this session's own defect is what fired it

The site is `From<ScalarError> for VmError`, which **every `?` over the flat scalar codec passes
through**. The census flagged it as the one conversion its grep happened to catch, and said so as
evidence that the class has members the scan cannot enumerate. It sat unexamined because no probe had
been aimed at it.

**No probe was needed in the end.** It fired twice, unprompted, in the pre-repair `narrow-word-16`
run:

> `call: InvalidBytecode("flat scalar codec: OutOfBounds")`

in `tuple_with_opaque_element_flattens_and_resolves` and
`tuple_with_opaque_and_trailing_scalars_offsets`. Both are ordinary programs on a real runtime. The
cause was the opaque-field width disagreement repaired earlier in this session, recorded in
[`NARROW_WIDTH_FAILURE_CLASSIFICATION.md`](./NARROW_WIDTH_FAILURE_CLASSIFICATION.md).

### The route is closed; the class is not

After the repair the message appears **zero** times in `narrow-word-16`, in `narrow-address-16`, and
in the default suite — three finished runs, and a statement about those three rather than about every
configuration.

**That closes one route, and the class stays live.** The site's reach is the union of every `?` over
the scalar codec, which this document's instrument cannot enumerate; it says so in its own opening.
The distinction is the one this tree keeps re-learning: an opcode was declared producerless and
another line found four producers within the hour, after which the rule became *no producer FOUND*,
never *unreachable*.

### The error kind misattributes the fault, exactly as in group F

The message says the bytecode is malformed. **It was not.** The artefact was correct and the runtime
disagreed with its own layout about how wide a field is. A host reading that message goes and
inspects their module, which is the wrong thing.

This is the same defect group F records for the hot-swap site, reached from the opposite direction —
there a host argument error was reported as `InvalidBytecode`, here a runtime bug was. **Not
repaired**: changing which variant a public API returns is a breaking change and the operator's call,
alongside the API-shaped decisions already queued.

## Group B is reachable, and it is a real deployment shape

**A module using floats verifies, loads, and traps on a runtime built without the `floats`
feature.** Measured with `--no-default-features --features verify`:

| step | result |
|---|---|
| `Module::from_bytes` | accepted |
| `verify()` | **accepted** |
| `Vm::new` | **loaded** |
| call | **`InvalidBytecode`** |

**Two independent reasons nothing catches it earlier**, and closing either would suffice.

1. **`verify()` has no `floats` gating at all** -- not one conditional in `src/verify.rs` mentions
   the feature, so the structural pass has no notion that a float opcode is inadmissible.
2. **The header width check cannot reject it.** Load admits when `got <= max_supported`, and
   `RUNTIME_FLOAT_BITS_LOG2` is **not** gated on the feature, so a build without floats still
   advertises the full width.

**Nothing here is corrupt.** The fixture is the ordinary output of the reference compiler, and
omitting floats is the point of the feature -- an embedded target is exactly where it is used.
Producing bytecode on one build and running it on another is the normal deployment shape for a
language that ships precompiled modules.

**Proportionality.** The trap is loud: a clean error at call time, not a wrong answer, a crash, or
memory unsafety. What is wrong is the LAYER. Exposure is to a host that builds without `floats` and
runs a module produced by a build that had them.

Pinned by `tests/float_opcode_without_floats.rs`. **Not repaired.**

### The recommended repair, and the evidence for it

Refuse a float opcode in `verify()` when the feature is absent, moving the refusal from run time to
load time. **Prototyped and measured**: about ten lines in the opcode scan, after which the pin fails
at its `verify()` step exactly as its message anticipates, and the float-free control still
compiles, verifies, loads and runs. The prototype was then reverted.

It was left undone for a reason that should be weighed rather than assumed away: **continuous
integration builds no configuration in which this test compiles.** Stated precisely, because the
loose version of it was wrong: CI DOES run `--no-default-features`, but bare, without `compile` or
`verify` — and the pin requires `verify` with `floats` absent, so it is configured out there. Every
other job is additive to the default features and therefore includes floats. An imprecise reason
outlives the finding it is attached to, so it is corrected rather than left standing.

### That objection is now discharged, and the decision is narrower than it was

The missing verification was supplied rather than handed over. With the repair applied, the full
`--no-default-features --features compile,verify` suite was run and compared against the same suite
unrepaired:

| | unrepaired | with the repair |
|---|---|---|
| new failures introduced | -- | **zero** |
| `tests/float_opcode_without_floats.rs` | passes, the hole open | fails at its `verify()` step, the pin firing as designed |

**And the semantic worry is moot for locally-compiled code.** The repair refuses a module CONTAINING
a float opcode, not one that executes it, so a module with unreachable float code would be refused.
That sounded like a capability loss until it was measured: **the LEXER refuses a float literal
without the feature**, so no float program can be compiled on such a build at all. The only artefacts
affected are ones compiled elsewhere and imported -- exactly the case where refusing at load is
unambiguously right.

What remains is a single semantic judgement for the operator, not an engineering risk.

### Getting that evidence required repairing the configuration itself

**`--no-default-features --features compile,verify` did not compile**, and two further tests failed
once it did. Five defects, all one class: float-dependent code with no `floats` gate.

| file | how it failed |
|---|---|
| `tests/selfhost_codegen.rs` | names `ScalarKind::Float`, a variant absent without the feature |
| `tests/selfhost_wire.rs` | its constant-kind match leaves the catch-all unreachable |
| `tests/multiword.rs` | one probe's source carries a float literal; refused at LEX |
| `tests/block_form_statements.rs` | the grammar's own example uses `Float`; refused at LEX |
| `tests/narrow_vm.rs` | a float-width helper is dead code there (warning only, left) |

**None of these was tolerated; every one was invisible.** The release gate's no-default step does not
add `compile,verify`, and continuous integration never omits floats, so **nothing anywhere built this
combination.** The configuration in which the hole lives was the configuration nothing exercised,
which is the whole reason the hole survived. It is now green at 105 binaries and 1863 tests.

Same family as the verify-without-floats build failure V0.2.2 repaired, which suggests the class
recurs and that a feature-combination sweep would be worth more than any single fix in it.

## Group C is defended, and by two checks that only work together

Worth recording because neither check alone is sufficient, and a future change to either reopens it.

- `verify()` rejects a `Fixed` fraction count at or beyond **the module's declared** word width.
- Load rejects a module whose declared word width **exceeds the runtime's** (`got <= max_supported`).

The runtime's own guard compares against the RUNTIME width. Without the second check, a module
declaring a 64-bit word with 32 fraction bits would pass verification and then trap on a 16-bit
runtime. With it, that module never loads there. **Removing or loosening the load-time width
comparison would reopen five sites at once.**

## Group H, for the record

`Op::Len` on a flat array, `Op::Len` on a flat tuple, and `Op::IsStruct` on a flat struct. All three
are addressed: the compiler has **no producer found** for `Op::Len`, and `Op::IsStruct`'s witness was
closed at two symmetry gaps. See [`OP_LEN_ROOT_REPAIR.md`](./OP_LEN_ROOT_REPAIR.md) and
`tests/opcode_reachability.rs`.

## What must not be concluded from this document

**No site here is claimed unreachable.** Group B is reachable; groups C and H are defended by
named checks; everything else is *not examined*, which is a statement about this census and not
about the code.

**Nothing should be deleted on the strength of it.** These are defences against corrupt artefacts,
and the wire format admits hand-built modules. This is a reachability record, not a deletion list.

## Group D is defended, and the defence is a third piece of code neither side reveals

The seven "operand form does not match" sites fire when the access form the compiler baked
disagrees with the representation the value has. The shape looked dangerous, and the code says so
from both ends: **the compiler bakes a FLAT access for a scalar-fielded struct**, while
`GenericValue::struct_with_widths` states plainly that a host-built composite is BOXED -- "no arena
here, so the no-arena path is boxed". The `GetField` dispatch has arms for flat-with-flat and
boxed-with-boxed and sends everything else to the refusal.

**The pairing does not occur, because a host-returned composite is canonicalized at the call
boundary** into an arena-resident flat body, after a companion pass restores a boxed enum's
discriminant and padding from the module's recorded layouts. The boxed body never reaches an access
site.

**This was verified by mutation, and the first mutation was aimed wrong.** Removing the
canonicalization on the ARGUMENT path changed nothing, because the return path is a different call
site. Removing it on the native-RESULT path produces exactly
`InvalidBytecode("GetField operand form does not match struct body")`. Had the census stopped at the
first mutation it would have recorded a mechanism that does not do the work attributed to it.

Seven shapes were driven through a native and none reached a refusal: a struct field, a struct
rebound then accessed, a tuple index, an index into an array of structs followed by a field, an enum
payload through a match, and two forms of the same struct case. **No witness found** -- not
unreachable.

The other route in is closed earlier. A composite-typed re-entrant `yield` reply is one of the two
shapes the typed pass defers on, and for a struct, tuple or array it is **refused at compile time**
by that pass rather than deferred to run time.

Both properties are pinned by `tests/native_composite_canonicalization.rs`, because the
canonicalization is load-bearing and invisible from either end: the compiler's baking and the
runtime's dispatch each look locally correct, and the code that reconciles them sits between them.
If it regresses, seven refusals open at once.

## Groups E and I: every index is checked at load, one operand RANGE is not

These two ask a **narrower question** than groups B and D, and the difference must not be lost. A
compiler does not emit an out-of-range data slot or a reserved immediate, so reaching these needs a
corrupted or hand-built artefact -- which the wire format admits, so the question is real. But both
outcomes are safe, because the runtime refuses either way. **This is defence in depth, not a hole in
the load-time guarantee**, and reporting it at group B's severity would discredit group B.

Measured by compiling a valid program, injecting one defect into the compiled artefact, and asking
what the load-time pass does. Each mutation's application is COUNTED, for a reason given below.

| defect injected | verdict |
|---|---|
| `GetData` slot far past the data layout | **rejected at load**, naming the slot and the layout size |
| `SetData` slot far past the data layout | **rejected at load** |
| `GetLocal` slot past the chunk's local count | **rejected at load** |
| `Const` index past the constant pool | **rejected at load** |
| `GetDataIndexed` / `SetDataIndexed` base past the layout | **rejected at load**, naming the slot RANGE |
| `Call` chunk index past the module's chunk count | **rejected at load** |
| `SetLocal` slot past the chunk's local count | **rejected at load** |
| `GetField` flat offset far past the body | **rejected at load**, by the typed operand-stack pass |
| `IsEnum` tag past the constant pool | **rejected at load** |
| `Reset` injected into a non-stream chunk | **rejected at load**, naming the block kind |
| **`PushImmediate` operand in the reserved range** | **ADMITTED** -- loads, and traps at the call |
| **`Trap` carrying an unrecognised kind code** | **ADMITTED** -- loads, and traps at the call |

**Eight indices rejected, two operand VALUES admitted.** The pass validates every index it meets,
precisely and with a good message, and does not validate an operand's value range.

**Two instances rather than one changes how this reads.** A single unchecked operand is an
oversight; two, against eight checked indices, is a boundary in what the pass was built to cover.
Which it is remains the operator's to say -- what is recorded is the observation, not the intent.

Pinned by `tests/immediate_operand_range.rs`, whose controls are the four rejected cases: without
them, "verify admits a bad operand" could be misread as the pass checking nothing.

**Not repaired.** A load-time check costs time on every load and this project rejects conservatively
on purpose; the observation is recorded and adopting it is a separate call.

### The probe's first revision produced a vacuous verdict, and that is worth recording

It mutated `PushImmediate` in a program compiled from `k + 1` -- **which contains no
`PushImmediate`**. Nothing was changed, the untouched module verified, and the probe reported
"admitted": a verdict about a mutation that never happened. It was caught only because the
follow-through ran the module and it returned the correct answer.

The probe now COUNTS the mutations it applies and reports a zero count as vacuous rather than as a
result. **Third instrument corrected in one session** -- after a file-attribution column that
reported warning locations under a passing verdict, and a mutation aimed at the wrong call site that
passed.

## Group F: the classification was an assertion, and testing it found something else

Groups F and J were called "host-contract surfaces" and set aside. **That was an assertion, not a
measurement**, and an exclusion made for a good reason is still an exclusion -- the same shape that
hid the narrow selectors from the feature sweep until they were swept.

One site did not obviously belong to the class. The message about a NEW module declaring a different
number of private slots than the host supplied fires on a **hot swap**, which is a shipped feature.
A host that swaps to a module with different data requirements has done nothing wrong.

**The classification survives.** `Module::data_layout` and `DataLayout::slots` are both public, so a
host can read the required slot count directly from the module it is about to install and supply
matching data. A correct host never reaches the site. It is genuinely a contract violation.

### But the error KIND is wrong by this codebase's own stated rule

`VmError::NotSuspended` carries this, verbatim, as its reason for existing:

> Distinguished from `VmError::InvalidBytecode` to keep API misuse separate from corrupt or
> malformed bytecode.

The hot-swap site reports **a host argument of the wrong length** as `InvalidBytecode`. The bytecode
is not malformed; the caller's argument is. So the project defines the distinction, builds a variant
to preserve it, and then breaks it here.

**Small, and worth exactly what it is.** A host sees an error naming their artefact when the fault is
in their call, which sends the reader to inspect the wrong thing. **Not repaired**: changing which
variant a public API returns is a breaking change and the operator's call, alongside the other
API-shaped decisions already queued for them.
## The final pass: four admissions, and the pattern is wider than operand values

Finishing the remaining sites found two more admissions, and the second is not an opcode operand at
all.

| defect injected | `verify()` | `Vm::new` | call |
|---|---|---|---|
| `entry_point` past the module's chunk count | **admits** | loads | **traps** `invalid chunk index` |
| `CallVerifiedNative` index past the native table | **admits** | loads | **traps** `invalid native index` |

Alongside them, newly rejected at load: a `GetEnumField` payload offset past the body (caught by the
typed operand-stack pass) and a shared-slot index past the layout.

**So the boundary is not simply "indices yes, operand values no".** It is closer to: the pass
validates **operands inside a chunk against tables inside the module**, and does not validate an
operand's value range, the **module-level entry point**, or the native index. The entry point is
plainly checkable -- the chunk count sits in the same structure. Whether the native index is
checkable at load is NOT established here, since natives are registered by the host after loading,
and this document does not claim it either way.

**Severity is unchanged from the rest of this class.** Every one needs a corrupted or hand-built
artefact; the compiler produces none of them, and the runtime refuses all of them. This is defence in
depth. **Group B remains the only entry where a module the compiler itself produced verifies, loads,
and traps**, and it is the only one that should prompt action.

### Why this pass happened at all

Three times in this session a remaining group was called low-value and set aside, and three times
testing it anyway found something: group D's undocumented boundary mechanism, groups E and I's two
admitted operands, group F's error-kind violation of a rule this codebase states in its own source.
**Three for three against my own judgement** was a better argument than the judgement, so the last
sites were probed rather than asserted away. Two more admissions is the fourth.

## Group G: no witness found, and the reason is structural rather than a check

Group G was **never examined and was missing from every list of what remained** until the tally was
re-derived by summing the per-group column. A group nobody counted is a group nobody checked, which
is reason enough to look.

It also had the best remaining chance of being a **group B shape** rather than a corrupt-artefact
one. The other outstanding sites need a hand-built module or a host API misuse; **holding a value
across a reset is something a program does.**

**A hypothesis from a filename was wrong, and it is recorded because it nearly went untested.**
`src/confine.rs` sounded like the mechanism that would prevent the escape. Read, it is a memory
planner asking whether a construction site's region can be reused -- nothing to do with refusing
escapes. Had the verdict been written from the filename, it would have credited a module that does
none of that work, exactly as an earlier mutation in this session credited the wrong call site.

**Measured with programs.** Four shapes were driven through a `loop main` across three resumes each:
a local array held across a yield, a `private data` composite read on later iterations, a struct in
a local, and a nested array. **None reached a staleness refusal**, and each ran to a `Reset` final
state, so the resets are shown to have happened rather than assumed.

**The reason is the shape of the language, not a check that catches it.** A transient composite
cannot be NAMED after the reset that ends its iteration, because the next iteration re-executes the
body and rebuilds it. The only storage crossing a reset is the persistent region, which is not
reset. So there is no expression that reads a pre-reset transient body.

**No witness found -- not unreachable.** One of group G's three sites was probed this way; the other
two concern host-supplied opaque handles going stale, which is a different question and untested
here.

## Where the next pass should start

Groups F and J, the host-contract surfaces, are what remain unexamined alongside group A and the
unprobed members of E and I. Groups F and J are host-contract surfaces and are lower value: a
host that supplies a mis-sized buffer or an unregistered native has broken a stated contract, which
is the same class as the native array-length finding rather than a hole in the guarantee.

## Addendum, 2026-09-10: a site witnessed from a supported producer that this census does not name

The census's question is *"can a module that a supported producer emitted, and that `verify()`
accepted, reach this site?"* On 2026-09-10 one did, and the message it raised appears nowhere in the
groups above:

```text
InvalidBytecode("NewComposite flat operand on non-flat values")   src/vm.rs
```

The producer was `compile_with_target` with a target declaring `addr_bits_log2 = 2`. That width was
accepted because `Target::validate_against_runtime` had no floor. The layout sizes an opaque by the
address width, four bits is zero bytes, and the value stopped being flat-eligible while the
compiler's baked access still expected a flat body. See
[`TARGET_WIDTH_FLOOR.md`](./TARGET_WIDTH_FLOOR.md).

**The route is closed** — the width is now refused at compile time — so this is a record of the
census's REACH rather than a live hole. Three things follow, and only the first is certain.

1. **Fact.** A compiler-produced, verified module reached that site, and no group in this census
   names that message. Whether the site is a member of an existing group, group D most plausibly,
   is not determined here; the census does not publish a per-line classification and guessing one
   would be worse than leaving it open.
2. **Inference.** The census enumerated by the error CONSTRUCTED at a site and reasoned about each
   group's reachability from the shapes a program can express. A degenerate TARGET DESCRIPTOR is a
   different axis: the program is ordinary and the module is malformed by the width it declares.
   Nothing in the population derivation reaches that axis.
3. **Not established.** Whether other sites are reachable along the same axis. The floor closes the
   two widths that had none, but the census's reasoning about what a supported producer can emit
   assumed a well-formed target throughout, and that assumption is now known to have been unstated.

**The next pass should ask each group's question again with the target descriptor as a variable**,
not only the program.

## Addendum, 2026-09-10 (second): the target-descriptor axis, swept

The addendum above says the next pass should ask each group's question with the target descriptor as
a variable rather than only the program. That sweep now exists as
`tests/target_descriptor_axis.rs`.

### What it varies, and how the bounds are obtained

Every descriptor the compiler ACCEPTS: the word and address widths from the narrowest implemented
one up to the runtime's maximum, each paired with no floats and with every float format the runtime
implements. The bounds come from `RUNTIME_*_BITS_LOG2` and from the `Word` and `Address` trait
impls, never from literals, so the sweep follows a build rather than describing one.

Against a corpus of six shapes chosen for the constructs whose layout is width-derived: scalar
arithmetic, a word field after an opaque, a nested composite child, an array striding over
opaque-bearing elements, an enum payload after a discriminant, and a tuple with fields after an
opaque. **Every expected value fits in an eight-bit word**, so a legitimate overflow at the narrow
end cannot be mistaken for an artefact defect.

### Result at the default build, 2026-09-10

**48 descriptors by 6 shapes, 288 cells. Every cell RAN and returned the expected value.** Nothing
was refused at compile time or at load, nothing faulted, and no cell returned a wrong answer.

The sweep is also green under `narrow-word-8`, `narrow-word-16`, `narrow-address-8` and
`narrow-address-16`, where the descriptor space shrinks with the runtime's maxima.

### The sweep is shown able to report, which is the part that makes the result mean anything

Removing the address floor and admitting sub-floor widths produces **twelve findings**, each named
by descriptor and shape:

```text
w3/a2/nofloat / array stride over opaque-bearing elements: NewComposite flat operand on non-flat values
```

That is the defect of the first addendum, reproduced through this harness. The same run classified
ninety compile-time refusals correctly, so the refusal path is exercised too.

**Only one shape of the six reaches it.** The array stride is the shape that multiplies an element
size, so a zero-byte scalar shows up there and is absorbed elsewhere. A corpus of five ordinary
programs could easily have missed the defect entirely, which is an argument about how thin the
evidence from any small corpus is, not a claim that this one is sufficient.

**That characterisation was refined on the same day by widening the corpus, and it was incomplete.**
See the third addendum below: striding is NOT the discriminating property.

### What the first draft of the sweep got wrong

It required at least one cell to be REFUSED, on the assumption that some admissible descriptor would
be rejected for these programs. **The sweep failed on its first run and said so.** Every descriptor
the compiler accepts compiles and loads every shape in this corpus. The check asserted a property
that had not been measured, inside a test written to measure properties, and it now constrains the
descriptor set's SPREAD instead, which is what non-vacuity actually requires.

### What this does NOT establish

- **The census's population is still a lower bound.** This adds one axis. It does not make the
  source-derived enumeration complete, and nothing here should be read as closing group A, F, G, I
  or J, whose verdicts carry probe counts for the reason those counts exist.
- The corpus is six shapes. A clean sweep over it is evidence about those shapes across the whole
  descriptor space, not about every construct the language admits.
- The sweep runs one runtime, the default `Vm` for the build. A module declaring narrower widths is
  admitted by the load check, which is the skew this exercises; a runtime narrower than the module
  is refused and is not part of this axis.


## Addendum, 2026-09-10 (third): the corpus widened, and what it sharpened

The addendum above says a clean sweep over six shapes is evidence about those shapes and not about
every construct. Seven shapes were added, each for a width-derived layout property the first six do
not stress rather than for variety:

| added shape | the property it stresses |
|---|---|
| a word field after an opaque and a `Fixed<4>` | a scalar sized by the WORD whose default fraction count is DERIVED from that width, so its semantics move with the descriptor |
| a word field after an opaque and a `Byte` | the only field whose offset contribution is descriptor-INVARIANT while its neighbours' are not |
| an array inside a struct after an opaque | stride COMPOSED with a field offset |
| a composite payload inside an enum | a body whose own offsets are computed behind a discriminant the descriptor sizes |
| a const-generic array length | a const parameter ERASED to a literal that then feeds a size |
| a `Multiword<2>` limb index | the only representation that is a COUNT of words rather than one |
| an array of arrays | stride NESTED, the outer index multiplying a size that is itself an array's |

### Result

**48 descriptors by 13 shapes, 624 cells, every one RAN and returned the expected value**, at the
default build and at all four narrow selectors including both eight-bit ones. The sweep names any
cell that does not run, so a shape refused everywhere could not be mistaken for coverage.

### What widening SHARPENED, which is the real result

The control was re-run against the larger corpus and produces **exactly the same twelve findings, on
exactly the same one shape.** Two of the seven additions also stride -- an array inside a struct, and
an array of arrays -- and **neither reaches the defect.**

So the earlier characterisation was incomplete. Striding is not the discriminating property. The
element must itself CONTAIN the address-sized scalar: the reachable shape is an array whose ELEMENT
is a composite bearing an opaque, and an array of words or of arrays strides just as much while
reaching nothing. **A widened corpus that finds no new defect can still correct a claim**, and here
it corrected one written the same day.

### What this does NOT establish

Thirteen shapes is more than six and is still not every construct. The negative result is evidence
about these shapes across the whole descriptor space. No group in the table above that carries a
probe count is closed by it, and the population derived from source remains a lower bound.


## Addendum, 2026-09-10 (fourth): the SECOND authority, swept

The sweep above varies the module's declared widths against ONE runtime, the build's default `Vm`.
That is only half the axis.

**Every width is carried twice**: by the module header `compile_with_target` writes, and by the
runtime type parameters of `GenericVm<W, A, F>`. The load check refuses a module WIDER than the
runtime and admits one NARROWER, and that asymmetry is where the second authority becomes visible.
It is where the original opaque-width defect lived.

`Word` is implemented for `i8`, `i16`, `i32` and `i64` and `Address` for `u8`, `u16`, `u32` and
`u64`, all unconditionally, so **sixteen runtime pairs are constructible in the default build** with
no `narrow-*` feature. `tests/composite_width_skew.rs` already relies on that for two hand-picked
runtimes; the grid generalises it from two points to all sixteen.

### Result

| | |
|---|---|
| cells | **9984** (16 runtimes by 48 descriptors by 13 shapes) |
| ran and returned the expected value | **3900** |
| refused at load | **6084** |
| refused at compile, faulted, or wrong | **0** |

Every load refusal is a module declaring a width wider than its runtime, which is **the guarantee
working**. Green at the default build and at all four narrow selectors.

### The harness is checked against an independent path

The loader's documented rule is that a module is admitted when no declared width exceeds the
runtime's. Evaluating that rule per cell, from the descriptor and the runtime's own trait constants,
yields a prediction the loader never sees. The measured 3900 agrees with it exactly. **A sweep that
silently skipped a runtime, or a refusal arriving from some check other than the width one, would
break the agreement**, so this tests the harness rather than the runtime.

**The first version of that check was wrong, and the narrow builds caught it.** It used a closed
form, the product of two triangular numbers, which assumes the runtime grid and the descriptor set
span the same widths. They do not: the grid is over concrete Rust types and is identical in every
build, while the descriptor set shrinks with the build's maxima. The form was right at the default
build and wrong at all four narrow selectors, reporting 2730 ran against 1170 predicted under
`narrow-word-16`. The per-cell rule makes no assumption about how the two sets relate.

### The control, and what it says about the defect

Reintroducing the sub-floor address produces **120 findings**, against twelve on the single-runtime
sweep, still on exactly one shape. They appear on **every runtime that admits the module**. That is
a statement about the defect's nature: it is a property of the MODULE's declared width, and the
second authority neither masks it nor creates it.

### What this does NOT establish

- The grid varies the word and address of the runtime and holds its float at `f64`. The float
  authority is exercised only from the module side. **Closed the same day; see the fifth addendum.**
- Thirteen shapes is still not every construct, and the source-derived population of this census is
  still a lower bound.
- No group in the table above that carries a probe count is closed by this.


## Addendum, 2026-09-10 (fifth): the third width, from both sides

The fourth addendum left the runtime's float fixed at `f64`, so the float authority was swept from
the module side only. Two changes close it.

**`f32` runtimes.** Each of the sixteen word-address pairs now runs on both float runtimes, so a
module declaring a sixty-four-bit float meets a runtime that cannot host it and must be refused at
load, the same asymmetry already swept on the other two widths.

**A shape that uses a float.** Without one the float width affected only admissibility and never a
computed offset. A `Float` field between the opaque and the word that follows it puts the float
width into an offset exactly as the address width enters through the opaque. **The value read back
is the integer field, never the float** -- a shape whose expected value were a computed float would
report an `f32`-versus-`f64` rounding difference as a wrong answer, which is a width difference the
sweep is not entitled to call a defect.

### Result

| | |
|---|---|
| cells | **21504** (32 runtimes by 48 descriptors by 14 shapes) |
| ran and returned the expected value | **6800** |
| refused at load | **14192** |
| refused at compile | **512** |
| faulted or wrong | **0** |

Green at the default build, at `narrow-word-8`, `narrow-word-16` and `narrow-address-8`, and in a
build with the `floats` feature absent, where the float shape is compiled out and the rest of the
sweep is unaffected.

### Both refusal counts land on their closed forms

The 512 compile refusals are the float shape against every descriptor declaring no floats: sixteen
such descriptors on each of thirty-two runtimes. The 6800 that ran match the per-cell prediction
exactly.

**The prediction had to get better to accommodate the float shape**, and that is an improvement
rather than an accommodation. It previously assumed every shape runs under every admissible
descriptor, so it could be a product. A shape that needs floats cannot run where the descriptor
declares none, and its refusal comes from the COMPILER rather than the loader. The prediction now
models each shape's own requirement, which is a truer statement of what the sweep claims.

### The control

Reintroducing the sub-floor address produces **200 findings**, against 120 on the sixteen-runtime
grid and twelve on the single-runtime sweep, still on exactly one shape. The count matches its own
closed form: eighty on no-float descriptors, eighty at `f5`, and forty at `f6`, the last halved
because only the `f64` runtimes admit a sixty-four-bit float.

### What this does NOT establish

- Fourteen shapes is still not every construct, and the source-derived population of this census is
  still a lower bound.
- No group in the table above that carries a probe count is closed by this.
- The float shape exercises the float width in a LAYOUT offset. Float arithmetic across a
  width-mismatched pair is not swept here -- **and stating it that way was nearly a mis-reading of
  the tree.** It is not an open gap. `tests/float_arith_width.rs` covers exactly that property,
  that arithmetic honours the module's DECLARED float width and not the runtime's, with every test
  declaring a 32-bit float on a 64-bit runtime and eight of ten narrowing sites established by
  mutation, the other two argued witness-free for a reason rather than merely unwitnessed. A
  limitation of this sweep is not a limitation of the tree, and the two must not be conflated.

  What WAS genuinely absent there, and is now present, is the two-authority differential: that file
  ran every case on ONE runtime, so it established the declared width governs *there* rather than
  that the answer is independent of the runtime. The same declared-`f32` module now runs on an
  `f32` runtime and an `f64` one and must agree bit-for-bit, on the same witnesses that file
  already established as width-discriminating. Mutation-checked: removing the `Op::Add` narrowing
  fails it.


## Addendum, 2026-09-10 (sixth): the sites the instrument cannot see, counted

The population above is derived by grepping for the variant where it is CONSTRUCTED, and this
document states what that misses: a site propagating the error from a helper, or mapping another
error kind into it, does not appear. It also notes that one such conversion exists and is included
only because the grep happened to see it.

**That conversion is `impl From<ScalarError> for VmError`, and the grep counts it as ONE site.** It
is one construction and many reaching paths. A malformed artefact arrives at it through every call
that can raise a `ScalarError`, and the table attributes all of them to a single group-A row.

### The paths, enumerated

`GenericValue::read_scalar_le` and `GenericValue::write_scalar_le` are the only functions returning
`Result<_, ScalarError>` that the runtime calls. Every call to either, in a function whose error
type is `VmError`, is a path to `InvalidBytecode` the variant grep does not see:

| file | call sites |
|---|---|
| `src/vm.rs` | **6** |
| `src/marshall.rs` | **4** |

Each converts through `?` in a `VmError`-returning function or through an explicit
`map_err(VmError::from)`; all ten were read individually rather than assumed from the pattern.

**So "the population is a lower bound" now has a number against it.** Forty-six constructed sites,
plus ten reaching paths collapsed into one of them. That does not make the enumeration complete --
the reasoning above applies to any future conversion, and only this one exists today -- but it
replaces an unquantified caveat with a count that a guard keeps current.

`tests/invalid_bytecode_indirect_sites.rs` pins the pair. **A failure there is not a defect**; it
means this document's population figure has gone stale, which is precisely what a lower bound
cannot tell you on its own.

### An overclaim caught by measuring it

The guard strips comments before counting, and the natural justification -- that both files'
documentation names these functions, so an unstripped count would include prose -- **is false
today**. Raw and stripped counts are both 6 and 4, because every prose mention omits the opening
parenthesis the pattern requires. The strip is defensive, not load-bearing, and the file says so;
its decoy carries the offending shape deliberately so the guard still fails if the strip is
removed.


## Addendum, 2026-09-10 (seventh): group G's other two sites, probed

Group G's entry says one of its three sites was probed and that "the other two concern
host-supplied opaque handles going stale, which is a different question and untested here". The
closing section names group G among what remains. Both routes by which a host-supplied opaque could
go stale are now probed, in `tests/opaque_across_reset.rs`, and **neither reaches an
`InvalidBytecode`.**

### Route one: an opaque in a persistent slot is refused at compile time

A private `data` slot's body survives RESET in the persistent region, so a composite bearing an
opaque and stored there would carry a registry index across a reset. **It cannot be stored there
at all**:

```text
data field type Handle is not a struct or enum: opaque types are not yet admissible in data
segment fields
```

The route is closed by construction, not by a runtime check. The probe admitted three outcomes --
resolve, fault, or compile refusal -- and the answer was the third; it was not guessed. The guard
asserts the MESSAGE, because a refusal for some unrelated reason would leave the route open for
every shape that reason does not cover.

### Route two: a host decoding a yielded opaque too late produces a `TypeError`

`src/vm.rs` documents that a yielded value stays arena-resident and the host must decode it before
the next `resume()`, which resets the arena, because "a read afterward resolves to a clean stale
error". **That was a claim about a host-facing contract with nothing checking it.** Doing exactly
the forbidden thing gives:

```text
TypeError("flat composite body read after the arena was reset; decode a yielded or returned
composite before the next resume (read-before-resume)")
```

**The variant is the census-relevant part.** Group G is a group of `InvalidBytecode` sites, and a
clean `TypeError` naming the contract is evidence that this route does not reach them. The test
asserts the variant is NOT `InvalidBytecode` and that the message names the contract, so a bare
"it failed" cannot be mistaken for the same result.

### What this changes, and what it does not

Group G's verdict stays **no witness found**. What changes is its parenthetical: all three sites
are now addressed rather than one, and the two host-facing ones are closed by a compile-time
refusal and a distinct error variant respectively, each recorded with the text that establishes it.

It does not make the group unreachable by a hand-built artefact, which the wire format admits and
which this census has never claimed to cover for any group.


## Addendum, 2026-09-10 (eighth): a per-line classification was attempted and is NOT published

The first addendum says this document "does not publish a per-line classification and guessing one
would be worse than leaving it open", and the group-K row above repeats it. That is not a
preference. **It was attempted on 2026-09-10 and the method failed.**

### The method, and how it failed

All forty-seven construction sites were extracted with their messages and assigned to groups by
what each message says. The derived group sizes then disagreed with the table in four places, and
**two assignments were demonstrably wrong on inspection**:

- `"no entry point"` and `"empty call stack"` were left unplaced. Neither is an index out of
  range, a composite operand form, a data-segment layout, or any other group's stated subject,
  yet both are in the population and the table's parts sum to its whole. They belong somewhere,
  and the messages do not say where.
- Group H was tallied at four and its own section names exactly three: `Op::Len` on a flat array,
  `Op::Len` on a flat tuple, and `Op::IsStruct` on a flat struct. The fourth was a message that
  reads like a should-never-have-been-emitted case and is not one.

**A classification with two known errors is worse than none**, because the value of this document
is that every site carries a verdict and a wrong row silently moves a verdict onto a site it was
never made about.

### What a sound derivation would require

Reading each site's surrounding code against the group's stated subject, rather than matching its
message. The messages were written to help a reader diagnose a fault, not to encode a taxonomy, and
several are equally plausible under two group definitions.

Until that is done, the unprobed members of groups E and I cannot be NAMED, only counted — which
is the state this document has been in since it was written, now with the reason recorded rather
than assumed.


## Addendum, 2026-09-10 (ninth): what a host SEES when it breaks a contract

Groups F and J are judged lower value above, on the grounds that "a host that supplies a mis-sized
buffer or an unregistered native has broken a stated contract". That is true, and it is not the
whole question.

**`InvalidBytecode` means *this artefact should never have been produced*.** When a HOST's mistake
is reported with it, the message directs the reader to distrust the bytecode, which is the one
thing that is not wrong. `docs/process/HANDOFF.md` records this for a single hot-swap site and
notes that changing which variant a public API returns is a breaking change.

**Measured in `tests/host_contract_faults.rs`, it is not a single site.**

| host mistake | variant |
|---|---|
| a hot swap whose data vector length does not match the new module's private slot count | `InvalidBytecode`, naming the mismatch |
| calling a native the host never registered | `InvalidBytecode`, naming the native |
| calling an entry point with an argument it does not take | **NOT `InvalidBytecode`** |

**The third row is the control and it is what makes the first two mean anything.** A runtime with
one error variant could not be said to choose it wrongly. This one distinguishes: an
argument-count mistake by the host gets a different variant, as does a late read under the
read-before-resume contract, which the seventh addendum measured as a `TypeError`.

So the observation generalises from one site to **both host-contract groups**: their members refuse
correctly, with messages that name the actual mismatch, under a variant that tells the host to
suspect its artefact instead of its own call. Every refusal above is CORRECT and none of this is a
defect report; **which variant carries them is the operator's call**, and it is a breaking change
either way.
