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
| 3 | Report the root `scripts/` pipe pattern | Sent 2026-10-28, sequence 122. V0.2.X accepted it as a defect report | V0.2.X | Reported, repair is V0.2.X's |
| 4 | Apply `opt-level = 1` | Set in `native_codegen/Cargo.toml` with its rationale. No pinned figure moved | This line | Done, in its pull request |
| 5 | Wait for the verifier fix | V0.2.X gave notice on 2026-10-28, sequence 126, that its pull request 598 adds a callee-aware time bound that admits `intern_id`. Once it is absorbed, `native_codegen/src/cost.rs` may call that form | V0.2.X, then this line | Waiting on V0.2.X's merge |
| 6 | Add `interp.kel` and `frame.kel` to the native corpus | Add the roots and re-pin the censuses they enter | This line | Open |
| 7 | Revise `RUNTIME_FAULTS.md` to the 2026-10-20 ruling | Relayed. V0.2.X reports the rewrite in its pull request 598 | V0.2.X | In review there |
| 8 | Compare slot types on hot swap | V0.2.X confirmed with the operator and will change `compute_schema_hash`, then this line adopts it natively | V0.2.X, then this line | Waiting on V0.2.X |
| 9 | Remove run-time kind tags | V0.2.X sent the rule's form on 2026-10-28, sequence 127, and this line replied. A measurement below shows the rule alone removes little | V0.2.X, then this line | Waiting on V0.2.X |
| 10 | Restore two Kaizen records, then delete the branch | Restore them, then coordinate the deletion | This line and Kaizen | Open |
| 11 | Narrow `native.yml` paths | Audit which documents native tests read | This line | Open |
| 12 | A hosted Linux job on two Ubuntu runners | Add the job and the host-triple route in `tier_one_targets.rs` | This line | Open |
| 13 | Literal-only bare `Text`, `Text<N>` for dynamic text, a two-word literal | V0.2.X confirmed with the operator and schedules `Text<N>` with the passing form of a pointer, a capacity and a length. The operator called this a development no-op for this line. A native differential follows `Text<N>` | V0.2.X, then this line | Waiting on V0.2.X |
| 14 | A typed host pointer for `Opaque` | Lower it with a distinct C type per declared opaque type | This line | Open |
| 15 | Install the `x86_64` Rust target | Installed. The Rust host runs on `x86_64` under Rosetta, and its first run found the exported stack bound 32 bytes short, repaired by `noredzone` | This line | Done, pull request 597 at `97f34b1e` |
| 16 | Native output from the `keleusma` command | V0.2.X confirmed with the operator that `keleusma-cli` delegates to `kel-native` when installed, done by V0.2.X | V0.2.X | Waiting on V0.2.X |
| 17 | Release when the operator says so, and V0.4.0 only in the abstract for now | None until told | Operator | Held |
| 18 | The V0.2.X dependency is met | Mark it met in the Order 6 record | This line | Done in this record |
| 19 | A self-hosted compiler compiled to native code and used as the compiler, a V0.4.0 requirement | Plan and build the native pipeline | This line | Open, planned below |

## Watch items

- **A struct marshalling defect reported against 0.2.2, 2026-10-28.** A
  session working on another repository, `sgeos/volaw-daw`, reported to the
  V0.2.X line, and pointed this line at, a defect observed on keleusma 0.2.2.
  A struct the host builds with `Value::struct_value` is packed in the host's
  field order without names and read through the script's declared offsets,
  so a declaration with another field order or other scalar types reads
  reinterpreted bits with no error. It is a peer report and is not verified
  here. The marshalling is in root `src/`, which V0.2.X owns and which this
  line carries unchanged, so the virtual machine embedding here is expected to
  behave alike and to receive V0.2.X's repair by absorption. Whether the native
  embedding, whose host struct layouts come from headers generated from the
  script's own declaration, is affected is open and will be checked when the
  repair lands.

## Question 9, what the empty-slot marker accounts for

Measured on 2026-10-28 with `static_kind_lowering.rs` at `97f34b1e` and an
uncommitted probe, since reverted, that started the coroutine type analysis
with every local and every private `Unit` slot at the empty kind instead of
`Unit`. That is what a definite-initialization rule would license.

| Start | Needing a run-time kind | Static, dynamic and refused coroutines | Ambiguous unread locals |
|---|---|---|---|
| `Unit`, the current analysis | 5995 of 182565 | 18, 11 and 1 | 40962 |
| Locals empty | 5995 | 18, 11 and 1 | 5811 |
| Private `Unit` slots empty | 5994 | 19, 10 and 1 | 40962 |
| Both empty | 5994 | 19, 10 and 1 | 5811 |

The private rule makes one coroutine static, the one that needed a kind
guard. The ten that stay dynamic are `piano_roll_0.kel` to `piano_roll_9.kel`,
each first at instruction 7 of `main`, which compares the result of a verified
native call. Their ambiguity is the native's return kind, which the analysis
does not know. Removing the tags therefore also needs native return kinds,
either from the bytecode or from a host contract. This was reported to V0.2.X
in the reply to sequence 127. The probe models only the analysis. It does not
show that every corpus program passes the rule.

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

**The three compilers.** V0.2.X relayed on 2026-10-28, from its own session
with the operator, that it should be possible to compile with the reference
compiler, the self-hosted interpreted compiler and the self-hosted native
compiler, and that the two lines will probably merge when native code
generation and self-hosting share one executable. This record treats the
three as the targets of question 19. It is a relayed remark, recorded here
with that provenance.

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
