# COMPLETION CONDITION — gate provenance

Ordering is NOT a completion criterion. These properties may be reached in any
order, by any route, on any branch. Judge only the end state of the tree.

## Required end-state properties

1. **The backend gate records what it verified.** After a successful or failed run,
   a tracked file in the tree carries, for the configuration that ran: the
   configuration, the commit the run was performed against, whether the worktree
   was clean or dirty at that moment, the verdict, and a timestamp. Both float
   configurations are representable simultaneously; running one does not erase the
   other's entry.

2. **The record is written outside every frozen window.** No gate phase that
   compares the tree against itself can observe the record being written.

3. **A reporter answers staleness by relevance, not by identity.** Some tracked
   command or test reports how many backend source changes have landed since the
   recorded commit. It does not treat "the recorded commit differs from HEAD" as a
   defect on its own, and it does not count the record file itself as a change.

4. **A guard checks the record, and its reach is demonstrated.** A test rejects a
   record that is malformed or that names a commit not present in history. The
   transcript shows this guard FAILING on at least one deliberately bad input —
   a demonstration on good input alone does not satisfy this.

5. **The guard is not a staleness enforcer.** It does not fail merely because the
   recorded commit is not HEAD.

6. **Real records exist.** The tree contains entries produced by actual gate runs,
   not hand-written placeholders, for both float configurations.

7. **The shared pre-push hook is unmodified.** The untracked hook shared with other
   checkouts is not altered. If the gap it leaves is described anywhere, it is
   described as the operator's decision, not closed unilaterally.

8. **The measured coverage facts are recorded.** Somewhere in tracked documentation:
   that continuous integration does not build this package, that the workspace
   selector cannot reach it because it is detached, and that the gate is therefore
   invoked only by choice. Each stated as measured, with what was measured.

9. **The record's limits are stated where it is defined.** The tree says in prose
   that a record attests a run occurred on a stated tree, and is not evidence the
   backend is green at any later commit.

10. **The suite is green in both float configurations**, with the run's own record
    reflecting the tree it ran against. No test is skipped, ignored, or deleted to
    reach this.

11. **No new opcode, and no `BYTECODE_VERSION` change.**

12. **Files under the repository-root `src/` and `tests/` are unmodified** by this
    work.

13. **Any figure stated in documentation as part of this work is one that was
    measured**, and no claim asserts a verification that did not run.

## Explicitly NOT required

- Any particular filename, format, separator, or command name.
- Continuous-integration coverage of the backend.
- A blocking mechanism preventing an ungated push.
- Any particular commit granularity, branch, or merge.

---

## Iteration 2 — fixed point

Ordering is not a completion criterion.

1. The tree carries machine-written records for both float configurations, naming a
   commit that is an ancestor of the tip, with a clean worktree and passing verdicts.
2. The staleness reporter, run at the tip, reports NO changed backend sources for
   either configuration.
3. That zero is reached WITHOUT narrowing what the reporter watches; the handoff
   remains among its inputs.
4. Tracked documentation states, as an observed result, whether the arrangement has a
   fixed point.
5. Local and remote agree, root `src/` and `tests/` are unmodified, and there is no
   opcode or bytecode-version change.

---

## Iteration 3 — README currency

Ordering is not a completion criterion.

1. No tracked figure in the backend package's README asserts a count or coverage claim
   that is neither re-derived by a guard, carried with an explicit measurement stamp, nor
   replaced by a pointer to the census that computes it.
2. Any figure found stale is corrected or removed, and the correction states what was
   measured.
3. No new guard compares two in-tree copies of the same figure; where a duplicate existed,
   the duplicate is removed rather than pinned.
4. Any new guard's reach is demonstrated by a perturbation whose failure MESSAGE is shown,
   not merely a failing status.
5. Both float configurations pass with a machine-written record, and its rows agree on the
   commit they name.
6. Local and remote agree; root `src/` and `tests/` unmodified by this line; no opcode or
   bytecode-version change; recorded test-population figures match the tree.
