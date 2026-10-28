# Order 6 rulings plan, 2026-10-28

> **Navigation** [Decisions](./README.md) | [Order 6 completion](./ORDER_6_COMPLETION.md) | [Decision options](./ORDER_6_DECISION_OPTIONS.md) | [Operator questions](../process/handoffs/v0.3.0-QUESTIONS.md)

The operator answered every open V0.3.X question in session on 2026-10-28.
The answers are quoted with their provenance in
[the question list](../process/handoffs/v0.3.0-QUESTIONS.md). This record
turns each answer into a work item with its owner and status, and it is
updated as each item lands. The operator asked for this line's own items to
land one pull request at a time.

## Work items

| Q | Ruling, in brief | Item | Owner | Status |
|---|---|---|---|---|
| 1 | Best effort, in bytecode cost units | Keep the current claim. No change | This line | Done, no work |
| 2 | Defer the bytecode half of the flat machine | None | None | Deferred |
| 3 | Report the root `scripts/` pipe pattern | Send the finding | This line reports, V0.2.X owns | Open |
| 4 | Apply `opt-level = 1` | Set it in `native_codegen` | This line | Open |
| 5 | Wait for the verifier fix | None on this line | V0.2.X | Waiting |
| 6 | Add `interp.kel` and `frame.kel` to the native corpus | Add the roots and re-pin the censuses they enter | This line | Open |
| 7 | Revise `RUNTIME_FAULTS.md` to the 2026-10-20 ruling | Relay the ruling | V0.2.X | Open |
| 8 | Compare slot types on hot swap | Relay the ruling, then adopt the new hash natively | V0.2.X, then this line | Open |
| 9 | Remove run-time kind tags | Request a definite-initialization rule, then remove the tags | V0.2.X, then this line | Open |
| 10 | Restore two Kaizen records, then delete the branch | Restore them, then coordinate the deletion | This line and Kaizen | Open |
| 11 | Narrow `native.yml` paths | Audit which documents native tests read | This line | Open |
| 12 | A hosted Linux job on two Ubuntu runners | Add the job and the host-triple route in `tier_one_targets.rs` | This line | Open |
| 13 | Literal-only bare `Text`, `Text<N>` for dynamic text, a two-word literal | A literal passes as a pointer and a capacity. `Text<N>` is requested | This line, and V0.2.X for `Text<N>` | Open |
| 14 | A typed host pointer for `Opaque` | Lower it with a distinct C type per declared opaque type | This line | Open |
| 15 | Install the `x86_64` Rust target | Install it and run the Rust host on both architectures | This line | Open |
| 16 | Native output from the `keleusma` command | Propose the shape to V0.2.X, which maintains `keleusma-cli` | This line and V0.2.X | Open |
| 17 | Release when the operator says so | None until told | Operator | Held |
| 18 | The V0.2.X dependency is met | Mark it met in the Order 6 record | This line | Done in this record |
| 19 | A self-hosted compiler compiled to native code and used as the compiler, a V0.4.0 requirement | Plan and build the native pipeline | This line | Open, planned below |

## This line's readings, stated as its own

- **Question 13.** The two and three words are the representation in which
  text passes, not its storage. A literal passes as a pointer and a capacity
  equal to its length. Dynamic text passes as a pointer, a capacity of `N`
  and a length, viewing a `Text<N>` that the authorized design still stores
  flat. Worst-case memory therefore stays static. This was stated to the
  operator in session and is proceeded on unless the operator objects.
- **Question 9.** `Unit` is also a language type, the empty type of zero
  bytes. The ambiguity the tags resolve comes from the virtual machine using
  its `Unit` value variant to mark an empty slot. This was stated to the
  operator in session.

## Question 19, the native self-hosted compiler

**The requirement.** The self-hosted compiler is compiled to native code and
used as the compiler, and everything is checked to work as intended. The
operator states that V0.4.0 is cut tactically and that this is to be assumed a
V0.4.0 requirement.

**What already exists.** All fourteen stage sources under `src/selfhost/kel/`
are native corpus subjects, lowered natively and compared with the virtual
machine one stage at a time with seeded inputs.

**What is missing, at the design level.** The stages are not yet run as a
pipeline natively. The self-hosted driver threads each stage's output into the
next through host code in `src/selfhost/mod.rs` and `src/selfhost_host.rs`,
which runs each stage on the virtual machine. A native pipeline needs the same
threading over native stages, a command that runs it, and a check that its
output equals the reference compiler's on the whole corpus. The plan for it is
written as its own record before it is built.

## Order

The operator delegated sequencing. Items that only relay a ruling are sent
first, because they start V0.2.X's work and cost this line nothing. This
line's items then land one pull request at a time, smallest first: 4 and 15,
then 6, 10, 11, 12, 14 and 13, with 9, 8 and 13's `Text<N>` part following
V0.2.X's changes. Question 19's plan is written alongside, and its build
follows.
