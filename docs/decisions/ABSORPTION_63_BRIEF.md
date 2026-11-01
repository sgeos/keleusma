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


---

## OUTCOME

**Merged cleanly. Backlog 0. Every prediction held, including the one filed with
low confidence and an explicit warning about its basis.**

| prediction | result |
|---|---|
| no conflicting files | **held** |
| corpus 74 modules, 1 refused | **held** |
| ISA 63 of 66, denominator 66 | **held** |
| test population unchanged | **held** |
| driven witnesses 64 of 66 | **held** |
| backend suite 602 | **held** — 592 + 10, both configurations, all eight phases FROZEN |
| **`outstanding_reports.rs` silent — LOW CONFIDENCE** | **held** — all five reports still reproduce |
| `lowering_robustness.rs` silent | **held** — the `confine.rs` panic it allows by origin is still there |
| `shared_channel_discipline.rs` silent | **held** — the addendum survived a second upstream rewrite |
| the other four silent | **held** |

**The verifier hardening did not reach the confinement path.** Two commits bounded
region-walk nesting and refused non-forward control-flow targets in `verify.rs`;
the reported panic is in `confine.rs` and remains. **The prediction was right and
its stated basis — file names rather than the fix — was the honest reason to
distrust it.** It is worth noting which way that cuts: had it been wrong, the
correct response was to RETRACT report 1, not to debug it.

## AND A LEAK WITH A NAME, BECAUSE THE LOG WAS CAPTURED THIS TIME

The narrow `corpus_differential` phase reported one leak:
`trap_child_runs_one_module_natively`. **A new test for the register, but not a new
mechanism** — it spawns a child that dies by signal, exactly like the only test
that had previously repeated there. The register's shape sharpens from *"one test
that leaks repeatably"* to *"the trap-child family leaks, and it has two
members"*.

The previous absorption's run reported *"2 leaky"* and could not name them,
because that command was piped through `tail`. **Capturing the whole log is what
turned an unusable observation into a refinement.**
