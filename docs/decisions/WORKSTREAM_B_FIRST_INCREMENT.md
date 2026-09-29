# Workstream B first increment

**Completed and revalidated on 2026-09-29 against `d9b6bceb` and this increment.**
The requested conditional `yield` shape was already implemented. This increment
strengthens its existing regression and corrects the stale resume instructions.
There is no backend lowering change.

The previous brief was invalid-and-stale for this task. All 150 archive ancestry
anchors passed, the archive stamp was four commits behind, and fetching upstream
left zero unabsorbed V0.2.3 commits. These ancestry facts did not establish the
brief's capability claims. The full-gate reporter returned the expected
`UNVERIFIED` result for changed inputs, rather than a recorded test failure.

## Established behavior

The conditional leaves in `lexer.kel` end their iteration. The existing
conditional-tail transformation follows branch targets and lowers the selected
`yield` to a return. No live continuation must cross that return. Nesting inside
an `if` alone does not require a resumable frame. The historical warning below
that this shape necessarily requires real suspension was incorrect.

The existing regression in
[native_codegen/tests/yield_sequence.rs](../../native_codegen/tests/yield_sequence.rs)
now also drives conditional tails with private state across six ticks. Both the
virtual machine and native code must produce `[15, 2200, 375, 425, 51, 6200]`.
The inputs visit all three leaves and distinguish persistence from reinitialization.
A temporary mutation making tail yields return zero compiled and failed this
sequence assertion. The mutation was removed. An initial mutation attempt did
not compile and supplies no behavioral evidence.

The seeded lexer differential in
[native_codegen/tests/stage_differential.rs](../../native_codegen/tests/stage_differential.rs)
compares 400 yielded values and the shared bytes. Its input spans token classes,
and its existing distinct-value floor excludes the unseeded end-of-source-only
run. This is finite-input evidence, not an exhaustive claim about every lexer path.

The instruction set architecture census still reports 63 of 66 opcodes lowered
over 74 compiled modules. The separate refusal census covers 67 modules and
reports one refusal, `13_telemetry_stream.kel`, for a composite escaping a loop
through a yield. The yield-escape gate is **unshadowed and load-bearing**.
The existing frontier test also retains the suspending-callee refusal.

No opcode, bytecode version, root source or root test changed. No broader stream
capability was added or removed. Complete native gates and the full workspace
suite were not rerun. Historical gate records remain unchanged and stale.

The exact verification receipt is maintained in the
[bounded resume brief](../process/handoffs/v0.3.0-BRIEF.md).

---

## Historical scope and findings, superseded by the result above

# SCOPING THE FIRST WORKSTREAM B INCREMENT — from measured facts, not from the roadmap alone

**Written 2026-09-27.** Milestones 2 through 6 are untouched. `STREAM_FRONTIER_BRIEF.md` records
that general `Stream` lowering is **"not in one increment"**, so this scopes a first one rather
than attempting it. Nothing here is implemented.

## What is already measured, and must not be re-derived

| fact | where |
|---|---|
| A single `yield` **in tail position** lowers — including a yielded COMPOSITE. Everything else is refused for `Stream`: yield followed by code, two yields, yield in an `if`, yield in a `for` | the handoff's done-list, eight shapes, zero reference-rejected |
| **Eight of ten** stage `loop` blocks have the tail shape and need **no coroutine intrinsic, no frame, no callback** — `parse`, `analyze`, `reconstruct`, the five `verify_*` | `V0_3_X_ROADMAP.md`, Workstream B |
| Only **two** need this workstream: `codegen` (delegates its body to a multiheaded `yield` callee) and `lexer` (nests its yields inside `if`) | same |
| **12 stages freed by the stream work alone, 0 needing more** — no OTHER unsupported opcode hides behind the refusal | `spike_stream_sufficiency.rs`, measured 2026-09-02 |
| `Op::Stream` is refused **deliberately**, and the backend's coverage residual **depends on that refusal standing** | same section |

## The recommended first increment

**Lower a `yield` nested inside an `if`** — `lexer.kel`'s shape — and nothing else.

**Why this one before `codegen`'s multiheaded delegate.** It is one chunk with no delegation, so
the suspension is local: state crossing the yield belongs to a single body. `codegen` additionally
needs the callee relationship resolved, which is a second problem stacked on the first.

**Why a differential subject already exists.** `lexer.kel` is seeded on real source and compared
native-against-VM in `stage_differential.rs`, and it is one of the two stages `probe_stage_vacuity`
shows producing non-degenerate output (10 distinct token codes against a single repeated `62`
unseeded). So a wrong lowering has somewhere to show up immediately, which the tail-shape work did
not have until seeding existed.

## Wrong turns, each earned

**Do not lift the `Stream` refusal wholesale.** The roadmap states the coverage residual depends on
that refusal standing. Widening it for one shape means the ISA census and the refusal census both
move, and those figures are guarded — so the increment includes re-deriving them, not discovering
afterwards that a guard went red.

**Do not read "freed" as "lowers".** The sufficiency measurement says no other unsupported opcode
sits behind `Stream`; it explicitly does not say the module lowers. That distinction was written
into the roadmap precisely because conflating them "would replace one overclaim with another".

**Do not treat the eight-of-ten split as a bytecode count.** The roadmap marks it a **source-level
reading** of ten `loop` blocks. It also says ten where the tree has **twelve** stage sources — a
count this session already found stale in `stage_differential.rs` — so the denominator wants
re-measuring before it is relied on.

**Do not assume the tail-shape result transfers.** A tail yield was measured to be **no suspension
at all**: the module declares no host yield hook, contains only `kel_chunk_0` and `llvm.trap`, and
RETURNS a pointer into the caller's region. The reference SUSPENDS where the native side RETURNS.
So a yield in an `if` is the first case where a real suspension is required, and none of the
existing evidence covers it.

## What would establish the increment

A yield-in-`if` module that lowers and agrees with the virtual machine over a tick sequence, with
the ISA and refusal censuses re-derived, and the yield-escape gate's shadow status restated —
since the done-list records that refusal as shadowed only because `Stream` is refused first.

---

## THE DENOMINATOR RE-MEASURED, 2026-09-27 — and the conclusion strengthens

The roadmap's split is over **"ten `loop` blocks"**. Measured: **all twelve stage sources contain
exactly one `loop` block**, so the denominator is twelve. Its ten are `parse`, `analyze`,
`reconstruct`, five `verify_*`, `codegen`, `lexer` — but there are **six** `verify_*` files plus
`wire.kel`, so **two stages were unclassified**.

Both are tail-shape, read rather than inferred:

| stage | loop block | callee |
|---|---|---|
| `verify_datalayout.kel` | `loop main(resume: Word) -> Word { yield run() }` | `fn run()`, plain; the file's ONLY yield |
| `wire.kel` | `loop main(cmd: Word) -> Word { yield dispatch(cmd) }` | `fn dispatch()`, plain; its other three "yield" hits are COMMENTS |

**So the corrected split is ten of twelve needing no coroutine intrinsic, and the set needing
Workstream B is unchanged at exactly two — `codegen` and `lexer`.** The roadmap's conclusion is
strengthened; only its denominator was stale.

## ⚠ AND THE CLASSIFIER USED HERE IS INSUFFICIENT IN GENERAL

**Demonstrated by `codegen`, whose answer was already known.** Its loop block is
`loop main(resume: Word) -> Word { yield emit_next(resume) }` — **a single tail yield**. Read by
loop-block shape alone it classifies as needing nothing. The roadmap is right that it needs the
workstream, for a reason **invisible at that level**: `emit_next` is declared
`fn emit_next(resume: Word) -> Word when st.started == 0`, a MULTIHEADED callee.

So "read the loop block" is not a sufficient test, and the two classifications above rest on the
additional check that each callee is a plain `fn` and the file contains no other yield expression.
**Validating the classifier against a case with a known answer is what exposed this**; had it been
run only on the two unknowns, both would have been recorded on an unsound method.

## A forward hazard, from `wire.kel`'s own comment

It notes that `dispatch` "still answers each command in one yield; converting the emit commands to
yield per record is the increment that follows." **That change would move `wire` out of the
tail-shape set and into the coroutine-needing one**, so the ten-of-twelve figure is contingent on
work the other line has already described as next.
