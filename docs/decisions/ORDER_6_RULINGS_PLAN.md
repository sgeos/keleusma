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
| 4 | Apply `opt-level = 1` | Set in `native_codegen/Cargo.toml` with its rationale. No pinned figure moved | This line | Done, pull request 600 at `a0dd6f7b` |
| 5 | Wait for the verifier fix | V0.2.X gave notice on 2026-10-28, sequence 126, that its pull request 598 adds a callee-aware time bound that admits `intern_id`. Once it is absorbed, `native_codegen/src/cost.rs` may call that form | V0.2.X, then this line | Pull request 598 absorbed in absorption 116. `cost.rs` bounds every chunk that is not a `Stream` iteration with `wcet_whole_chunk_with_callees`, and the self-hosted lexer has a composed claim that its traced steps stay within. Done on 2026-10-30 |
| 6 | Add `interp.kel` and `frame.kel` to the native corpus | `src/hosted` and `src/crypto_frame` are corpus roots in every full root list, and the seven pins that moved were re-derived. `frame.kel` executes in the corpus differential and agrees. `interp.kel` is exempt there, because the virtual machine refuses it with empty `io` tables. A seeded `interp.kel` differential follows as its own pull request | This line | Roots done, pull request 602 at `4a41f53a`. The seeding function arrived in absorption 116 as `hosted::seeded_shared` with the `hosted::protocol` constants. `native_seeded_interp.rs` runs `interp.kel` natively over seeded buffers for five programs and fourteen example scripts, and it agrees with V0.2.X's driver and with the virtual machine. Done on 2026-10-30 |
| 7 | Revise `RUNTIME_FAULTS.md` to the 2026-10-20 ruling | Relayed. V0.2.X rewrote it in its pull request 598 | V0.2.X | Done there, absorbed in absorption 116 |
| 8 | Compare slot types on hot swap | V0.2.X confirmed with the operator and will change `compute_schema_hash`, then this line adopts it natively | V0.2.X, then this line | Done. V0.2.X's typed hash arrived in absorption 119, and `native_hot_swap.rs` shows a slot type change alone refused by both shapes, merged as pull request 653 at `3e532b22`, 2026-10-30 |
| 9 | Remove run-time kind tags | V0.2.X sent the rule's form on 2026-10-28, sequence 127, and this line replied. A measurement below shows the rule alone removes little. V0.2.X holds the rule, sequence 131, because it rejects programs in use, and the choice is open question 20 | Operator, then V0.2.X, then this line | Question 20 answered 2026-10-30, option a, narrow the rule and keep tags where it cannot prove a slot written, relayed in sequence 154. The census was narrowed on 2026-10-29, and absorption 116 freed `interp.kel`, leaving `14_frame_log.kel`. V0.2.X's narrowed rule arrived in absorption 122. It leaves composite private data unchecked, so 1 of 192475 instructions still needs a run-time kind, the private slot guard in `14_frame_log.kel`, which is question 26 |
| 10 | Restore two Kaizen records, then delete the branch | Restored verbatim into `NATIVE_LOWERING_INVENTORY.md` under a provenance note, from `af8b61ac` and `e4964490`. The branch is deleted after the merge and Kaizen is told | This line and Kaizen | Done. Pull request 604 at `3d79be0b`, the branch deleted and Kaizen told |
| 11 | Narrow `native.yml` paths | Audited. Of `docs/decisions`, the suite reads only the newest absorption receipt, so the filter and the reach set name `ABSORPTION_*_BRIEF.md`. The audit also found the citation guard reading sources the workflow does not watch, now limited to the watched paths | This line | Done, pull request 606 |
| 12 | A hosted Linux job on two Ubuntu runners | `tier_one_targets.rs` runs a Linux triple on its own machine with the system `cc` and fails if it did not. `native.yml` gains a job on `ubuntu-24.04` and `ubuntu-24.04-arm` that runs that file and `kel_native_cli.rs`, with LLVM 22 from `apt.llvm.org` | This line | Done in PR 607, both runners green in run 40823596424 |
| 13 | Literal-only bare `Text`, `Text<N>` for dynamic text, a two-word literal | V0.2.X confirmed with the operator and schedules `Text<N>` with the passing form of a pointer, a capacity and a length. The operator called this a development no-op for this line. A native differential follows `Text<N>` | V0.2.X, then this line | `Text<N>` values and the literal cast arrived in absorption 122, with other operations refused and an operator decision pending, question 25. The native differential is this line's next item |
| 14 | A typed host pointer for `Opaque` | Built as planned below. `kel-native` reads the names from the source, the header and bindings type each position, a contract the word cannot carry is refused, and a C host differential agrees with the virtual machine. The struct field and shared slot routes stay refused | This line | Done, in its pull request |
| 15 | Install the `x86_64` Rust target | Installed. The Rust host runs on `x86_64` under Rosetta, and its first run found the exported stack bound 32 bytes short, repaired by `noredzone` | This line | Done, pull request 597 at `97f34b1e` |
| 16 | Native output from the `keleusma` command | V0.2.X confirmed with the operator that `keleusma-cli` delegates to `kel-native` when installed, done by V0.2.X | V0.2.X | Done by V0.2.X as `3098f653`, already in this line's history. `keleusma native` finds `kel-native` and passes every argument through |
| 17 | Release when the operator says so, and V0.4.0 only in the abstract for now | None until told | Operator | Held |
| 18 | The V0.2.X dependency is met | Mark it met in the Order 6 record | This line | Done in this record |
| 19 | A self-hosted compiler compiled to native code and used as the compiler, a V0.4.0 requirement | Planned in [its own record](./NATIVE_SELF_HOSTED_COMPILER_PLAN.md). The first step, an executor boundary in the driver, is requested from V0.2.X | V0.2.X, then this line | Step 1 done by V0.2.X, `StageExecutor` and `with_stage_factory`, absorbed in absorption 118. Step 2 begun on 2026-10-29, a native executor driven through a local adapter, with host natives bound by arity. Step 3 on 2026-10-30, `NativeStageFactory`, compiles byte-identically with native stages, and `frame.kel` runs natively with its closure natives bridged. Stages are cached, and all 69 corpus sources in the subset compile byte-identically with native stages. The selector `kel-native --compiler rust|self-hosted|self-hosted-native` landed on 2026-10-30, and the three build byte-identical libraries. Done |

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

V0.2.X answered in sequence 129 that the module records each native's return
shape. This line checked the site. The native is `host::song_name`, declared
with a bare `use` and no signature, so its recorded shape is `Top`, which is
correct. Its result is discarded by `PopN(1)` at instruction 12, and the
instructions from 7 only test the soft-failure flag. The census flags any
ambiguous operand on the stack, including one no instruction reads, so the
piano roll figure may be this line's over-approximation rather than a missing
kind. That is to be measured on this line before anything is asked of V0.2.X.

Measured on 2026-10-29, it was the over-approximation. No instruction in the
ten coroutines reads the native result's kind. Only `PopN` and the
soft-failure `Trap(HostError)` pop it, and the native code of a trap stores
only its code. The census now counts only the operands an instruction reads,
with `Return` counted as reading its value, and the corpus grew since the
table above, so the four starts were measured again.

| Start | Needing a run-time kind | Static, dynamic and refused coroutines | Ambiguous unread locals |
|---|---|---|---|
| `Unit`, the current analysis | 21 of 188131 | 28, 2 and 2 | 44241 |
| Locals empty | 21 | 28, 2 and 2 | 65 |
| Private `Unit` slots empty | 20 | 29, 1 and 2 | 44241 |
| Both empty | 20 | 29, 1 and 2 | 65 |

Native return kinds are therefore not needed for the piano roll, and the
claim in this line's reply to sequence 127 that they are is withdrawn. The
private rule frees `14_frame_log.kel`. What remains is `interp.kel`, which
moves an operand of more than one kind into a local at chunk 2 instruction 51.

Its cause was identified on 2026-10-29. Chunk 2 is `checked_add_word`, whose
checked addition matches `ok`, `overflow` and `underflow` with unguarded arms
that each build a three-word tuple. `compile_checked` in root `src/compiler.rs`
still ends the match with its wrapping default, `GetLocal` of the low word and
`Break`, so the join at the loop's end holds a tuple or a word. The default
cannot execute, because the outcome's flag is one of the three covered
classes, but the analysis does not track the flag's value. The compiler
already omits the zero-divisor fallback when an unguarded arm covers that
class, and the same omission for a fully covered match would remove the
ambiguity. Chunk 2 is only the first such site. The sibling checked
functions share its source shape and are expected to account for the other
instructions, which was not measured. The compiler is V0.2.X's, and the
request was sent in sequence 136. The newly static piano roll coroutines
call host natives, so `static_kind_lowering.rs` admits them on the coroutine
route without driving them, and their agreement with the virtual machine is
checked through `lower_module` by `module_differential.rs`.

Measured again on 2026-10-30 after absorption 116, which brings V0.2.X's
answer to sequence 136. Both compilers now omit the default and the last
arm's class test when unguarded arms cover every class, so the join holds
only the arms' kind. With the current analysis, 1 of 188375 instructions
needs a run-time kind, and the static, dynamic and refused coroutines are 29,
1 and 2. `interp.kel` is static and lowers with static kinds, as
`an_operand_no_instruction_reads_needs_no_run_time_kind` now requires. The
one dynamic coroutine is `14_frame_log.kel`, at a kind guard over a private
slot that starts as `Unit`, which is the case the private rule of question 20
frees. The three other start rules were not measured again. By the table
above the private rule would leave no instruction needing a run-time kind,
but that is this line's inference and not a measurement.

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

## Question 14, a typed host pointer for `Opaque`

**The starting point, measured on 2026-10-28.** A probe built six small
programs with an opaque type `Device` and lowered each as a library. Four
lowered already: a native's opaque result discarded, passed to another native,
held across a yield, and an opaque parameter. A struct with an opaque field
was refused, its construction having no packed extent, and the shared-slot
probe did not compile. The module records a native's opaque result as scalar
kind 7 and records no opaque type names and no native parameter types. The
header declares every native word as `int64_t`. So the Order 6 record's claim
that opaque handles are refused on every route was wrong, and it is corrected
there.

**The design, this line's own under the ruling.**

1. The names come from the source. `kel-native` already parses the program,
   so it reads each `use` declaration and records which parameter and which
   result names a type the program does not declare, which is what makes a
   type opaque. They reach the library through `HostContracts`, beside the
   fallible natives. A caller with only a module supplies none, and the
   header then keeps `int64_t`, as today.
2. The header declares one incomplete `struct kel_opaque_<Name>` per opaque
   type and uses `struct kel_opaque_<Name> *` in each native prototype. The
   Rust bindings declare a matching zero-sized `#[repr(C)]` type and use a
   raw pointer.
3. Native code carries the pointer as a word and never dereferences it. A
   module whose address width differs from the target's pointer width is
   refused, so the word always holds the whole pointer.
4. A differential executes a library whose C host defines the natives over
   real structures, against the virtual machine holding `Value::Opaque`, on
   the four lowered routes. Yields must agree.
5. The struct field and shared slot routes stay refused, each with its
   reason recorded, unless the probe shows the virtual machine stores an
   opaque value there in a form a pointer can match.

**Built on 2026-10-28.** Steps 1 to 4 are in `native_codegen/src/opaque.rs`,
`HostContracts::opaque_natives`, `linkage.rs` and `kel-native`, with
`native_opaque.rs` holding six tests. The differential was checked to run and
to fail when the host's result moved by one. Step 5 stands as written. The
virtual machine keeps an opaque struct field as a registry index, which a host
pointer does not match, and the shared-slot probe did not compile, so neither
route was opened.

**The trust boundary.** Native code cannot check an opaque pointer. The host
must keep each pointer valid for as long as the instance can hold it, and a
stale or wrong pointer is the host's error, which native code cannot detect.
The header says so beside the declarations.

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

**The plan is now [its own record](./NATIVE_SELF_HOSTED_COMPILER_PLAN.md)**,
written on 2026-10-28 from a reading of the driver. The paragraph below is
the earlier summary it replaces.

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
