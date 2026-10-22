# Strengthening the Order 1 and Order 2 completion claims

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**. Open work list, written 2026-10-20 from the Opus 5.5 audit of
feat/v02-order3-trap-analysis at 36ebe64d. The operator asked for a record of
what needs to be done to strengthen these claims and expects Codex or Claude to
pick it up. Items are ordered by how much each one would change the confidence a
reader may place in the claim.

## Verdict being strengthened

The audit judged the claims broadly correct with caveats. Order 1 is broadly
correct. Its criteria were written before the declaration and every test it names
exists. Order 2 is correct in a narrowed sense. The Keleusma validator agrees with
the reference on the stage corpus, while the original gate asked for the whole
verify verdict. Order 3 is not claimed complete, and that is accurate.

None of the items below asserts that a claim is false. Each names a place where a
reader cannot currently check the claim, or where the claim says more than its
evidence.

## Cross-cutting

1. **Run the claims through remote continuous integration.** The 65 commits
   from 89fc6aac to 36ebe64d have never run in remote continuous integration. Open
   a pull request to v0.2.3 and record the run identifier in each audit. Until
   then every receipt is local.
2. **Make receipts portable.** Every receipt cited by both audits is a log
   under the ignored tmp directory. Record in each audit row the exact command,
   the commit, the exit status and the pass and fail counts, so that the row can
   be reproduced without the log. A continuous integration run identifier is the
   strongest portable receipt.
3. **Match the continuous integration build profile.** The Codex matrix used
   `--release`. Continuous integration runs in debug, where overflow checks and
   debug assertions are active. A release pass is not evidence of a debug pass.
4. **Refresh the figures in CLAUDE.md.** The 3,236 and 2,978 totals describe
   99b7ab01. The 1407 and 1357 library counts predate later library tests. The
   label reads "Measured 2026-10-13" over figures edited on 2026-10-15. Re-measure
   at one commit and date the figures by that commit.

## Order 1

5. **Name a test for every acceptance row.** The rows for reference-scaffold
   independence and pipeline-owned type inputs cite production functions, not
   tests. The rows for complete serialization, the shipping command, compatibility
   and bounds, and the pilot rest on prose or logs. Each needs a named test that
   fails when the property regresses.
6. **Annotate superseded limits.** The audit cites a lexer limit of 393,216
   bytes, now 524,288. Mark such figures as historical with their date.

## Order 2

7. **Name the tests behind each row.** The rows for the typed verdict, the call
   graph, module summaries, the yield fixpoint, text allocation, resource
   arithmetic and shipping integration give counts without names. Only
   tests/selfhost_rejection_families.rs is named.
8. **Record the scope narrowing as a decision.** The roadmap gate reads
   "reproduces the whole `verify()` verdict (structural, typed, resource) for the
   stage corpus". The selected widening profile first appeared in a12c45c9 and
   1cfb6dd9 on the declaration day, and the completion plan was written about
   three and a half hours before the declaration. Record the narrowing explicitly,
   with the operator's acceptance if it is given, rather than leaving it implicit
   in the audit.
9. **Close or pin the permissive divergence.** The typed stage accepts a
   cross-join scalar witness that the reference rejects. Only the retained
   reference consistency gate catches it. Add a test that pins the divergence
   and fails if the reference gate is removed, or repair the stage.
10. **Correct the stale capacity.** The audit states 1536 opcodes per chunk.
    7a058917 raised it to 2048.

## Order 3, before any completion claim

11. **Complete the matrix on a fixed tree.** The last full run, at 09e74c94,
    failed in all five feature sets. The repairs were confirmed only by targeted
    runs.
12. **Widen the trap-free corpus.** All fourteen stages classify as possibly
    trapping, so agreement between the two analyses on a trap-free verdict rests
    on about a dozen small modules. The comparison also runs only under the
    self-host feature.
13. **Compare the tag table with the analysis.** The 66-tag test in
    src/selfhost/traps.rs compares against a table written in the test, not
    against classify_traps. Derive the expectation from the Rust analysis.
14. **Cover stale artifacts.** Bytecode built before the in-place contract
    changes still passes the version and fingerprint checks. The operator
    accepted this for the unpublished cycle. A test that loads such an artifact
    would show whether it is rejected or executes with changed meaning.
15. **Retire stale stage-count prose.** About twenty-five places still say eleven
    or twelve stages, mostly in tests/selfhost_wire.rs,
    tests/consts_region_composition.rs and tests/lexical_divergence_census.rs.
    Judge each as a dated ledger entry or a live claim. The present-tense instance
    in tests/claimed_counts.rs was corrected on 2026-10-20.

## Addressed on 2026-10-21

- Item 1. Both audits now record CI workflow run 39704059671, which passed every
  job at 89aaba69 through pull request 484. That run tested the tree after the
  audit follow-ups rather than 36ebe64d itself, as each audit says.
- Item 6. The Order 1 audit marks its lexer limit as historical and names the
  commit that raised it.
- Item 10. The Order 2 audit marks its opcode limit as historical and names the
  commit that raised it.
- Item 13. The per-tag test in src/selfhost/traps.rs derives its expectation from
  `trap_analysis::classify_instruction`, which `classify_traps` applies, and sends
  each subject through the real operand transport. Misclassifying BoundsCheck in
  the Rust analysis made it fail.

## Already addressed on 2026-10-20

The unchecked operand access in proven execution was replaced by retained
checks, and a per-opcode test now compares the operand-effect table with the
runtime. STANDARD.md and STRUCTURAL_ISA.md no longer describe faults that the
runtime no longer raises. See ORDER3_PROVEN_EXECUTION.md and
ORDER3_HANDLER_SURFACE_PLAN.md.
