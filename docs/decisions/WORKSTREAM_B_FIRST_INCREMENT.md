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
