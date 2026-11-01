# Order 4 completion audit

> **Navigation** [Hosted runtime plan](./ORDER4_HOSTED_RUNTIME_PLAN.md) | [Order 3 audit](./ORDER3_COMPLETION_AUDIT.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md)


**Accepted by the operator first-hand on 2026-10-28.** The operator accepted the Order 4
first-pass gate as recorded in this audit.
## Scope and disposition

This audit records the state of Order 4, the first pass of the hosted runtime,
on 2026-10-25. Its scope is the gate of the hosted runtime plan, which follows
the two operator rulings given first-hand in the V0.2.X session that day.

**Disposition.** Order 4 is complete for its first-pass scope on 2026-10-25, on
the local and remote evidence recorded below. The residual limits below are part
of the disposition.

The hosted runtime is `src/hosted/interp.kel`, a bytecode interpreter written in
Keleusma that runs on the Rust virtual machine. Its driver is
`src/hosted/mod.rs`, behind the off-by-default `self-host` feature, and its
harness is `tests/hosted_runtime.rs`. `BYTECODE_VERSION` remains 2 and the
instruction set remains 66 opcodes. No capacity was raised and nothing was
published.

## Operator rulings applied

1. **Gate.** The interpreter runs the modules the self-hosted compiler emits for
   a corpus of subset programs, including Stream, Yield and Reset, and its
   results match the Rust virtual machine on every case. Running the self-hosted
   compiler's own stage modules under the interpreter is not part of the first
   pass.
2. **Bound composition**, roadmap open decision 2. The interpreter is a `loop`
   that executes at most a fixed number of interpreted instructions per tick, so
   its per-tick bounds are constants. A module is admitted only when its
   verified bounds fit, and its verified time bound bounds the ticks.

A third ruling arose during the work. The instruction count found the verifier's
time bound short of its own cost model, and the operator ruled first-hand that it
be fixed before Order 4 continued. That fix is pull request 519, recorded in
[WCET_STRUCTURAL_MARKERS.md](./WCET_STRUCTURAL_MARKERS.md).

## The bound composition

The resolution of open decision 2 is the following statement.

- **The interpreter's own bounds are constants.** Each tick runs at most 32
  interpreted instructions. The tick's time bound is the transitive bound of the
  interpreter's `loop main`, 55,824,637 nominal cycles, and its memory is fixed
  by its declared tables, so the Rust verifier admits it as an ordinary bounded
  program.
- **An admitted module's work is bounded.** Every instruction costs at least one
  nominal cycle, so a module's verified nominal time bound for one call, or for
  one stream iteration, bounds the instructions it executes. Admission computes
  that bound transitively across calls, on analysis copies in which every call
  is charged its callee's bound by the verifier's own path walk.
- **The composition.** A call or a stream iteration of an admitted module with
  bound `W` takes at most the ceiling of `W` over 32 ticks, plus the tick in
  which it ends and one per yield, and each tick costs at most the interpreter's
  per-tick bound.
- **Memory.** The interpreter's memory bound is constant because its stack,
  frames, heap and data tables have fixed capacities. Admission refuses a module
  whose tables do not fit, and exhausting a capacity while running is a distinct
  fault status, never an interpreted trap.

## Gate evidence

| Gate condition | Evidence | Disposition |
|---|---|---|
| The interpreter is a Keleusma program the Rust verifier admits, with constant per-tick bounds | `the_interpreter_is_admitted_by_the_rust_verifier` builds the interpreter's virtual machine and computes its transitive tick bound, 55,824,637 cycles, against a shallow bound of 40 | Met |
| The corpus is the 107 boundary cases plus a supplement, each compiled by the shipping self-hosted compiler | `tests/hosted_runtime/corpus.rs` copies the 107 `Ok` boundary cases at `ec536b28`, adds ten call programs and eight stream programs, and the harness compiles every one through `keleusma::selfhost::self_hosted_compile`, which errors on any divergence from the reference | Met. All 125 compile. |
| Outcome and value agree with the Rust virtual machine on every case | `the_interpreter_agrees_with_the_virtual_machine_on_the_corpus` ran 1066 argument vectors over 116 admitted cases, 1034 finishing and 28 trapping, with 638 composite-argument runs, 8 composite results, 20 data-segment runs and 75 runs across a call. `the_interpreter_agrees_with_the_virtual_machine_on_streams` drove all nine stream cases through 61 traces and 541 events, 307 yields including 35 composite yields, 233 resets and 1 trap | Met, with the one refusal below |
| Ticks and instructions stay within the computed bounds | Both harnesses check every run against the instruction bound and the tick bound, and the stream harness checks the largest iteration against the per-iteration bound. `a_callee_body_is_inside_the_instruction_bound` shows a run exceeding the shallow bound and staying within the transitive one | Met |
| Refusal before execution, and a fault distinct from a trap | `a_module_with_an_unsupported_instruction_is_refused`, `a_cyclic_call_graph_is_refused_rather_than_bounded` and `a_fault_is_reported_and_is_not_a_trap` | Met |

**The one refused case.** `ctrl/for_bare` has a loop with no static iteration
bound. Admission refuses it for lack of a verified time bound, and the Rust
virtual machine's own `Vm::new` refuses it for the same reason, so it is not a
runnable program on either runtime.

**Non-vacuity.** Every count above is a floor asserted by the harness, so a
change that stopped reaching composites, data, calls, yields or resets would fail
the harness rather than pass it with fewer cases.

## Defects found and repaired

1. **The verified time bound fell short of its cost model**, by up to two cycles
   per `if` and one per loop pass, plus each counted loop's final header pass.
   Found by the instruction count and repaired in pull request 519.
2. **The type checker never checked a yield, and the flat Text decode trusted its
   words.** Reported by the pykeleusma line and repaired in pull request 517
   before Order 4 began. The interpreter's composite yields rely on the first
   repair.

## Residual limits

- **Input.** The host decodes the verified module and marshals it into the
  interpreter's tables. Decoding wire bytes in Keleusma is outside the first
  pass.
- **Admission is host-side.** The interpreter trusts its tables, so a fault
  names a defect in admission or in the interpreter.
- **The supported subset.** Native calls, Text operations, Fixed and float
  arithmetic, boxed composites, shared data, private composite data, composite
  resume values and flat scalar element kinds other than Word and Byte are
  refused at admission.
- **The interpreter is compiled by the reference compiler.** The self-hosted
  compiler refuses `interp.kel`, because its tuple `let` destructuring is
  outside that compiler's subset.
- **The tick bound is loose.** Composite copies and fills are charged at their
  static caps of 256 bytes and 64 values.
- **The corpus is a copy.** A case added to the boundary corpus later is not
  added here.
- **The self-hosted stages are not run under the interpreter**, by the gate
  ruling.

## Verification receipts

A full local matrix ran between 10:08 and 11:18 UTC on 2026-10-25 on the tree of
feat/v02-order4-hosted-runtime rebased onto v0.2.3 at c2733d0a. Every step used
nextest in the Cargo release profile with `--no-fail-fast`, except formatting,
Clippy and the documentation builds.

| Command | Result |
|---|---|
| `cargo fmt --all --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0, with Checking output |
| `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` | Exit 0, with Checking output |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | Failed on three intra-doc links in src/hosted/mod.rs, repaired, then exit 0 |
| `cargo nextest run --cargo-profile release --workspace` | Exit 0, 3524 passed, 2 skipped |
| `cargo test --release --workspace --doc` | Exit 0, 12 passed, 3 ignored, over six summaries |
| `cargo nextest run --cargo-profile release -p keleusma --features self-host` | Exit 0, 3264 passed, 2 skipped |
| `cargo nextest run --cargo-profile release -p keleusma --no-default-features` | Exit 0, 338 passed |
| `cargo nextest run --cargo-profile release -p keleusma --features signatures` | Exit 0, 2504 passed |
| `cargo nextest run --cargo-profile release -p keleusma --features signatures,shell` | Exit 0, 2521 passed |
| In compiler/, `cargo fmt --all -- --check`, deny-warning `cargo clippy --all-targets` and `cargo test --release` | Exit 0, 86 passed |

The documentation repair changed only comments in src/hosted/mod.rs, and it was
made while the self-host step was running. The steps before and during it ran
binaries built from a tree that differs from the final one only in those comments.
The process records were written after the matrix, and the guards that read them
were re-run on their final text. Miri was not run, because this work adds no
unsafe code.

Remote continuous integration ran pull request 521 at 7aeb0188 as workflow run
[40188772558](https://github.com/sgeos/keleusma/actions/runs/40188772558), with
Book run 40188772535, on 2026-10-25. Every job passed and only the Pages
deployment was skipped. The job logs show all eight hosted-runtime tests passing
in the Test job and all eight across the self-host shards. The pull request
merged into v0.2.3 as 7d4bc434.

| Job | Nextest summary |
|---|---|
| Test, the workspace without the self-host binaries | 2665 passed |
| Test, self-host feature, shards 1 to 4 | 803, 799, 780 and 882 passed, 3264 in total |
| Test, no default features | 338 passed |
| Test, signatures feature | 2504 passed |
| Test, broad features | 2521 passed |
| Test, keleusma-wire and keleusma-bench | 53 and 19, and 6 passed |
