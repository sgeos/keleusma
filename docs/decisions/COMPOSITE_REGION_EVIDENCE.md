# Composite region reuse — what the V0.2.3 runtime establishes

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Audience: whoever drafts the proof in `docs/proofs/COMPOSITE_REGION_REUSE.md`.** That document
lives on the `v0.3.0` line. This one lives on `v0.2.3` and is the evidence index for every claim the
proof rests on that concerns the *runtime* rather than the native backend.

Read it with `git show origin/v0.2.3:docs/decisions/COMPOSITE_REGION_EVIDENCE.md` if you are working
on the other line.

## Why this file exists rather than a section in the proof

Two of the proof's load-bearing premises originally came from this line **as prose in a message**,
and one of them had not been measured when it was written. It was correct, but it was a claim about
`src/vm.rs` supported by nothing a reader could run. **A proof resting on an unverified claim is
worth less than no proof.**

Everything below is either (a) executed, with the test named and a command to reproduce it, or (b)
explicitly marked as read from the virtual machine's dispatch rather than run. **Do not promote a row
from (b) to (a) without running it.**

## Ownership — do not edit these from the `v0.3.0` line

| surface | owner |
|---|---|
| `src/verify.rs`, `src/vm.rs`, `src/bytecode.rs`, `src/flat_value.rs` | **v0.2.3** |
| `src/wire_schema.rs`, `src/selfhost/`, `.github/workflows/` | **v0.2.3** |
| `docs/proofs/`, `native_codegen/`, the native backend and its corpus | **v0.3.0** |

**If the proof implies a change to `src/verify.rs`, that is a request to the `v0.2.3` line, not an
edit.** The mailboxes are `docs/process/handoffs/v0.2.3.md` and
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`.

## The established facts, with provenance

Run everything from the repository root on `v0.2.3`.

### 1. A yielded composite is a HANDLE, not a copy — **EXECUTED**

After B28 the only non-empty composite representation is
`FlatComposite::Arena(ArenaHandle<[u8]>)`: a pointer and length into the arena, read through
`resolve`. The host holds a reference to arena bytes.

```sh
cargo test --test composite_escape_window
```

`two_iterations_composites_are_live_together_and_distinct` is the one that carries weight. It shows
two iterations' composites resolving **simultaneously to different values**. A test asserting only
that a held handle still reads its own value passes on a runtime that yields the same value twice.

### 2. The epoch guard does NOT cover an overwrite in place — **EXECUTED**

`resolve` fails `Stale` when a `RESET` has advanced the epoch. **An overwrite at the same address
within the same epoch advances nothing**, so `resolve` succeeds and returns the newer bytes.

**Consequence: slot reuse produces a silent wrong value, not an error.** This is the unfavourable
branch, and it is why the proof's Theorem B could not be stated as "reuse is sound".

`a_yielded_composite_outlives_its_iteration_and_dies_at_reset` asserts both directions — the handle
reads correctly before the reset **and goes `Stale` after it**. Without the second half the test
would pass on a runtime that never invalidates anything, which is a different and worse property.

### 3. The escape window is ONE STREAM CYCLE, not one iteration — **EXECUTED**

`Op::Reset` is emitted once per `loop main` body, **not** once per `for` iteration. A `for` loop
containing a `yield` therefore runs every iteration inside a single epoch.

```
step  state                          epoch   handle held from iteration 1
0     Yielded (iteration 1)          0       1
1     Yielded (iteration 2)          0       1
2     Yielded (straight-line site)   0       1
3     RESET                          1       Stale
```

`reset_is_once_per_stream_cycle_not_once_per_loop_iteration` asserts the op-level fact;
the other two assert the behaviour. **"Bounded by `RESET`" is not actionable without this** — it is
the difference between a window of one iteration and a window of arbitrarily many.

### 4. The escape routes are FIVE, enumerated from the instruction set — **TOTAL, mixed provenance**

```sh
cargo test --test composite_escape_routes
```

`tests/composite_escape_routes.rs` classifies **all 66 opcodes**, with totality asserted against the
`Op` enum read out of `src/bytecode.rs` at test time.

| opcode | provenance |
|---|---|
| `Yield` | **executed** (§1, §2) |
| `SetLocal` | **read from dispatch** |
| `Return` | **read from dispatch** |
| `CallVerifiedNative` | **trust boundary — see below** |
| `CallExternalNative` | **trust boundary — see below** |

**`SetLocal` is the route that breaks the naive restriction, and it needs no host at all:**

```keleusma
let mut last = P { a: 0, b: 0, c: 0 };
for x in xs { last = P { a: x, b: x, c: x }; }
```

The opcode **cannot distinguish a binding declared inside the loop from one declared outside** — that
is a property of the slot the compiler assigned — so it is classified by its worst case. A
restriction phrased as *"loop bodies containing no `yield`"* is **not sufficient**.

**The two native calls are a trust boundary this line cannot close.** A native receives the composite
and what it retains is the host's affair. If a theorem excludes them, that is an obligation
documented on the **embedder**, not a property of the language, and it should say so in those words
rather than be counted safe.

### 5. The two "copies out" routes — **EXECUTED, and deliberately so**

**A wrong `Escapes` makes a restriction loose. A wrong `CopiesOut` makes it UNSOUND.** That asymmetry
is why these two were run rather than read.

| claim | discriminator | test |
|---|---|---|
| a composite written to a **`private data`** slot is copied | it survives two resets that reclaim the region it was built in; a stored handle would fail `Stale` | `a_composite_written_to_private_data_is_copied_not_aliased` |
| nesting into a **flat** composite copies | the parent's 24 bytes are `[11, 22, 33]` — the child's words inline | `nesting_a_composite_into_a_flat_one_copies_its_bytes_inline` |

**`private` was used rather than `shared` on purpose.** A host `&mut [u8]` buffer must copy by
construction, so proving that proves the easy half. The persistent-region path is the one that could
have aliased and does not.

**Stated limit: the BOXED construction path DOES alias.** It stores operands as separate values. It
does not arise for the transitively-scalar composites this proof concerns, but a claim phrased
without that boundary would be false in general.

## What the verifier actually computes, and what a proof would change

`wcmu_region` in `src/verify.rs` is **cumulative, with a maximum only across mutually exclusive
branches**. It is not peak concurrent liveness; an earlier revision of the other line's notes said so
and was retracted.

| construct | site | behaviour |
|---|---|---|
| every allocating op | `heap.saturating_add(...)` | accumulates |
| `If` with both arms | `src/verify.rs:992` | `heap + max(then, else)` |
| **loop body** | `src/verify.rs:1079` and `src/verify.rs:1087` | **`body_heap_one * iter_count`** |

**Adopting Theorem B changes line 1079 and nothing else in shape**: the loop body's contribution
would stop being multiplied by the iteration count where the restriction holds, and revert to
`k * sz` where it does not.

**That LOWERS a published worst-case-memory-usage figure**, which is the crate's headline guarantee
and a changelog-visible weakening. It is a request to the `v0.2.3` line, not an edit, and it needs
the operator rather than either agent.

Worked example, if a reference point helps — the proof's own §4.1 counterexample bounds here at
**stack 320, heap 112**, where `112 = 2*24 + 24 + 24 + 16`. The backend's `sites * sz` gives 88.

## What this line has NOT established, stated so it is not assumed away

1. **That each individual opcode classification is correct.** Totality is mechanical and holds as the
   instruction set changes. The per-opcode verdicts are analysis; three are execution-backed and the
   rest are read from dispatch. **The table is the place to argue a disagreement.**
2. **Anything about the native backend's lowering.** Every measurement here is against the virtual
   machine.
3. **Whether the loop-dominated direction of the planner gap is unsafe today** — the proof's §6.2.
   Untouched from this side.
4. **What a host native does with a composite it is handed.** Not knowable from here.

## Traps specific to this area

- **A `Value` is 32 bytes and carries a handle, not bytes.** Reasoning about "the value" and "the
  region" as the same thing is the error that makes reuse look safe.
- **`data` with no visibility modifier is not `private data`.** The plain form compiles to the
  host-owned shared buffer and needs `call_with_shared`; `private data` uses the persistent arena
  region. A test written against the wrong one proves the easy half.
- **`Op::Reset` ends a stream cycle, not an iteration.** Conflating them collapses the escape window
  by an unbounded factor.
- **Do not zip the parse cursor and record traces** if you end up in the self-hosted stages —
  different sampling rates, and the result looks like data.
- **On macOS `timeout` does not exist**; it is `gtimeout`. A wrong lowering can emit non-terminating
  code, so mutation work here wants a hard timeout.
