# BRIEF — a gate that leaves evidence

## The gap, measured 2026-09-24

Three instruments could cover `native_codegen`. Measured, not recalled:

| instrument | covers the backend? | evidence |
|---|---|---|
| continuous integration | **no** | `grep -c native_codegen .github/workflows/ci.yml` = 0, across 14 jobs |
| the shared pre-push hook | **no** | runs `cargo nextest run --workspace`; this package is DETACHED, so the selector cannot reach it |
| `tools/backend-gate.sh` | yes | and it is invoked only when a human chooses to |

The gate prints `PASS` to a terminal and exits. **Nothing is written down.** So the
question "was the backend green at commit X?" has no answer in the tree, and every
handoff's "green in both configurations" is a claim a reader must take on trust.

This is not hypothetical drift. It is the shape of the ELEVENTH catalogue row, which
this session's author produced: a gate ran to completion on a tree carrying probe
files that were never committed. `frozen-run.sh` correctly reported FROZEN, because
the files were present for the entire window — freezing proves the tree did not MOVE,
never that it was the COMMITTED tree. The result was a green verdict belonging to no
commit. A record naming the commit and the clean/dirty state would have shown it at a
glance.

## What to build

The gate writes one line per configuration recording: configuration label, the commit
`HEAD` named, whether the worktree was clean, the verdict, and a UTC timestamp. A
reporter answers the question a future session actually has — *not* "does this match
`HEAD`", which is uninformative, but **"have any backend sources changed since the
commit that was verified?"**

## Wrong turns to avoid

**Do not touch the shared pre-push hook.** It lives in `.git/hooks`, is untracked,
and is shared with the `v0.2.3` line's checkouts. Adding a 26-minute backend gate to
every push from either line is a cost the gate script's own header already assigns to
the operator. Report the gap; do not close it unilaterally.

**Do not make the guard assert that the record matches `HEAD`.** Committing the
record necessarily produces a commit whose parent is the verified one, so such a guard
is red the moment it is written and red after every docs commit. A guard that is
normally red trains its reader to ignore it, which is worse than no guard. Check the
record's SHAPE and that its commit is real; report staleness, do not enforce it.

**Write the record only after every frozen window has closed.** `frozen-run.sh`
compares `git status --porcelain` across its own window; a write landing mid-window
would make the gate perturb its own freeze check — the exact fault of catalogue row
eleven, reintroduced by the fix for it.

**Prove the guard's reach before trusting it.** A passing guard is evidence about the
guard before it is evidence about the tree. Feed it a malformed record and a
fabricated commit and SHOW it going red; a guard demonstrated only on good input has
demonstrated nothing. This is recorded in memory as its own lesson.

**A record is disclosure, not a warrant.** It says a run happened on a stated tree
with a stated verdict. It does not make the run correct, does not make a dirty-tree
run acceptable, and must never be cited as evidence the backend is green NOW.

---

## Iteration 2 — does the arrangement have a FIXED POINT?

The first iteration left a standing warning: the reporter counts the handoff as a
test input, the handoff was edited after the gate ran, so reach is nonzero at the tip.
Documented and honest — but it raises a question the design must answer. **If every
gate run is necessarily followed by a handoff edit, and every handoff edit invalidates
reach, the warning is permanent.** A permanently-warning instrument is the thing this
brief argues against in its own "wrong turns": a reader learns to ignore it.

The claim to test is that a fixed point exists, reached by running the gate and then
committing ONLY the record, which reach excludes by construction.

**Wrong turn: clearing the warning by narrowing the reporter.** Dropping the handoff
from reach would make the warning vanish and restore exactly the false assurance the
widening removed. The handoff is read by `handoff_figures.rs`; that is a fact about
the suite, not a preference.

**Wrong turn: settling this by argument.** The reasoning above is a hypothesis. This
line's method is that a claim about an instrument is checked by running the
instrument. Run it.

**Wrong turn: touching the handoff after this iteration's gate run.** That edit is the
variable under test. If the fixed point requires not editing the handoff last, say so
plainly as a property of the arrangement.

---

## Iteration 3 — the README is the same shape as the banner

The handoff banner drifted 121 commits because `handoff_figures.rs` guarded the state table
BESIDE it and nothing read the banner. **Proximity to a guard is not coverage.**
`native_codegen/README.md` has the same shape: it sits next to a censused package and is
itself unguarded, and it records having been badly wrong before in its own voice — "It
understated the backend by more than half and pointed a reader at finished work. A
hand-maintained opcode list is what drifted." It fixed that one list. The question is what
else it asserts that no guard re-derives.

**Wrong turn: guarding a figure with no derivable referent.** That is what a
`measured at <commit>` stamp is for; a check invented for it would be guessing.

**Wrong turn: guarding a DUPLICATE.** Copying the state table's figures into a README guard
makes two things to keep in step. The gate-record row points at `GATE_RECORD.md` rather than
restating its hash, and that decision paid off one commit after it was made. Prefer DELETING
a restated figure and pointing at the census that computes it.

**Wrong turn: an unscoped extraction.** The README, like the handoff, carries historical
notes in which an old number is correct AS HISTORY. The banner guards are scoped to the
banner region for exactly this reason.
