# What needs your decision, and what happens if you say nothing

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**One page, no arguments re-run.** Each case has its own record; this only says what is open, what it
costs, and what I do by default. **Where the underlying record deliberately declined to recommend, so
does this.**

## The measured position

**All remaining capability work on this line is behind a decision only you can take.** Re-derived
2026-08-29, not recalled:

| | |
|---|---|
| corpus modules the backend lowers end to end | **66 of 69** |
| chunks fully lowerable | **1070 of 1074** |
| unlowerable chunks | **4 — and all four sit in the 3 refused modules** |

There is no fourth thing to fix. The three are below.

---

## 1. `Stream` on `13_telemetry_stream.kel` — a soundness obligation

**Record**: [`COMPOSITE_SLOT_REUSE_OBLIGATION.md`](./COMPOSITE_SLOT_REUSE_OBLIGATION.md)

A composite built inside a loop and yielded out gets one offset for the life of the chunk, so a host
holding iteration *n*'s handle reads iteration *n+1*'s bytes — **a silently wrong value, not a `Stale`
error**. The backend refuses the shape today at **zero** cost, because the module is already refused
for `Stream`.

**The trade, which is why it is yours**: discharging it requires the planner to consume a confinement
verdict, and consuming no verdict is exactly why a wrong verdict cannot miscompile today. The option
that would convert the silent wrong value into a `Stale` error edits `src/vm.rs` and the arena, **which
this line may read and must not edit**.

**Default if you say nothing**: the refusal stands, coverage stays where it is, three tripwires fail if
the situation changes.

---

## 2. The float entry ABI — one refused module

**Record**: the float guard routes, in `native_codegen/tests/float_guard_routes.rs`.

The backend has no float representation: no `f64_type`, no float opcode lowered, and an entry ABI of
`i64` where a double belongs. Refusing the module is the guard.

**Default if you say nothing**: unchanged. This is operator-held and I have not touched it.

---

## 3. `lower_module`'s admissibility precondition — documented, not enforced

**Record**: [`BACKEND_ADMISSIBILITY.md`](./BACKEND_ADMISSIBILITY.md)

A module that `verify()` accepts but `Vm::new` **refuses** — no statically extractable bound — is
lowered without complaint, and the code is not memory-safe. **Zero live instances**: 66 lower, 0
unbounded, pinned by a test.

**The open choice**: enforce it inside `lower_module`, which couples a pure lowering function to the
resource analysis and pays that on every call, or leave it documented.

**Default if you say nothing**: documented and pinned, as now.

---

## Not a decision, listed so it is not mistaken for one

**`Len`** is refused because the only construct reaching it produces a loop with no statically
extractable bound, so the program is inadmissible regardless. **Nothing to decide** — see
[`LEN_FLAT_ARRAY_HAZARD.md`](./LEN_FLAT_ARRAY_HAZARD.md) for the separate hazard it exposed, which is
also reported rather than repaired.

---

## What I will do without an answer

Continue on **correctness of what already lowers** rather than on new capability, because that is what
is available: the differential's mutation family, its filters, and the instruments that report figures
to you. **Publication remains HELD** and nothing here is a request to change that.

**A caution about my own recent work**: five defects in a row were in my instruments, all understating
the subjects, and four published figures were corrected. The corpus walk and the mutation probe are now
single shared functions so that class cannot recur silently, but read my figures with that history in
mind.
