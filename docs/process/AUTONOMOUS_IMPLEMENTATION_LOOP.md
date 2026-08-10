# Autonomous Implementation Loop

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The substrate for driving the self-hosted-compiler frontier as a bounded autonomous
loop. It is the item-7 deliverable of the 2026-07-22 process audit, and it extends the
**Autonomy Boundaries** in [PROCESS_STRATEGY.md](./PROCESS_STRATEGY.md#autonomy-boundaries)
with a concrete increment cycle, a task queue, and explicit stop conditions. It does not
replace that section; the proceed/stop lists there remain in force and are widened below.

---

## The standing default: keep going

**Proceeding on an obvious increment is the default and does not need to be re-issued.**
When the next action is clear and the loop is green, the agent implements the next
increment without asking. The operator does not have to say "continue" each time, and
operator silence means continue. The loop surfaces to the operator **only** at the
decision points in the Stop list below. Everything else is the agent's to carry.

This inverts the usual gate: rather than proceed-on-approval, the loop is
proceed-until-a-real-fork. The forks are few and enumerated, so most of the frontier is
crossed without interruption.

## What the loop consumes (the task queue)

The frontier is the self-hosted-subset boundary, pinned by the
`self_hosted_construct_support_boundary` characterization test in
`tests/selfhost_codegen.rs` (**79 Ok / 4 Gap / 1 RefRejects**, 84 cases, recounted 2026-08-09 —
recount with a grep rather than trusting this number; it has now been found stale twice, by 2 on
2026-07-30 and by 12 on 2026-08-09). A successful increment moves a Gap to Ok or adds a new `SOk`
case.

**Recount it with the case table's line range, not a whole-file grep.** The file contains the words
`Gap` and `RefRejects` in prose, and one comment inside the table itself reads "This is a Gap by
design", so a naive count over the file reports one Gap too many. The table runs from the
`let cases:` binding to its closing `];`.

**A boundary `Gap` entry is not the only source of work, and the count is a lagging indicator.**
Most of the real queue is constructs that are simply absent from the case list. The measured
queue as of 2026-07-31 (each probed against the reference with a control):

- `for … limit … on { ok => …, break(bi) => …, limit => … }` outcome arms (a bare `break;`
  already self-hosts; only the outcome-arm lowering and its index binding diverge)
- `struct { t: (P, Word) }` — a struct field that is a tuple containing a struct. NOTE the
  admission already ADMITS this and the drain then gets it wrong, which is the silent-wrong-output
  class the 3-level struct increment was caught by; treat it as higher-risk than its size suggests
- enum array payload; enum deep-struct payload; enum→struct→enum
- array-of-array nested in a struct; array-of-deep-struct; array of tuple-of-struct
- `struct { i: I }` where `I` holds an enum, and the same where `I` holds an array

**Support does NOT generalize to the enclosing-composite form.** Array-of-array is supported but
array-of-array inside a struct is not; an enum tuple payload is supported but an enum array payload
is not. Never infer support by analogy — probe it.

Beyond the subset frontier, the Order-1 gate (`V0_2_X_ROADMAP.md`) needs exactly three things: the
type checker, the monomorphizer, and wire-format serialization. These are bounded roadmap tasks, so
choosing among them is the loop's call, not the operator's.

Floats and generics are the deferred tail (harder, out of the near-term queue). The
authoritative next-step detail — the reference lowering, the `.kel` touch points, and the
known traps — lives in the frontier assessments in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md). Read the **newest** assessment before selecting;
the current one is the 2026-07-25 post-P11 re-scout, which names tuple-of-struct as the
smallest bounded increment and separates what P11 freed (inter-stage record/node kinds, now
native tags) from what it did not (the runtime wire-op space, still minimal, so the nested
extract reuses op 53 rather than adding an opcode). Do not re-derive what a current
assessment has already scouted, but treat any recipe predating a major encoding or ISA change
as leads to re-validate in the plan step, not as final plans.

## Choosing the next task (no operator prompt for roadmap ordering)

When an increment finishes and the loop must pick the next task, and **every candidate is
already on the roadmap**, the loop chooses on its own — it does NOT prompt the operator to
direct by priority. Order the candidates:

1. **Minimize context switching first.** Prefer a task in the same area as the just-finished
   work — the same stage machinery, the same files, the same fresh understanding — over one
   that changes context. Doing several increments in one area before switching is cheaper and
   less error-prone than jumping between workstreams.
2. **Then by priority.** Among tasks of comparable context cost, take the higher-priority or
   higher-value one.

Switch to a different workstream only when the current area is exhausted — its remaining
candidates are all genuine stops (unbounded, or needing a design decision) or intentional
defers. A choice among bounded roadmap tasks is the loop's to make; the operator prompt is
reserved for the genuine decisions in the stop list below, not for roadmap task ordering.

## The increment cycle (the loop body)

One pass over one gap:

1. **Select** the next task by the ordering policy above (context-switching first, then
   priority) from the DESIGN_JOURNAL frontier notes; among same-context gaps, the smallest
   bounded one.
1a. **PROBE BEFORE PLANNING, ALWAYS WITH A CONTROL.** Before writing any stage code, compare the
   self-hosted pipeline against the reference on the target construct. Then point the SAME probe at
   a known Gap (`scope/float_arith__GAP`) and confirm it reports DIVERGE. Without that control a
   false "identical" is indistinguishable from a real one, because `self_host_compile` builds on
   `compile_src(src)` and replaces chunk bodies — a skipped replacement would report identity
   trivially. Also confirm the REFERENCE accepts the source (`compile_src` alone): a reference
   rejection is NOT a self-host gap, and bad probe syntax reads like one (the language has no
   `let mut`, and a `for` needs `limit` — take valid syntax from `tests/for_limit.rs`).

   This step is not optional bookkeeping. On 2026-07-30 an increment was authorized to implement
   tuple-in-tuple across three `.kel` stages; a probe showed it already worked. On 2026-07-31 four
   of six Order-1 residuals turned out already closed. **A recorded status claim is a lead, not a
   fact** — including one in this document. Equally, a conservative ADMISSION deferral is not
   evidence of a gap: the path it defers to may already be correct.

2. **Plan the lowering**: how the Rust reference compiler lowers the construct, and the
   minimal `.kel` changes to reproduce it byte-identically — typically a detector in
   `parse.kel`, routing in `reconstruct.kel`, and emission in `codegen.kel`. Reuse
   existing tags, opcodes, and node kinds. Do **not** add an opcode, record/node kind,
   wire-format field, or `BYTECODE_VERSION` bump — that is a Stop (see below).
3. **Implement** on a short-lived feature branch cut from `v0.2.3`.
4. **Verify (inner loop)**: `KEL_SELFHOST_CACHE=1 scripts/fast-check.sh 'test(<the new
   construct test>)'`. The construct self-compiles byte-identically, the whole-stage
   self-compiles stay byte-identical, and the boundary test moves the gap Ok. The
   memoized fast lane makes this seconds-scale on unchanged stages.
5. **Record** on all three channels: append the increment reasoning to `DESIGN_JOURNAL.md`
   (what, why, the byte-identity finding, the gotchas), overwrite `REVERSE_PROMPT.md` with
   the bounded latest state and the next intended increment, and update `TASKLOG.md`. The
   handoff prompt [`HANDOFF.md`](./HANDOFF.md) is separate: it is not written every increment,
   only before a planned compaction, and it is stamped with the current commit (see Guardrails).
6. **Commit** with a scoped conventional message ending in the
   `Co-Authored-By: Claude ...` line.
7. **Merge at a natural point**: when the full gate (`scripts/release-gate.sh`) is green, merge the
   **GATED COMMIT** into `v0.2.3` with a **no-fast-forward merge commit** (`git merge --no-ff
   <gated-commit>`), push, and confirm CI is green. The green local gate authorizes the merge; CI is
   the binding authority afterward, so a red CI result is remedied immediately (see
   [GIT_STRATEGY.md](./GIT_STRATEGY.md#definition-of-green)). The no-ff merge keeps the `v0.2.3`
   first-parent history green while preserving the per-increment commits on the merged bubble.

   **DO NOT REBASE BEFORE MERGING.** This step used to say to rebase onto the current `v0.2.3` tip
   first, "so it stays fast-forwardable". That instruction is **wrong and was acted on once before
   the conflict was noticed** (2026-08-10):

   - **A rebase rewrites every commit hash on the branch, including the gated one.** The commit the
     gate ran against then no longer exists, so the green result describes nothing that is being
     merged. That directly contradicts the rule this document and `HANDOFF.md` both insist on —
     *gate the tip you intend to merge*.
   - **The conflict is not rare, it is the normal case.** A gate takes about 2h33m and the version
     branch routinely gains a docs or mailbox commit in that window, which is exactly when the old
     instruction said to rebase.
   - **Fast-forwardability is not required.** `--no-ff` creates a merge commit regardless, so the
     stated reason for rebasing did not hold even on its own terms.

   **The branch tip is usually NOT the gated commit**, because the worktree gate deliberately leaves
   the main tree free and work continues during the run. Merge the gated commit by name, then rebase
   whatever remains onto the new `v0.2.3` and gate that separately. Note this makes the gate result
   valid for a commit whose *merge* still adds any commits `v0.2.3` gained meanwhile; that residual
   is what CI covers, and it is why CI is the binding authority rather than the local gate.

   **A rebased line is the exception**, and it is already covered: `v0.3.0` lands feature branches
   as one green squashed commit. See [GIT_STRATEGY.md](./GIT_STRATEGY.md).
8. **Continue** to step 1 for the next gap. This is the keep-going default; no operator
   prompt is required to start the next increment.

## The hard signal

The byte-identical differential oracle is the correctness gate. An increment is correct
if and only if the self-hosted stage output stays byte-for-byte identical to the Rust
reference **and** the boundary count moves as intended. A red oracle is a hard stop for
that increment. Never weaken an assertion, relax the boundary test, or edit the oracle to
make an increment pass — that defeats the one signal the whole loop trusts.

## Stop and consult the operator

Beyond the PROCESS_STRATEGY triggers (a semantics change, a significant tradeoff needing
human judgment, an unclear assumption, an approaching token limit), the loop **stops** and
writes the question into `REVERSE_PROMPT.md` when:

- **A new opcode, record/node kind, wire-format field, or `BYTECODE_VERSION` bump would be
  needed.** The rad-hard minimal ISA is a high-priority constraint and opcode-count growth
  is an operator decision. Prefer tag reuse plus module-side tables and a build-record with
  a high node kind, the pattern array-of-enum used. If genuinely impossible, escalate with
  the options.
- **The oracle diverges and the branch is NOT CONVERGING toward green.** The criterion is the
  TRAJECTORY, not an attempt count or a commit count. A hard increment may sit red for many
  commits and still be healthy: `v0.2.3` only ever sees green merges, so a long red feature branch
  costs nothing but itself, and there is no virtue in rushing it to green or in abandoning it early.

  **Converging** (keep going, however long it takes): each step shrinks or relocates the divergence
  to a narrower place; the first differing op moves later; regression fixtures that were passing
  keep passing; a newly surfaced unknown is smaller than the one it replaced; the remaining work is
  describable in concrete terms.

  **Not converging** (stop, and abandon or re-approach): the divergence does not narrow across
  successive changes; fixing one fixture reliably breaks another that had been green; each answer
  reveals an unknown as large as the last; or the approach has been shown structurally unable to
  express what the reference emits — a design fault no amount of iteration fixes.

  When not converging, record the first differing op or offset and the structural reason, then
  delete the branch unmerged and re-cut for a different approach, or surface it to the operator.
  Never force an unsound approach to green, and never weaken the oracle or a fixture to get there
  (see [GIT_STRATEGY.md](./GIT_STRATEGY.md#feature-branches)).
- **The full gate goes red for a reason not attributable to the current increment**
  (pre-existing breakage), or the change would touch the shared inter-stage protocol or the
  runtime wire format, which couples stages and must be a single coordinated change.
- **No remaining candidate is a bounded roadmap task** — every option needs a genuine design
  decision (not merely deep or high-effort work) or is off-roadmap. A mere choice among
  bounded roadmap tasks is NOT a stop: order them by the policy above (context first, then
  priority) and proceed without prompting. Surface only when the choice itself requires
  operator judgment (a semantics change, a real tradeoff, or an off-roadmap direction).

  **None of the following is a fork, and none licenses a stop** (each has been used as a
  rationalization and each is wrong):
  - *"The candidates differ by an order of magnitude in cost."* Cost asymmetry is an ORDERING
    input, not a decision point. Take the one that fits the remaining budget and say so.
  - *"This one is multi-session / wants a dedicated run at the budget."* Then start it and
    checkpoint at the budget stop; a large task is entered incrementally, not pre-authorized.
  - *"All the work has to happen eventually, so which does the operator want first?"* If every
    candidate must be done anyway, the order is the loop's to choose — that is precisely the
    case the ordering policy exists to settle, so asking is pure deferral.
  - *"The cheap work is exhausted."* Exhausting the cheap work is not a stop condition. Only an
    exhausted FRONTIER is (see below), and cheap-work exhaustion is the normal state of a loop
    that is making progress.

  The test is not "is this choice significant?" but "does this choice require information only
  the operator holds?" Effort, risk, and sequencing are the loop's to weigh; semantics,
  tradeoffs the operator must own, and off-roadmap direction are not.
- **An irreversible or outward-facing action would be required** — a crates.io publish, a
  force-push of a shared line, a tag. Confirm first; a prior "keep going" does not license
  these.
- **The frontier is exhausted** — the Gap count reaches the deferred set (floats, generics)
  or zero. Report completion and hand back.
- **A per-run iteration or token budget is reached.** Checkpoint the three channels and
  stop cleanly so the next session resumes.

## Guardrails (always in force)

- Feature branches are cut from and merged to `v0.2.3` (via a no-fast-forward merge commit),
  never `main`; merge only after a green full gate; CI must be green after every push and a red
  result remedied immediately. Feature-branch intermediate commits may be red, but the branch tip
  must be green at merge. The full branch model is in [GIT_STRATEGY.md](./GIT_STRATEGY.md).
- Direct commits to `v0.2.3` are allowed **only** for small green documentation or process-file
  changes (the three resume channels, a plan doc); every code change flows through a feature branch.
- Never bypass the pre-push gate (`--no-verify` is prohibited).
- No new opcode or `BYTECODE_VERSION` bump without operator authorization.
- Commit messages end with the `Co-Authored-By` line.
- Confirm before any irreversible or outward-facing action.
- Update `DESIGN_JOURNAL.md` (append), `REVERSE_PROMPT.md` (overwrite), and `TASKLOG.md`
  every increment, so the loop is always resumable from a cold start.
- Write/overwrite [`HANDOFF.md`](./HANDOFF.md) before a planned compaction — the self-contained
  resume prompt, stamped with the current commit as its parent (commit the handoff last, so its
  parent is exactly that commit). It is not kept always-current. On resume, validate it by comparing
  its recorded parent commit to `git rev-parse HEAD~1`; on a mismatch, report it invalid-and-stale to
  the operator and familiarize from the live channels instead, rather than trusting it. A resume
  familiarizes and reports first; the keep-going default is for an active session, not a cold resume.

## Running it

- **Human-in-the-loop**: the operator issues one "continue" (or nothing, given the standing
  default). The agent runs increments back to back and surfaces only at a Stop point.
- **Self-paced**: drive via the `/loop` skill. Between increments schedule the next
  wake-up with a long fallback interval; each firing re-enters the cycle. The loop is
  idempotent — `REVERSE_PROMPT.md`, the boundary test counts, and the git state are the
  resume anchors, so a fresh session picks up mid-frontier without loss.
- **Parallel**: the self-host frontier is largely one stream because the `.kel` stages are
  lockstep, so run it as a single loop and parallelize only disjoint construct areas or the
  independent crates, per [PARALLEL_DEVELOPMENT.md](./PARALLEL_DEVELOPMENT.md).

## Exit

Terminal success is the self-compiling fixed point extended across the reachable frontier:
the boundary test's Gap count reduced to the deferred tail (floats, generics), each closure
byte-identical and CI-green, with the three channels current. At that point the loop reports
and returns control to the operator for the next milestone.
