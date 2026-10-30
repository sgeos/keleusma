# Order 4 completion, Workstreams E and F

> **Navigation** [Decisions](./README.md) | [Plan](./ORDER_4_PLAN.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

The roadmap gives Order 4 the gate "WCET and WCMU bounds carried onto native;
the total-opcode-plus-`Trap` ISA lowered". The requirements are those of
[`ORDER_4_PLAN.md`](./ORDER_4_PLAN.md), which also records the two premises
this work takes. Native WCET is treated as best effort in bytecode cost units,
because roadmap open decision 1 is the operator's. An unhandled partial
operation traps natively, following the roadmap and the operator ruling of
2026-10-20 rather than the default values in
[`RUNTIME_FAULTS.md`](../spec/RUNTIME_FAULTS.md).

## The gate

**Met on 2026-10-25, with one bound carried in part.** Each requirement below
has executable evidence. The cost claim is established for twelve of the thirty
coroutine-shaped corpus entries and not for the self-hosted lexer, for a reason
recorded under the residual limits.

## Workstream F

| Id | Requirement | Executable evidence | Disposition |
|---|---|---|---|
| F1 | A native trap in a coroutine library reaches the host as an outcome | [`native_trap_outcomes.rs`](../../native_codegen/tests/native_trap_outcomes.rs) drives ten subjects covering five trap kinds, zero divisor, index out of bounds, loop limit, refinement failure and unmapped discriminant, in the entry, in ordinary callees, two callees deep, in a delegated callee and in a stream. Each reports status three with the kind the VM's error names. The instance is inactive afterwards and its slot starts again normally. The same programs on other inputs do not trap | Implemented |
| F2 | A host native can fail softly | [`native_host_failure.rs`](../../native_codegen/tests/native_host_failure.rs). A native named in `HostContracts::fallible_natives` returns a value and a failure word. Handled and bare failures, in the entry and in a callee, agree with the VM. A C host defines the native from the generated header and agrees with the VM on both inputs | Implemented for the AAPCS64 and System V x86-64 return conventions, of which the first is executed |
| F3 | Each first-pass total operation produces its flag, and a handled outcome is a branch | [`handled_partial_outcomes.rs`](../../native_codegen/tests/handled_partial_outcomes.rs) runs nine handled subjects natively on failing and passing inputs, optimised and not. Division, remainder, division overflow, the three checked arithmetic operators, array indexing and indexed private data reads and writes all agree with the VM | Implemented for Word operands. Byte, Fixed and shared data reads are covered by earlier files |
| F4 | The superseded designs are recorded | `OverflowPolicy::Trap` and the specification's native default values are marked superseded in the code and in `partial_operation_census.rs`. The disagreement with `RUNTIME_FAULTS.md` was reported to the V0.2.X line, which owns the specification | Done |

The trap mechanism is small. Each function in a coroutine keeps a trap-code
cell. A `Trap` writes its kind there and branches to the trap block. In the
entry the trap block writes the code plus one into a trap record in the
caller's frame and ends the instance through the coroutine's single cleanup. In
a callee it writes the code into its call context and returns, and the caller
tests the context after the call. The stable handles read the record and report
status three. The record is the aligned word that `coroutine::trap_offset`
names, below any dialogue metadata.

## Workstream E

| Id | Requirement | Executable evidence | Disposition |
|---|---|---|---|
| E1 | Native code executes the bytecode's cost path | [`native_cost_path.rs`](../../native_codegen/tests/native_cost_path.rs). `LowerOptions::op_meter` counts each bytecode instruction natively, and VM breakpoints count the same per host step. On the lexer over 40 turns for two sources, and on four smaller subjects including a delegated suspension, every step's counts are equal for every instruction | Implemented |
| E2 | The bytecode WCET claim is carried onto the library | `keleusma_native::cost` composes the verifier's per-chunk bound through calls, callees first. Libraries export `<prefix>_wcet_cost` and `<prefix>_wcet_status`. Every traced step of the subjects with a claim stays within it, and each claim equals its worst traced step. A census over the corpus establishes 12 of 30 claims. A module without a claim exports status zero and cost zero | Implemented, best effort as native time, and not established for the lexer |
| E3 | The machine stack is bounded | `keleusma_native::stack` reads every function's final frame size from the code generator's diagnostics and takes the longest path through the emitted call graph. Libraries export `<prefix>_stack_bytes`. The Rust lexer host paints its stack and measures 592 bytes against an exported bound of 656. [`machine_stack_bound.rs`](../../native_codegen/tests/machine_stack_bound.rs) shows the bound rising with call depth and lying between the largest frame and the sum of all frames. Writing the object refuses if its frames bound a different figure | Implemented, for code the library defines |
| E4 | The arena bounds are carried | The exported frame, slot, shared, private and region figures are those the host provisions, and the lexer host now places guard words after every buffer and refuses to finish if any is written. `bound_transfer.rs` and `arena_high_water.rs` hold the earlier evidence | Implemented, with the region figure the backend's own rather than the verifier's |

## What was found along the way

1. **The verifier's per-chunk cost is shallow, and a module's cost header can
   understate its step by orders of magnitude.** The lexer verifies with a
   header of 252 cost units per step. Its callee `intern_id` holds nested loops
   of up to 1280 by 64 iterations, which the shallow figure charges as one call
   dispatch. Composition is what exposes it. The shallowness is documented in
   `src/verify.rs`. What is new is the measured size of the gap on a shipped
   program.
2. **The verifier's text pass refuses a comparison of two `Word` call results.**
   `intern_id` compares `peek_at(a) != peek_at(b)`, both declared `Word`. The
   whole-chunk analysis reports a text operation of unbounded length at that
   comparison, so the callee has no bound and the lexer has no composed claim.
   This is a conservative refusal, reported to the V0.2.X line, which owns the
   verifier. This line edits neither `src/verify.rs` nor its callers.
   **Amended 2026-10-30.** V0.2.X's pull request 598, absorbed in absorption
   116, adds `wcet_whole_chunk_with_callees`, which knows each callee's text-ness
   from its declared return shape. `native_codegen/src/cost.rs` now uses it for
   every chunk that is not a `Stream` iteration, and the lexer has a composed
   claim of 26669019 units against a traced worst of 1485. The claim is loose
   for the reason item 1 gives.
3. **Code generation rewrites the module it emits.** The first stack bound was
   measured on the same module that was then written, and the self-check in
   `write_object` caught the two disagreeing, 656 against 976 bytes. Both
   measurement and writing now generate a copy.
4. **Converting an operand to `AnyValueEnum` reclassifies a function constant.**
   The first call graph had no edges, so the bound was the start handle's own
   frame. The host measurement caught it at 912 bytes against 176.

## Departures and assumptions

- **The trap payload is not reported.** An indexing trap's index and bound and
  a host failure's code stay inside the instance. The kind is reported.
- **A function lowered outside a coroutine still aborts.** `lower_module` has
  no host interface to report to, so its trap block keeps `llvm.trap`.
- **The stack bound adds sixteen bytes per frame.** This covers a return
  address the reported frame may omit and a realignment at a call. It is an
  assumption about the AAPCS64 and System V conventions, checked by
  measurement only on `aarch64-apple-darwin`.
- **The lexer's frame bound rose from 1216 to 1416 bytes.** The measured
  growth is attributable to this increment, since it is the only change to the
  lowering since Order 3. Each call context gained an eight-byte trap word, and
  contexts live across a delegated suspension are kept in the frame. That
  mechanism fits the figure and was not measured directly.
- **The cost meter is one global.** A metered module serves one instance on
  one thread at a time. The meter is off by default and is not exported.

## Residual limits

- The lexer has no composed cost claim, for the reason in finding 2. Five
  corpus entries saturate the 32-bit figure, eleven call host natives whose
  cost is the host's attestation, and one delegates its suspension.
- No figure is a native time bound. Open decision 1 remains the operator's.
- The stack bound excludes host natives and compiler runtime routines. The
  lexer calls neither.
- One target was executed, `aarch64-apple-darwin`. The fallible-native return
  convention and the stack probe have x86-64 code that has not run.
- The trap kinds `NoMatchingHead`, `NoMatchingArm` and `CheckedArithNoArm`
  have no subject here, and `AssertionFailed` exists only in debug builds.
  Their `Trap` lowers through the same path as the five covered kinds.
- A coroutine reports a contract guard's failure with `coroutine::TRAP_CONTRACT`
  rather than a `TrapKind`, since the bytecode does not establish those
  representation contracts.

## Verification

Run locally on 2026-10-25 with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1`
on the final tree, which did not change during the run. Logs are under ignored
`tmp/order-4/`, and each figure is read from its log's summary lines and its
captured status.

| Check | Result |
|---|---|
| `cargo fmt --check` in `native_codegen/` | status zero |
| `cargo clippy --locked --all-targets -- -D warnings`, default and `narrow-float-32` | status zero in both |
| `cargo doc --locked --no-deps` with `RUSTDOCFLAGS="-D warnings"` | status zero |
| `shellcheck tools/*.sh examples/lexer_host/run.sh` | status zero |
| Native `cargo test --locked --no-fail-fast`, default features | 827 passed, 0 failed, status zero |
| Native `cargo test --locked --no-fail-fast --features narrow-float-32` | 827 passed, 0 failed, status zero |
| Root `--test documentation_links --test comment_citations --test ci_shard_coverage --test gate_ci_correspondence` | 16 passed, status zero |

An earlier run was stopped and discarded because two census files were edited
while its narrow suite was reading the tree. The guards that read this record
and the handoff were rerun after this table was written. These checks do not
run the root workspace suite, the self-host tier or the backend gate's
optimised phase. The hosted continuous integration run on the pull request is
the version-branch integration check.
