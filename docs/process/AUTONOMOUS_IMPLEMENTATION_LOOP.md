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
`tests/selfhost_codegen.rs` (currently **47 Ok / 7 Gap / 1 RefRejects**). A successful
increment moves one Gap to Ok. The near-term queue, smallest-bounded first, is the
nested-composite-equality family:

- tuple-of-struct (`(P, W) == (P, W)`)
- enum-in-struct (`struct { e: E, w: W }`)
- 2+-level nesting (`struct O { m: M }` where `M` has a composite field)
- struct-of-array-of-struct, enum-struct-payload

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
   the bounded latest state and the next intended increment, and update `TASKLOG.md`.
6. **Commit** with a scoped conventional message ending in the
   `Co-Authored-By: Claude ...` line.
7. **Merge at a natural point**: when the full gate (`scripts/release-gate.sh`) is green,
   fast-forward the branch into `v0.2.3`, push, and confirm CI is green.
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
- **The oracle diverges and two or three bounded attempts do not resolve it.** Record the
  first differing op or offset and stop, rather than thrashing.
- **The full gate goes red for a reason not attributable to the current increment**
  (pre-existing breakage), or the change would touch the shared inter-stage protocol or the
  runtime wire format, which couples stages and must be a single coordinated change.
- **No remaining candidate is a bounded roadmap task** — every option needs a genuine design
  decision (not merely deep or high-effort work) or is off-roadmap. A mere choice among
  bounded roadmap tasks is NOT a stop: order them by the policy above (context first, then
  priority) and proceed without prompting. Surface only when the choice itself requires
  operator judgment (a semantics change, a real tradeoff, or an off-roadmap direction).
- **An irreversible or outward-facing action would be required** — a crates.io publish, a
  force-push of a shared line, a tag. Confirm first; a prior "keep going" does not license
  these.
- **The frontier is exhausted** — the Gap count reaches the deferred set (floats, generics)
  or zero. Report completion and hand back.
- **A per-run iteration or token budget is reached.** Checkpoint the three channels and
  stop cleanly so the next session resumes.

## Guardrails (always in force)

- Feature branches are cut from and merged to `v0.2.3`, never `main`; merge only after a
  green full gate; CI must be green after every push.
- Never bypass the pre-push gate (`--no-verify` is prohibited).
- No new opcode or `BYTECODE_VERSION` bump without operator authorization.
- Commit messages end with the `Co-Authored-By` line.
- Confirm before any irreversible or outward-facing action.
- Update `DESIGN_JOURNAL.md` (append), `REVERSE_PROMPT.md` (overwrite), and `TASKLOG.md`
  every increment, so the loop is always resumable from a cold start.

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
