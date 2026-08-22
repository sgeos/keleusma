# Brief — the remaining Order 1 work, and the file operand

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

A working brief, written 2026-08-20 for autonomous execution. It exists because the
operator granted a block of unsupervised time and the failure modes of unsupervised
work in this repository are already known and written down. **The value here is not
the plan. It is the list of specific wrong turns**, every one of which has been taken
at least once in this tree.

## The goals, and why these three

**G1 — derived binding rows from the pipeline.** Order 1 item 3. The declared
bindings — a function's return type and each parameter's type — already come from
`parse_functions`. A `let` bound to a literal or a call still produces no pipeline
row, because the initialiser's shape lives in the body record stream rather than in
the header. `the_pipeline_rows_are_the_declared_subset` pins that boundary and tells
the next increment to fold the case into the agreement test rather than delete the
pin. **Highest confidence: the work is named, the boundary is pinned, and the oracle
already exists.**

**G2 — the file operand and the sidecar fingerprint.** Ruled 2026-08-19: accept a
file operand, keep standard input as the default. `docs/decisions/PIPELINE_THEN_MONOLITH.md`
records the ruling and marks the fingerprint MANDATORY rather than conditional,
because the ruling is the one that keeps a sidecar reachable. **Self-contained, and it
touches a command-line surface rather than a stage source**, so it cannot break byte
identity.

**G3 — the `CONSTS` region.** Order 1 item 1, and the largest. Both recorded
obstacles were wrong: the interning-order conflict is unreachable for this corpus
(every corpus constant is `Int`, pinned by `the_flattener_interns_no_name_for_any_stage`),
and the size figures predate the all-default elision. What remains is the 170-node
flattener cap, about five batches for `parse` rather than a hundred and three.
**Stretch. Do not start it with a partial understanding of the two node caps** — see
below.

## The failure this tree keeps repeating, in its most recent form

**I derive a set from the part of the system I am thinking about, rather than from
the system.** Recorded seven times. The two most expensive instances were this week:

- I reported the ECC plane as unexercised because a decision document's status field
  said so. Eight tests already drove it on real compiler output. **That cost an
  operator ruling, which is scarcer than my time.**
- I framed the token array as a capacity question when the streaming it presupposed
  was already built and measured.

**The rule that actually works: read the tree before forming the question.** A
question citing a file and a line is worth more than one citing a status. Prefer an
instrument that prints raw evidence over one that prints a verdict.

## Wrong turns specific to G1

**THE BODY RECORD STREAM HAS THREE DECODERS AND THEY DIVERGE LEGITIMATELY.** The
driver, the parse harness and the codegen harness all consume it. When the `let`-name
record was added on tag 90, I asserted the blast radius before measuring it and eight
tests failed on `unsupported node kind 90` from the third decoder. **Only the tag is
shared; the skip sets differ on purpose**, because the codegen walker consumes kind 35
where the parse harness skips it. Enumerate all three before adding anything to that
stream.

**DO NOT INVENT A SECOND ENCODING.** Order 1 asks for the input to come from
`parse.kel` plus `reconstruct.kel` because the structure is available there. Every
previous slice honoured that: the parameter name was already in the stream and the
driver was discarding it; the `let` name was added under an operator ruling on an
explicit fork. **If a new record seems necessary, that is a signal to re-read the
stream, not to design a format.**

**THE STAGE MAY ALREADY SUPPORT THIS.** `verify_types.kel` gained a bounded fixpoint
in which a binding may take **form 2**, meaning "this binding takes whatever
expression node N yields", and the stage proves a tag for an operator node whose two
operands agree. **Check what the stage already does before writing stage code.** The
missing piece is plausibly only the host saying WHICH NODE, which is as syntactic as a
literal tag.

**THE DIVISION OF LABOUR IS CHECKED BY MUTATION AND MUST STAY THAT WAY.** The host
says only which node the initialiser is; the stage decides the type. Making
`tyb_node_tag` return unknown must fail the test. Without that control the host could
be supplying the answer and the test would still pass.

**THE TYPE CHANNEL IS KEYED BY NAME AND `Local` RECORDS CARRY A SLOT.** That mismatch
is what made the previous slice necessary. Do not key by slot for locals and by name
for everything else — that fork was put to the operator and decided the other way.

**COMPARE BY NAME STRING, NOT BY ID.** The reference extraction assigns ids by
insertion order as it walks; the pipeline uses the lexer's intern table. Comparing ids
compares the numbering rather than the content.

## Wrong turns specific to G2

**`--chunk` CAN ONLY BE OPTIONAL UNDER THIS RULING**, supplied if present and derived
if absent. That derivation is available only because the ruling admits a reopenable
input.

**THE FINGERPRINT IS THE POINT, NOT A NICETY.** A `--chunk=` naming a table built from
a different input produces a byte-plausible WRONG artifact. Without the fingerprint
the feature is aimed at exactly the property this project exists to prove. It converts
a silent wrong artifact into a refusal naming both files.

**DO NOT BREAK STANDARD INPUT.** It stays the default so cut pipelines keep working.

**THE CODEGEN STREAM'S SECTION BOUNDARY IS CATEGORY-DEPENDENT.** Phase one ends at
`Return` for an `fn`, `Reset` for a `loop`, and `Trap(1)` for a multiheaded dispatch,
and a multihead's per-head `Return`s are interior ops. A consumer that does not already
know the function's category cannot find the boundary. Under fusion the driver knows
it; across a serialised boundary it would not.

## Wrong turns specific to G3

**THERE ARE TWO NODE CAPS AND THEY ARE DIFFERENT CAPS.** I conflated them once and
told the other line their figure was wrong when it was right. The module-input walk
refuses past **1,024 nodes** (`nm_max_names`, error `-240`). The flattener out of
`wire.fin` refuses past **170**, `fin` being 1,024 words at six words a node. Only the
second is derived from a word count.

**`wire.fin` IS 1,024 WORDS AND ITS USERS OVERLAP.** Chunk records take 0..990 at
eleven each; the header rides 990..1001. `parse`'s chunks overran it once and silently
rewrote the header.

**DERIVE EVERY FIGURE FROM `tests/consts_region_composition.rs`, NEVER FROM PROSE.**
Every recorded size predates the all-default elision, which took the eleven-stage body
from 712,936 bytes to **109,552** (re-measured 2026-08-22; the 103,544 recorded for that
figure was itself measured before the stage sources grew, and `CONSTS` within it is
37,152 bytes, 33.9%, not the 90.5% the earlier records carry).

## Method rules that are not optional here

- **CAPTURE EXIT CODES OUTSIDE THE PIPE.** Six constructed statuses are on record,
  including `echo "CLIPPY OK"` running unconditionally and a pipe's exit read as
  clippy's. `${PIPESTATUS[0]}` or a plain redirect.
- **A COUNT OF GREEN LINES IS NOT A PASSING SUITE.** One report of "exited with code
  0" with forty green lines was `grep`'s exit while eighteen binaries never ran. **The
  tell was the SHAPE** — nothing in the list took the ninety-eight seconds
  `selfhost_parse` takes.
- **A CAP IS A FAMILY.** Twice in two increments a widening moved no wall because the
  counter indexed six or eight other arrays. Derive the family from the source and
  assert the derivation is non-vacuous.
- **A GUARD THAT CANNOT FIRE IS WORSE THAN NONE.** Before adding a check, construct
  the input that makes it fire.
- **A GUARD WITH A SCOPE NARROWER THAN ITS CLASS IS THE DEFECT IT PREVENTS.** One
  walked `src/` and `tests/` and missed a live copy in `compiler/`.
- **PREFER A BEHAVIOURAL GUARD TO A TEXTUAL ONE**, and if the behavioural one is
  unaffordable, say so in the source rather than shipping the textual one.
- **CONFIRM THE REFERENCE ACCEPTS A GENERATED PROGRAM** before concluding anything
  about a stage. Five probes measured something other than intended, including one
  generated against the REFERENCE tokenizer while the cap governed the STAGE's lexer.
- **THE REFERENCE PARSER'S `MAX_PARSE_DEPTH` IS 24** and is shared between chain
  position and arm-body nesting. A source-level probe cannot reach deep nesting;
  assemble chunks from ops instead.
- **APPEND TO A SLOT-ADDRESSED BLOCK, NEVER INSERT.**
- **READ THE FEATURE MATRIX OUT OF `ci.yml`.** Publishing a constant gated on
  `self-host` broke three CI jobs while every local check passed, because every local
  check had the feature on. Four `cargo check --tests` runs: `--no-default-features`,
  `--features signatures`, `--features self-host`, `--features signatures,shell`.
- **`--all-features` IS NOT A FEATURE SET THIS PROJECT PASSES.** It cascades the
  mutually exclusive narrow-word selectors and pulls in an SDL3 build.
- **ROOT `cargo fmt --all` DOES NOT REACH `compiler/`**, which declares its own
  workspace.
- **AN ITEM IS ITS ATTRIBUTES AND DOC BLOCK, NOT ITS `fn` LINE.** An insertion once
  detached an `#[allow]` from the function it applied to; restoring from `HEAD` and
  reapplying beat a third correction stacked on two bad ones.
- **DO NOT COMPETE WITH A VALIDATED WALKER.** An independently written one invented its
  own `If`/`EndIf` handling and reported 365 of 386 loops disagreeing. `EndIf` RESTORES
  the depth saved at its `If`.
- **PIN RATHER THAN REPAIR WHEN THE CHANGE IS A JUDGMENT CALL, AND SAY SO.** An error
  code, a public signature and a capacity are all observables.

## What to do when blocked

**Stop and record, rather than working around.** Stage two of the token residency is
blocked on a `ParsedFn` accessor decision, and the correct action was to establish the
blocker by measurement, revert the experiment, ship the preparatory half, and write
down the precise decision needed. **A workaround that widens scope unsupervised is
worse than a clean stop**, because the operator loses the choice.

**A partial win that degrades a margin is not a partial win.** Shrinking the token
array to clear the true floor would have cut headroom from eighteen percent to four
for a fifteen percent memory saving. Declining it was correct.

## G1 design, established by reading the tree on 2026-08-20

**THE REFERENCE CLASSIFIES A `let` THREE WAYS**, and the pipeline must match all three:

| initialiser | reference row |
|---|---|
| literal | `(name, literal_tag, 0)` |
| call | `(name, callee_name_id, 1)` — an alias hop the STAGE resolves |
| operator expression | **no row here**; needs the initialiser's node index, form 2 |

**DO NOT LOOK AT THE RECORD ADJACENT TO `LetIn`.** `LetIn` is node kind **5** and is
BINARY: it pops its right child then its left. In postfix emission the stream is
`[value][rest][name 90][LetIn]`, so the record before `LetIn` is the root of the
REST, not the initialiser. Reasoning from adjacency here gives the wrong node.

**USE THE RECONSTRUCTED FOREST INSTEAD.** `reconstruct_via_kel(records, category,
param_count) -> Body` already exists and is validated. `Body` holds
`nodes: Vec<Node { kind, arg, lhs, rhs }>` with private fields, which is fine because
`binding_rows_from_pipeline` lives in the same module. Walk for `kind == 5`, take the
left child as the initialiser, and read ITS kind. **This honours "do not compete with a
validated walker."**

**JOIN BY SLOT, NOT BY POSITION.** `LetIn`'s payload carries the frame slot, and
`let_names` holds `(slot, name)`. A slot join is a real join; fold-order pairing is
positional and would fail silently on a reordering. Shadowing is out of scope and is
already documented as such in `binding_rows`.

**Node kinds established**: leaf — Literal 1, Local 2, DataRead 11, Unit 20, 38.
Binary — BinOp 3, LetIn 5, Andalso 8, Orelse 9, DataAssignIn 12, IndexAssignIn 14,
ExprStmt 21, plus 31, 43, 44, 45, 60. Unary — Not 6, Neg 10, IndexRead 13, YieldExpr
24, Cast 26, FieldAccess 28, plus 29, 30, 37. Call packs `chunk + count*256` in `arg`.

**THE OPEN QUESTION FOR THE NEXT SESSION**: how a `Literal` node's arg encodes its
TYPE, since the row needs `Word`/`Bool`/`Byte` and not just the value. Determine that
from the stream before writing the classification, and if it is not recoverable, that
is a finding rather than a reason to invent a record.

**SCOPE THIS SLICE TO FORMS 0 AND 1.** Form 2 needs a node index surviving into the
type channel, which is a further slice. Restate the pin to name form 2 as what remains
rather than deleting it.

## G0 — A REGRESSION I INTRODUCED IN PR #175, FOUND 2026-08-20. DO THIS FIRST.

**`bool` IS THE BOOLEAN PRIMITIVE AND `Bool` IS AN ORDINARY NAMED TYPE.** Measured,
not inferred, by parsing each spelling and printing the `TypeExpr` constructor:

```
Word => Prim      bool => Prim      Byte => Prim      Float => Prim
word => PARSE FAIL   Bool => Named   byte => PARSE FAIL  float => PARSE FAIL
```

`bool` is the only lowercase primitive; the parser recognises it with
`at_lower("bool")`. The reference rejects `fn f(b: Bool) -> Word { 1 + b }` with
**"cannot add Word and Bool"** — a named type it cannot add, not a boolean.

**WHAT I DID WRONG.** In `d1148e76` I added `named_type_tag`, mapping
`Named("Bool")` to the stage's boolean tag, on the reasoning that "`Bool` does not
parse as a `Prim`, so a match on `Prim` alone silently drops every `Bool`
annotation". The observation was true and **the conclusion was backwards**: those
annotations are dropped because they are NOT booleans. I turned correct behaviour
into a defect. The `Word`, `Byte` and `Float` arms of that function are dead, because
all three parse as `Prim` and never arrive as `Named`.

**WHY THE TEST DID NOT CATCH IT, WHICH IS THE REAL LESSON.** I changed BOTH SIDES of
a differential comparison in the same way. The reference extraction learned to call
`Named("Bool")` a boolean, and `binding_rows_from_pipeline`'s `tag_of` maps the type
NAME string `"Bool"` to the same tag. **Two wrongs agreeing is a green test.** A
differential oracle only detects a defect introduced on ONE side, and nothing in the
suite noticed because I was the common cause.

**THE FIX.**

- Delete `named_type_tag` and restore `type_tag`'s `Named` arm to yield 0.
- In `binding_rows_from_pipeline`, `tag_of` must key on **`"bool"`**, not `"Bool"`.
  `"Word"` and `"Byte"` are already correct.
- Correct any test source that wrote `Bool` intending a boolean.
- **Add the case that would have caught it**: a parameter of user-named type `Bool`
  used where a boolean is required. The reference rejects it. Assert the stage does
  too. If the stage already rejected before the fix, say so rather than implying the
  test found something it did not.

**DO NOT "FIX" THE SECOND SIDE TO MATCH THE FIRST.** The temptation, on finding the
two extractions disagree after correcting one, is to make them agree again. The
reference compiler is the oracle; both sides must match IT, not each other.
