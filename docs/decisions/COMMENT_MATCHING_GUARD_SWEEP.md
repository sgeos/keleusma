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
| `call_chunk_index_limit` | ABSENCE: no site may use the old radix | **repaired** — string-aware strip |
| `op_tag_tables` | anchor, then a bijection | **repaired** — one of its two extractions located on raw source |
| `wire_self_compile_status` | PRESENCE of the historical repair | **repaired** — a comment satisfied it |
| `stage_command_reach` | PRESENCE of three command declarations | **repaired** — a comment satisfied them |
| `composite_escape_routes` | anchor on the opcode enum | **repaired** |
| `forest_child_channels` | anchor on the body struct | **repaired** |
| `forward_data_reference` | two anchors feeding an ORDERING claim | **repaired** |
| `selfhost_bare_for` | ABSENCE of a removed refusal, plus a window | **repaired** |
| `selfhost_driver_parity` | COUNTS seeding calls | **repaired** |
| `consts_region_composition` | line prefix via `trim_start().starts_with` | **safe by construction** |
| `selfhost_parse` | line prefix | **safe by construction** |
| `selfhost_typecheck` | line prefix, and sources it defines itself | **safe by construction** |
| `reconstruct_failure_modes` | searches ERROR MESSAGES, not source | **not in the class** |

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

Only `call_chunk_index_limit` needs a string-aware strip, because truncating inside `"http://a"`
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

## THE MOST USEFUL RESULT IS NOT THE COUNT

Three of the repaired files **document the hazard in their own prose** and guarded one of two
readers anyway. And the defect was **committed inside its own fix**: an edit removing this shape
mixed stripped and raw offsets and had to be caught by running the tests.

**The class is therefore not carelessness that attention prevents.** That is why a mechanical sweep
found instances four documented prior incidents had not.
