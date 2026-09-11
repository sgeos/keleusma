# Guard reach census: which of this line's guards were SHOWN able to fail

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: population derived, verdict recorded for every member, and every member now rests on
either a demonstrated check or a measured argument that no check is possible. Three were repaired
in the course of the census and one more afterwards; the last is recorded as safe by construction
with the measurement that established it.

## The question, and why it is not the same as "do the tests pass"

A passing check is evidence about the CHECKER's reach before it is evidence about the tree. This
line of work produced a large number of source-reading guards, and each one's value rests entirely
on whether it can fail when the property it names is violated. Several of them say in their own
prose that they discriminate. **Saying so is not the same as having shown it**, and the difference
is invisible from a green run.

## Derivation, so it can be re-run

The population is the test files added or modified between `639108fd` and `b74380a2`, which spans
this line of work:

```
git diff --name-status 639108fd..b74380a2 -- tests/
```

Eighteen files: four added, fourteen modified. The verdicts below are read from the tree as it
stands, not from the state at `b74380a2`, so a file repaired during the census is recorded with its
current verdict and the date of the repair.

## Verdicts

**Demonstrated in the file itself** — the file records a mutation of its own subject and the
observed result, so a reader of the guard can see why it is trusted.

| file | the demonstration it records |
|---|---|
| `block_comment_tripwire.rs` | both directions of the detector pinned in-file, including that a real block comment IS found |
| `call_chunk_index_limit.rs` | the historical note that fired the absence guard, measured |
| `claimed_counts.rs` | mutation-tested at the time of its repair |
| `composite_width_skew.rs` | a four-site mutation table, plus narrow-build reach measured 2026-09-10 |
| `flat_float_field_width.rs` | the header names the mutation that establishes the corpus is not vacuous |
| `float_arith_width.rs` | a zero-width float module measured compiling, loading and returning 3.75 |
| `module_runtime_width_skew.rs` | a per-axis table of which mutations were caught and which were not |
| `narrow_vm.rs` | **repaired 2026-09-10**, see below |
| `op_tag_tables.rs` | both directions mutation-tested, including that a duplicate tag still fails |
| `selfhost_bare_for.rs` | the false failure a historical note produced, measured |
| `selfhost_counter_reset.rs` | mutation-tested by deleting the historical repair itself |
| `selfhost_driver_parity.rs` | **repaired 2026-09-10**, see below |
| `selfhost_typecheck.rs` | several mutation-tested claims; the witness pairs assert the rejection MESSAGE, not merely that a rejection occurred |
| `stage_command_reach.rs` | a silent false pass measured as `2 passed, 0 failed` |
| `wire_self_compile_status.rs` | measured reporting `ok` with the real reset deleted and its text left in a comment |
| `forward_data_reference.rs` | **repaired 2026-09-10**, see below |

**Both remaining entries were resolved on 2026-09-10, and they resolved DIFFERENTLY.** The
judgement recorded here previously was that inventing a fourth near-identical test might be worth
less than saying plainly that two files rest on an unchecked property. That judgement was reversed
on new information: the guard added to `forward_data_reference.rs` under the same argument was
mutation-checked, and with its comment strip removed it FAILS. The pattern is load-bearing rather
than ceremonial, at least once, which is the thing the earlier judgement was uncertain about.

Asking the question of each file produced opposite answers, which is the useful part.

| file | verdict |
|---|---|
| `forest_child_channels.rs` | **repaired, and the strip shown load-bearing.** Its extraction splits on `": "`, which a comment satisfies as readily as a field: a line reading `// channel: Vec<u32>` becomes the pair `("// channel", "Vec<u32>")` and enters the field list as a seventh channel that does not exist. Measured — with the strip removed the new guard fails, reporting the phantom |
| `composite_escape_routes.rs` | **safe by construction, established by measurement rather than inspection.** Removing `code_only` entirely leaves all ten tests in the file passing. A comment cannot contribute an opcode name: a line beginning `//` is skipped, and the anchor line itself fails the uppercase-identifier test, so two independent filters reject it |

**The first attempt at a guard for `composite_escape_routes.rs` was VACUOUS and was reverted rather
than shipped.** A decoy naming the anchor in a comment produced the identical opcode list with the
strip removed, so it demonstrated nothing; retargeting it at the per-line filters did no better,
because removing either filter still leaves the phantom rejected by the other. **A test that cannot
fail is worse than no test, because it reads as coverage**, so the file keeps its strip and gains
no guard, and this table records why.

## The three repaired during the census

**`narrow_vm.rs`.** Three assertions were widened from a fixed message to a predicate accepting any
of three width fields, so they could keep running at a narrow build instead of being excluded. The
predicate's doc states that it "deliberately does NOT accept any error at all", because a rejection
for an unrelated reason would pass such a check and the test would stop meaning anything. **That
was a claim.** A later edit relaxing the predicate toward "any rejection" would have made three
tests vacuous with every one of them still reporting ok. It now has a negative case built from real
error wordings taken from elsewhere in the runtime, and a positive one so it cannot be narrowed into
uselessness instead.

**`selfhost_driver_parity.rs`.** Only the false-FAILURE direction had been verified. The
silent-false-PASS direction was measured on 2026-09-10 by deleting a real seeding call from the
shipping driver and leaving the identical text in a comment. The guard failed, naming the slot and
both counts. The control matters more than the result: with the comment strip disabled and the same
mutation in place, **the guard reported `ok`**. The strip is therefore load-bearing rather than
decorative, which is a stronger statement than the guard merely passing.

**`forward_data_reference.rs`.** Its ordering assertion reads two positions and compares them, and
the comment beside it says that a comment naming either anchor can move a position and make the test
assert the wrong thing about the stage rather than fail loudly. Nothing checked that. A synthetic
source now places a historical note naming the LATER declaration before the earlier one, which
inverts the order on raw source, and the test confirms the strip prevents it. Measured: with the
strip removed, the new test fails.

## What this census does NOT establish

- It asks whether each file records a demonstration, not whether the demonstration was SUFFICIENT.
  A file with one mutation and a file with six both read as "demonstrated" here.
- Files outside the derived range are not examined. The tree contains many older guards and nothing
  here says anything about them.
- "Demonstrated" is a property of the file's recorded evidence. Two of the entries above rest on
  measurements made in earlier sessions and re-read rather than re-run.
