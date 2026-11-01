# Every guard that searches source for a code pattern, with a verdict

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Why this exists

This repository had recorded **four** instances of a guard matching prose it was never meant to
read: a must-fire guard firing on the comment explaining the fix it guarded, a no-copies guard
flagging itself, a witness extractor matching its own English header, and a corpus reader matching
`for k in 0..3` inside a comment.

Four instances is a class. Two more were then found **by reading**, which raised the question this
document answers: **how many are there, and which ones are still exposed?**

## The population, derived mechanically rather than noticed

Test files that read `.rs` or `.kel` source **and** search it for a literal containing code
punctuation. That property, not inspection, produced the list.

| file | what it does | verdict |
|---|---|---|
| `tests/call_chunk_index_limit.rs` | ABSENCE: no site may use the old radix | **repaired** — string-aware strip |
| `tests/op_tag_tables.rs` | anchor, then a bijection | **repaired** — one of its two extractions located on raw source |
| `tests/wire_self_compile_status.rs` | PRESENCE of the historical repair | **repaired** — a comment satisfied it |
| `tests/stage_command_reach.rs` | PRESENCE of three command declarations | **repaired** — a comment satisfied them |
| `tests/composite_escape_routes.rs` | anchor on the opcode enum | **repaired** |
| `tests/forest_child_channels.rs` | anchor on the body struct | **repaired** |
| `tests/forward_data_reference.rs` | two anchors feeding an ORDERING claim | **repaired** |
| `tests/selfhost_bare_for.rs` | ABSENCE of a removed refusal, plus a window | **repaired** |
| `tests/selfhost_driver_parity.rs` | COUNTS seeding calls | **repaired** |
| `tests/consts_region_composition.rs` | line prefix via `trim_start().starts_with` | **safe by construction** |
| `tests/selfhost_parse.rs` | line prefix | **safe by construction** |
| `tests/selfhost_typecheck.rs` | line prefix, and sources it defines itself | **safe by construction** |
| `tests/reconstruct_failure_modes.rs` | searches ERROR MESSAGES, not source | **not in the class** |

**A line-prefix search is safe because a comment line begins with `//`** and therefore cannot match
a pattern that must start the trimmed line. That is a property of the search, not a judgement about
the file, which is why those three need no change.

## THE DIRECTION RULE, WHICH IS THE TRANSFERABLE PART

The correct strip is **not the same for every guard**, and choosing by appearance is wrong in both
directions.

| assertion | what an early truncation costs |
|---|---|
| **ABSENCE** | a missed offender **passes silently** |
| **PRESENCE** | a real occurrence is hidden and it **fails loudly** |
| **anchor** | the anchor goes missing and the `expect` **fails loudly** |
| **COUNT** | a phantom or a miss, both **loud** as a mismatch |

Only `tests/call_chunk_index_limit.rs` needs a string-aware strip, because truncating inside `"http://a"`
would drop a real occurrence and its assertion is the one where that is silent. The rest fail
loudly, so the naive form is correct in each.

**They are deliberately not unified into a shared helper.** Doing so would add cost to eight and
remove a needed guard from one. Each carries the comparison instead.

## What this does NOT establish

**It is "no further site found", never "no further site exists."** The derivation matches a literal
containing code punctuation; a guard searching for a bare identifier with no punctuation would not
appear, and neither would one that reads source through a helper this scan does not follow.

### THE BLOCK-COMMENT GAP IS REAL, MEASURED AT ZERO, AND NOW TRIPWIRED

**None of these strips handles `/* … */`**, which needs cross-line state a per-line walk does not
carry. Rather than leave that as a note, it was measured.

**The exposure today is zero.** No source any of these guards reads contains a real block comment.
The only `/*` in the stage sources sits inside a LINE comment describing the `+`, `-` and `*`
operators, and every occurrence under `src/*.rs` is inside a doc comment or a test string literal.

**But Keleusma supports block comments** — `src/lexer.rs` skips them and has tests for the
multi-line and unterminated cases — so a stage source could gain one and so could a Rust file. The
risk is latent, not absent.

**Teaching nine helpers cross-line comment state is real complexity bought for no current
exposure.** `tests/block_comment_tripwire.rs` converts the latent risk instead: it fails if a block
comment ever appears in one of these files, and names this document. Cheaper than nine parsers, and
unlike a note in a document it cannot be forgotten.

Its detector deliberately does **not** fire on a `/*` inside a line comment or a string literal,
both of which this tree contains — a detector that flagged either would fire on exactly what is
recorded here as harmless, which is the too-loose direction this whole class is about. A second
test pins those two shapes so that tightening the detector until it reports nothing cannot pass for
a fix.

The same distinction this repository records for `Op::IsStruct`, declared producerless with four
producers found within the hour.

## I FIXED WHAT THE GUARD CAUGHT, NOT THE CLASS — MEASURED AFTERWARDS

`tests/comment_citations.rs` scans two documents: the handoff and the reverse prompt. It flagged two
bare file names in the reverse prompt, and they were corrected.

**The same two names were also in the task log's newest currency note, and were not corrected**,
because nothing flagged them. That is the identical one-of-two-sites shape this document catalogues
in other people's guards, committed while cataloguing it. They are paths now.

**The append-only design journal keeps its bare names deliberately.** Its entries are dated records
of what was written, not live claims, and correcting them would edit history to satisfy a guard that
does not read it.

### SHOULD THE TASK LOG BE SCANNED? MEASURED, AND LEFT AS THE OPERATOR'S CALL

Scanning it whole is **not** viable: measured with the guard's own resolver, **sixty** of its
citations name identifiers that no longer exist. Inspection shows most are legitimate — pins deleted
as the work moved on, such as a test asserting the self-hosted compiler could not yet compile
`wire.kel`, removed when it could. **A currency note is a dated record of a past state**, so its
names going stale is the file working correctly.

**The newest note is different**: it is the live status, and its citations should resolve. Scanning
only the most recent note is a bounded and meaningful check.

It is **not adopted here**. It needs a rule for where a note begins and ends, and it puts a
recurring cost on whoever writes the next one. That is a per-increment cost and therefore the
operator's call, recorded with its measurement rather than taken.

## THE MOST USEFUL RESULT IS NOT THE COUNT

Three of the repaired files **document the hazard in their own prose** and guarded one of two
readers anyway. And the defect was **committed inside its own fix**: an edit removing this shape
mixed stripped and raw offsets and had to be caught by running the tests.

**The class is therefore not carelessness that attention prevents.** That is why a mechanical sweep
found instances four documented prior incidents had not.
