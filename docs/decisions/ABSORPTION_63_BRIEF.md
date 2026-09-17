# BRIEF — absorption 63

## Stamp

**Computed against `8977d983` (ours) and `77130d7f` (`origin/v0.2.3`), with
4 unabsorbed commits.**

## What the backlog contains, and why it is NOT like the last three

Four commits, and **two of them change `src/verify.rs` in substance** — 316 lines
— rather than documentation:

| commit | what it does |
|---|---|
| `d8c9cf56` | refuses a non-forward control-flow target before any region walk |
| `fc79c936` | bounds how deeply the region walks may nest, threading a `depth` through `analyze_yield_coverage` and cutting off at `MAX_REGION_DEPTH` |
| `f63a886d` | replaces a stack-margin claim in `src/parser.rs` that measurement refuted |
| `77130d7f` | the merge, bringing `tests/hostile_module_mutation.rs` and `tests/verify_hostile_termination.rs` |

**This is the first absorption in four that touches the reference's behaviour on
malformed input**, and that is precisely the surface this line has an open report
against.

## Prediction — FIGURES

1. **No conflicting files.** `merge-tree` computes a clean tree
   (`ea66a82b895c6184406ab4539e38c311234bbf66`) at this stamp.
2. **Corpus unchanged at 74 modules, 1 refused** — no `.kel` file changed.
3. **ISA unchanged at 63 of 66**; `src/bytecode.rs` is untouched, so the
   denominator still parses 66.
4. **Test population unchanged** — `native_codegen/` is untouched.
5. **Driven witnesses unchanged at 64 of 66.**
6. **Backend suite unchanged at 602.**

## Prediction — WHICH GUARDS TRIP

**`outstanding_reports.rs` IS THE ONE TO WATCH, AND I EXPECT IT TO HOLD — WITH
LOW CONFIDENCE.**

Report 1 is a `confine.rs` index panic on a truncated op stream. These commits
harden the verifier against exactly that family: hostile modules, non-forward
control-flow targets, unbounded region nesting. **If the fix reaches the
confinement path, report 1 stops reproducing**, and that guard's own instruction
is to **RETRACT rather than debug**. An un-retracted report becomes an accusation.

I predict it still reproduces, because the reported panic is in `confine.rs` and
these changes are in `verify.rs` — **but that is an inference from file names, not
from reading the fix**, and it is the weakest prediction in this brief. Report 2,
a multi-parameter stream faulting after its first rewind, is likewise near this
surface.

The others, predicted silent:

- **`corpus_fingerprint.rs`** — no `.kel` changed.
- **`opcode_denominator.rs`** — `src/bytecode.rs` untouched.
- **`lowering_robustness.rs`** — it allows the `confine.rs` panic BY ORIGIN FILE
  and asserts it has not gone. **Same exposure as report 1**, from the other side.
- **`shared_channel_discipline.rs`** — upstream rewrote `REVERSE_PROMPT.md` again;
  the addendum must survive intact and attributed, as it did at absorption 62.
- **`handoff_figures.rs`** — no derivable row should move.
- **`comment_citations.rs`, `upstream_premise_census.rs`** — scan
  `native_codegen/` only.

## The thing that would be a real finding

**A verifier that now rejects a module this backend lowers.** The two
implementations must agree on what is admissible, and a module the reference
refuses at load is one this backend must not silently accept. The corpus
differential compiles and runs 74 modules; if one begins to be refused, that is
the result, not a nuisance.

## Wrong turns to avoid

- **Debugging a report that stopped reproducing.** Retract it. The guard says so.
- **Assuming a `verify.rs` change cannot reach `confine.rs`.** Read the fix before
  asserting the boundary; the prediction above is explicitly marked as an
  inference from file names.
- **Patching a guard's constant.** A figure that moves is the result.
