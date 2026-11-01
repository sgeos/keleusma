# Brief — driving the `CONSTS` streaming path, Order 1 item 1

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: opened 2026-08-21 (session 50, continued).
**Scope**: a test that drives stage commands 176/177 directly; then, only if that succeeds, the
driver side.
**Constraints**: no new opcode, no `BYTECODE_VERSION` change, stage sources must still self-compile
byte-identically, and `highest_command()` stays 181 unless a command is genuinely added.

## The situation, derived from the tree rather than from prose

`docs/` prose about `CONSTS` has been wrong twice — a record count read as a name count, and
figures that predated the all-default elision. **Take numbers from
`tests/consts_region_composition.rs` and from `wire.kel` itself.**

What those actually say:

- **`fl_walk` is capped at 170 nodes** (`fl_max_nodes()`), because the whole forest must sit in
  `wire.fin`, which is 1,024 words at six words a node.
- **The cap is the blocker, not any interning-order question.** Pinned by
  `the_node_walk_cap_is_what_excludes_the_stages`. The interning conflict is unreachable for this
  corpus: `the_flattener_interns_no_name_for_any_stage` pins that **every constant across all
  eleven stages is `Int`**.
- **Widening the array is a non-answer, not merely an expensive one.** A stage's private data array
  initialises one `Int(0)` per word, so a `fin` wide enough for N nodes adds `6 * N` records to the
  walking stage's OWN `CONSTS`. Pinned at exactly the node width, six to one.

## Why a streaming path exists and why it can work

`fl_walk` needs a **queue**: a composite's record carries `(first, count)` into children numbered
after every node at its own depth, so it cannot write a record until it knows how many nodes
precede its children.

**A forest of scalars has no children.** The queue never grows past the roots, the walk degenerates
to a linear scan, and then it is one node in, one record out, with no state but a cursor. That is
what `fl_stream_begin` (176) and `fl_stream_step` (177) are.

They refuse `-264` on a node with children, `-265` on a tag that interns, and `-266` on a tag
carrying a range. **That refusal is the point, not a limitation**: a composite reaching this path
would be emitted with a zero range and a zero `aux` — structurally valid, silently wrong, and
indistinguishable downstream from a correct record. Refusing keeps the gap visible instead of
encoding it in the bytes.

## THE ONE FACT THAT GOVERNS THE WHOLE INCREMENT

**Commands 176 and 177 have never executed.** They are written, dispatched at `cmd == 176` and
`cmd == 177`, and announced to the other line — and no driver or test has ever called them. Pinned
by `tests/stage_command_reach.rs`, with `CMD_STEP = 175` directly below them as the control that IS
driven.

So this is not "wire up an existing path". It is **validate never-run code, then wire it up**, and
the first half must not be skipped because the second half is more interesting.

## The order, and why it is not negotiable

1. **Drive 176/177 from a test, against a hand-built forest**, and compare the emitted bytes to
   what `fl_walk` produces for the same input. `fl_walk` is the oracle here because it is the path
   that has always run.
2. **Exercise every refusal**: a node with children, an interning tag, a range-carrying tag. A path
   whose refusals have never fired is a path whose refusals are guesses.
3. **Only then** consider the driver.

Doing 3 before 1 means a divergence could be in the stage, in the driver, or in the seam, with no
way to tell — which is the situation the four defects repaired earlier today all shared.

## The wrong turns, named in advance

1. **DO NOT COST THIS FROM THE PROSE.** Two recorded obstacles to `CONSTS` were both wrong: the
   interning-order conflict is unreachable for this corpus, and the figures predated the elision
   that removed 85% of the body. A third wrong belief is likelier than a first.

2. **DO NOT ASSUME THE MACHINERY IS MISSING — OR THAT IT WORKS.** Chained indexing was specified as
   three pieces and two already existed. Commands 176/177 are the mirror case: they exist and have
   never run. **Check, in both directions.**

3. **DO NOT LET `highest_command()` DRIFT SILENTLY.** It is a real guard; a new command returns
   `0 - 99` until it moves. If this increment needs a command, move it deliberately and say so.

4. **DO NOT WIDEN `wire.fin`.** The six-to-one ratio is pinned and the test says explicitly that if
   it ever inverts, the batching plan should be revisited. It has not inverted.

5. **DO NOT CONFLATE THE TWO NODE CAPS.** `nm_max_names()` is 1,024 and bounds the module-input
   walk; `fl_max_nodes()` is 170 and bounds the flattener out of `wire.fin`. This line conflated
   them once, told the other line their figure was wrong when it was right, and retracted.

6. **A REFUSAL PROVES WHICH LIMIT FIRED ONLY IF THE TEST NAMES THE ONE IT EXPECTED.** `-240`,
   `-264`, `-265` and `-266` are different causes. Assert the code, not merely that something
   refused.

7. **STOP AND RECORD IF THE STAGE SIDE DOES NOT VALIDATE.** If 176/177 turn out to be wrong, that
   is a complete and valuable result — never-run code found defective before anything depended on
   it. Reporting that is success, not failure.

## Proportionality

Nothing here changes what a user can compile. `CONSTS` is an emit-path region; the gap is that the
self-hosted stage cannot emit it for the larger stages, so those regions are host-supplied and
**not covered** by the self-hosting claim. Closing it widens what the byte-identity oracle proves,
which is the point of Order 1.

---

# RESULT — STEP ONE IS DONE AND THE PATH IS SOUND (2026-08-21)

**Commands 176 and 177 have executed for the first time, and they are correct.**

`tests/selfhost_wire.rs` drives both directly, reusing the existing `Call`/`run_call` harness rather
than adding a sixth way to drive the stage.

## What was measured

- **A scalar `Int` node streams a record matching the documented layout byte for byte** — tag u16
  at 0, flags u16 at 2, `aux` u32 at 4, payload u64 at 8. The expected record is built from the
  OFFSETS rather than from a captured blob, so a layout change fails loudly instead of being
  quietly re-baselined.
- **`aux` is confirmed written as zero rather than left alone.** The window is reused between
  calls, so a stale index from an earlier record is exactly the kind of wrong answer that looks
  right.
- **All three refusals fire, each asserting WHICH code came back**: `-264` a node with children,
  `-265` an interning tag, `-266` a range-carrying tag.
- **An accepting control passes**, so the refusals discriminate rather than describing a path that
  rejects everything.

The path needs no region and no directory, because it emits at window offset zero and the host
places the sixteen bytes. That is what makes it streamable and is why it does not inherit the
170-node cap.

## What this changes about the cost of `CONSTS`

The remaining work is now **driver wiring against a validated stage**, which is what the analysis
originally claimed it was — but that claim was only true after this step, not before it. Had 176/177
turned out defective, the wiring would have been built on a wrong foundation and any divergence
would have been attributable to the stage, the driver, or the seam, with no way to tell.

**`tests/stage_command_reach.rs` is narrowed rather than deleted.** It asserted the commands were
"driven by nothing"; that is no longer true, and it now pins the narrower fact that the DRIVER does
not reach them. The distinction is load-bearing: it is what makes a future divergence attributable.

## What is NOT done, stated plainly

- **The driver is not wired.** `CONSTS` is still host-supplied for the stages that exceed the walk
  cap, and a region whose payload comes from the host is **not covered** by the self-hosting claim.
- **The streamed output has not been compared against `fl_walk`'s** for the same forest. The walk
  writes into a region and needs a directory and a seeded artifact; the streaming path does not.
  Comparing them end to end is the next slice, and doing it properly means driving the walk through
  the full region harness rather than approximating it.
- **Multi-node streaming is unexercised.** One node in, one record out is proven; the cursor
  advancing across a forest is not.

---

# RESULT — THE EQUIVALENCE CLAIM, AND IT CLEARS THE CAP (2026-08-21)

**The streaming path reproduces the reference encoder's `CONSTS` region byte for byte, for a
forest the walk refuses.**

| case | nodes | walk (cmd 141) | streaming |
|---|---|---|---|
| one constant | 1 | accepts | byte-identical to the reference |
| 200 constants | 200 | **refuses `-240`** | **byte-identical to the reference** |

The 200-node case is the point. `fl_max_nodes()` is 170 because the whole forest must sit in
`wire.fin`; the streaming path holds ONE node and is not bounded by it. **This is not "streaming
also works" — it is streaming doing what the walk cannot**, which is the entire justification for
the path.

## What makes the comparison trustworthy

- **The oracle is `encode_aux_body`, the Rust encoder**, not `fl_walk`. For the 200-node case
  `fl_walk` could not have served as an oracle at all, since it refuses the input.
- **The refusal is asserted by CODE.** `-240` is the node cap specifically; a different code would
  mean the input failed for an unrelated reason, and this line has recorded three near-misses where
  a refusal was read as the wrong limit.
- **The all-scalar precondition is asserted per node.** Breadth-first and linear order coincide
  only when nothing has children; a composite entering the corpus would make the comparison
  silently measure two different orders.
- **A vacuity guard requires some case to exceed the cap**, so the test cannot degrade into the
  small case alone and keep passing.

## Two incidental findings

**`fn main() -> Word { 42 }` has exactly ONE constant.** The first version of this test used it
alone and therefore proved a single record while reading as though it proved a region. Caught by
measuring the corpus rather than trusting the label "scalars-only".

**A 200-term expression overflows the parser stack** rather than producing a parse error. That is
the recorded dispatch-depth ceiling, and it is why the corpus here uses 200 flat `let` statements
instead of one long sum. Recorded because the failure mode is a crash in the test binary, which
reads as a harness fault rather than a language bound.

## What is still NOT done

- **The driver is unwired.** `CONSTS` remains host-supplied for the stages past the walk cap, and a
  region whose payload comes from the host is **not covered** by the self-hosting claim. Nothing
  here changes that; it changes what the remaining work costs and how attributable it is.
- **No stage source has been streamed end to end.** The 200-node corpus is synthetic. `parse.kel`
  carries 817 constants and streaming it through the driver is the actual Order 1 deliverable.

---

# STOPPED HERE, DELIBERATELY — THE DRIVER NEEDS SOMETHING THE ANALYSIS DID NOT COUNT

Wrong turn 7 of this brief says to stop and record if the work widens. It widened, and this records
where.

## What the driver would need, measured against the code

| piece | status |
|---|---|
| the streaming stage commands | **validated** — correct, and past the walk cap |
| a place to branch in the region loop | **exists** — `CHUNKS` is the precedent |
| the window guard blocking a large region | **not a problem** — the `CHUNKS` branch `continue`s before it, and the streaming path emits at window offset zero with the host placing each record |
| `CONSTS` reaching the emitter at all | **it does not** — it falls into `_ => continue` and is silently skipped, which is why the region is host-supplied |
| **a faithful model of which constants the encoder emits** | **absent from the library** |

The last line is the cost nobody counted.

## Why that model is not a small thing

`tests/selfhost_wire.rs::encoder_const_roots` is not a neutral helper. It **mirrors encoder rules**,
and its own comment says the rule is *mirrored rather than approximated* — including the
all-default private-pool elision, which the encoder applies by writing `first = ABSENT` and storing
no records. Its comment states the consequence plainly: a model counting them would over-count the
region by the whole data segment, **which on a real stage is most of it**.

So wiring the driver requires either:

- **duplicating that model into `src/selfhost/mod.rs`** — a second implementation of an encoder
  rule, exercised in one place and not the other, which is precisely the defect class that produced
  four silent miscompiles earlier in this same session; or
- **lifting it out of the tests into the library** — a refactor across a file many other tests
  depend on, and one that changes what the differential oracle is made of.

Neither is a small change, and the first is the one that would look small.

## THE PATTERN IN THE ESTIMATES IS THE FINDING

Four cost estimates in this area have now been checked against the code:

| estimate | reality |
|---|---|
| the interning-order conflict blocks `CONSTS` | **unreachable** for this corpus |
| the recorded region figures | **superseded** by the all-default elision, 85% smaller |
| chained indexing needs three coordinated pieces | **two already existed** |
| the remaining `CONSTS` work is driver wiring | **wiring plus an encoder model** |

Three ran high, one ran low. **The pattern is not a direction — it is that none of them survived
contact with the code.** That is the argument for checking every one rather than for applying a
correction factor.

## What a resuming session should decide first

**Which of the two routes above** — duplicate or lift — before writing any driver code. It is a
judgment about where the encoder's rules should live, and answering it while mid-edit is how a
duplicate gets created by default rather than by choice.

---

# THE ROUTE DECISION, SHARPENED (2026-08-21, later)

The earlier entry left "duplicate or lift" for a resuming session. **Both framings were incomplete,
and one blocker turned out not to exist.**

## The blocker that was not real

I recorded the driver work as needing a model of which constants the encoder emits, and treated
`src/wire_schema.rs` as out of reach. **It is not — it belongs to `v0.2.3`.** That was established
by reading both lines' handoffs after an ownership escalation that needed no ruling; see the
ownership table in `docs/process/HANDOFF.md`.

So a third route exists that neither earlier framing considered.

## The three routes, and why the obvious one is not a drop-in

| route | what it is | cost |
|---|---|---|
| **a. duplicate** | reimplement root-selection in the driver | a second implementation of an encoder rule, exercised in one place — the class that produced four silent miscompiles this session |
| **b. lift** | move the test's model into the library | changes what the differential oracle is made of; the test and the driver would share a model, so a wrong model is wrong on both sides |
| **c. extract** | one definition the encoder ITSELF consumes | cannot drift, because the encoder depends on it |

**Route (c) is right in principle and is not mechanical.** `SchemaBuilder` calls `add_constant_pool`
**per contributor** and needs each returned `ConstRange` to build that contributor's record. It
cannot consume a flat list of roots. So the shared thing cannot simply be
`fn constant_roots(module) -> Vec<ConstValue>`; deciding what it IS — an iterator of contributors, a
visitor, or a pair of functions with one asserted against the other — is the actual open question.

## What is already known, so the next attempt does not re-derive it

- **The emission order is**: every chunk's constants in chunk order, then the data layout's
  `private_init` — **elided entirely when wholly default**, which on a real stage is most of the
  segment.
- **`WireAuxBody` is public**; `module_to_wire_bytes` builds one internally at
  `src/wire_format.rs:1633`. Its `op_byte_offset`/`op_record_count` fields require encoding the
  opcode stream first, but **those fields are irrelevant to constant roots**, so an extraction
  scoped to constants does not inherit that dependency.
- **`tests/selfhost_wire.rs::corpus_aux_of` hand-builds an approximation of that aux body.** It is a
  second construction and could drift from the shipping one. Replacing it with the encoder's own is
  a self-contained improvement that is worth doing whether or not the driver work proceeds.
- The stage side is validated and the branch point exists; see the earlier entries.

## WHY THIS IS RECORDED RATHER THAN STARTED

Four cost estimates in this area have been checked against the code and **none survived contact**.
Three ran high, one ran low. That is not a direction to correct for — it is a reason to decide the
shape before writing code, because a decision made mid-edit is a duplicate created by default
rather than by choice.

**The next session should pick between (a), (b) and (c) before touching the driver**, and should
know that (c) needs a design answer rather than a refactor.

---

# BRIEF — THE ROUTE IS (c), AND THE DESIGN QUESTION DISSOLVED ON READING THE CODE (2026-08-22)

The previous entry left three routes and said (c) "needs a design answer rather than a refactor".
**That was wrong, and it was wrong for the fifth time in this area**: an obstacle recorded from the
shape of an interface rather than from its body.

## What the route decision actually turned on

Route (c) was costed as hard because `SchemaBuilder::add_constant_pool` is called **per contributor**
and returns a `ConstRange` each time, so it cannot consume a flat list of roots. That is true and it
is not the obstacle, because **the flat list was never what had to be shared**.

`add_constant_pool` is a pure accumulator: it extends `const_roots` and returns `(first, len)`.
Everything about which roots reach the table is therefore structural — chunk constants in chunk
order, then `private_init` — **except one predicate**, the wholly-default elision. A predicate is
shareable by ordinary dependency. A range-returning contributor call is not, and only the second
was ever in the way.

So (c) is: the encoder and the model share the **elision predicate**, and the order is stated once
beside it.

## The figures were wrong by more than the route was

Measured on 2026-08-22 and pinned by tests:

| quantity | recorded | measured |
|---|---|---|
| `CONSTS` across the eleven stages | 645,312 bytes, 90.5% of the body | **37,152 bytes, 33.9% of a 109,552-byte body** |
| `parse`'s forest | 17,391 nodes | **857** |
| corpus auxiliary body | 103,544 | **109,552** |

The first two come from the same cause: **every figure counted the wholly-default private-slot
initialisers, which the encoder elides.** They describe a forest nothing emits. The doc comment
carrying them also claimed "every figure in this section is derived by a test", and no test asserted
any of them.

**The conclusions survive and the magnitudes do not.** `parse` at 857 nodes still exceeds the
170-node walk cap, so the cap still excludes the stages — six calls rather than a hundred and two.
The six-to-one widening argument still holds, because the ratio is the node width and does not
depend on the forest size. Stating both halves is the point: a correction that only reports "the
conclusion stands" teaches nobody why the number moved.

## The wrong turns, named for the next increment

1. **DO NOT MEASURE THE FOREST THE MODULE CARRIES.** `all_constants` in
   `tests/consts_region_composition.rs` includes the elided pool and is right only for an interning
   census. `keleusma::wire_schema::constant_roots` is the emitted set. Using the first for a size or
   a capacity is exactly how the 17,391 figure entered the tree.

2. **DO NOT MAKE THE ORACLE DELEGATE.** `the_all_default_initialiser_pool_is_elided_from_the_region`
   restates the elision rule and measures it at the bytes. It must keep restating it: a version that
   called the shared predicate would agree with a WRONG predicate. The agreement between the two
   statements is a separate test. This looks like duplication and is not.

3. **DO NOT PIN A REGION SIZE EXACTLY.** An exact record count fails on every stage edit, which
   trains its reader to re-baseline rather than to read. The bands here are wide enough for ordinary
   growth and far too narrow for a return to the pre-elision magnitude.

4. **THE COINCIDENCE IS MEASURED, NOT STRUCTURAL.** For this corpus the blob model
   (`const_roots_of`) and the emitted set are equal, because every stage's private pool is wholly
   default. A stage that gained one non-zero initialiser would part them silently. Anything built on
   the equality must consume `constant_roots`, not `const_roots_of`.

5. **THE SHARE IS OF THE BODY.** 33.9% of the auxiliary body and 37.5% of the summed region payloads
   are the same measurement of two different denominators. Quoting them interchangeably is how a
   percentage stops meaning anything.

## What is done and what is not

**Done**: the shared predicate, the one `constant_roots` definition, the test-local copy delegating
to it, the corrected figures pinned by tests, and three mutations demonstrating each new guard fails
when the thing it guards is broken.

**Not done**: the driver still does not emit `CONSTS`. That remains the Order 1 deliverable and it is
now a smaller job than the record said — a stage's forest is hundreds of scalar roots, not tens of
thousands — but it is still a job, and the streaming commands it needs were validated separately and
have never been driven from `src/selfhost/mod.rs`.

## WHAT THE NEXT SLICE NEEDS, CHECKED AGAINST THE CODE RATHER THAN COSTED

The driver still does not emit `CONSTS`. Before costing that, three things were looked up:

| piece | expected | found |
|---|---|---|
| a `ConstValue` to wire-tag mapping in the driver | absent, and adding one would be a new copy | **already there** — `const_tag_and_name` in `src/selfhost/mod.rs`, complete for all eleven tags |
| the child / flags / discriminant extraction | absent | **already there**, inside `push_blob_node` |
| the emitted root set | absent from the library | **now `keleusma::wire_schema::constant_roots`** |

**So the remaining driver work is assembling six words per node and looping, not building machinery.**
That is the fourth time in this area that a recorded obstacle dissolved on being looked up, and the
second time the dissolution was in the direction of hidden PROGRESS rather than hidden cost.

**A SEPARATE FINDING, HALF CLOSED.** The `ConstValue` to tag mapping exists in three Rust
statements — `flatten` in `src/wire_schema.rs`, `const_tag_and_name` in the driver, and
`push_preorder` in `tests/selfhost_wire.rs` — plus the stage's own `fl_tag_*` predicates.

**The claim "they agree today" was checked rather than asserted, and the check found the interesting
part.** `flatten` names `wire_schema::tag::*`. The driver wrote the bare literals `1..12`, so the
two agreed by coincidence: the tag numbering is the wire contract, and renumbering it would have
left the shipping driver emitting the old contract with nothing to notice. **That is now closed** —
the driver names the encoder's constants.

What remains open is the arm LIST rather than the numbering, and it is not mechanical: extracting a
shared mapping would have to cover `StaticStr`, `Struct` and `Enum`, whose records carry an `aux`
that `flatten` computes from a name interner it owns. A partial extraction covering only the scalar
arms would be a fourth statement rather than a third, so it is recorded rather than started. The
test-local `push_preorder` is deliberately left restating the numbers, on the same ground as
`KIND_CONSTS`: a test of a wire contract that imports the contract cannot catch the contract
changing.

---

# RESULT — THE DRIVER EMITS `CONSTS` FOR EVERY STAGE, BYTE-IDENTICALLY (2026-08-22)

`keleusma::selfhost::wire_consts_via_kel` drives commands 176 and 177 over a module's constant
forest and reproduces the reference encoder's `CONSTS` region **byte for byte for all twelve stage
sources**, including the two the breadth-first walk cannot process at all.

This is Order 1 item 1. `CONSTS` is the largest single region of a stage's auxiliary body — 37,152
bytes across the eleven stages, 33.9% of the corpus body — and until now its payload came from the
host, which means it was **not covered** by the self-hosting claim in any degree.

## Why it turned out to be a small change

Everything it needed already existed and the record said otherwise, for the fifth time in this area:

- `const_tag_and_name` in the driver, complete for all eleven tags.
- `push_blob_node`'s child, flag and discriminant extraction, now shared as `const_children` and
  `const_flags_and_discriminant`.
- `window_emit_chunks`'s coroutine discipline, now shared as `enter_wire` — build the virtual
  machine ONCE and resume, because calling a suspended coroutine stacks an activation and a
  several-hundred-record region exhausts the arena that way.
- `constant_roots_of_module`, added so a caller holding a `Module` need not build a second
  approximation of the encoder's input to ask which roots it emits.

## THE GUARD THAT WAS SUPPOSED TO ANNOUNCE THIS COULD NOT HAVE FIRED

`tests/stage_command_reach.rs` pinned that the driver did not reach 176/177, and was written
"pinned in the firing direction: when the driver drives them, this fails". **It did not fail.** It
searched the driver source for the STAGE's function names, `fl_stream_begin` and `fl_stream_step`,
and the driver addresses the stage by COMMAND NUMBER and never writes those names at all.

**Second instance of "a guard that cannot fire is worse than none"** on this line; the first
compared `directory.len()` against a stage buffer when that length is the shared array's size, false
by construction. The replacement derives from the command numbers and **was made to fail** against a
driver with those constants renamed.

## WHAT A GREEN RUN DOES NOT ESTABLISH, FOUND BY MUTATION

**Swapping the `flags` and `discriminant` words in the driver's six-word node passes every test.**
Every constant in the corpus is an `Int`, so both words are zero on every record compared, and
swapping two zeros changes nothing.

The first draft of the test recording this asserted a stronger thing — that a non-zero flag is
UNREACHABLE, since only an enum sets one and the path refuses enum tags. **The witness could not be
constructed.** Two shapes were tried and both fold to a discriminant `Int` at compile time:
`const data k { e: E = E::B }` gives `Int(0)`, `let e = E::B` gives `Int(1)`. Neither produces a
`ConstValue::Enum`.

So the recorded position is: no source reaching this path was found that produces a flag-bearing
constant, and **two attempts is not a search**. Asserting unreachability from two probes would have
been the seventh instance of deriving a set from the part of the system one is thinking about.

Two of the three refusals ARE exercised through the driver, each asserted by its own code: `-264` a
node with children, `-265` an interning tag. `-266`, a range-carrying tag, is not, and the test says
so rather than leaving a reader to infer that all three are covered because two are.

## What is still not done

- **Placement and the directory.** This emits at window offset zero and the host concatenates, which
  is what makes it streamable and therefore what it does not test. Assembling a whole artifact from
  the self-hosted regions is a separate question.
- **The remaining region kinds**, Order 1 item 2. `STRUCT_AUX` and `ENUM_AUX` remain empty in every
  stage, so a byte identity for either would pass while emitting nothing.
- **A shared `ConstValue`-to-tag mapping.** The interning arms compute `aux` from a name interner
  `flatten` owns; covering only the scalar arms would be a fourth statement rather than a third.
