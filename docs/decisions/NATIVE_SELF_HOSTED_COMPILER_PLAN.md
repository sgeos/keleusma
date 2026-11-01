# The native self-hosted compiler, plan, 2026-10-28

> **Navigation** [Decisions](./README.md) | [Rulings plan](./ORDER_6_RULINGS_PLAN.md) | [Operator questions](../process/handoffs/v0.3.0-QUESTIONS.md)

## The requirement

The operator answered question 19 on 2026-10-28 with option a. The
self-hosted compiler is compiled to native code and used as the compiler,
and everything is checked to work as intended. The operator stated that this
is to be assumed a V0.4.0 requirement, and that V0.4.0 is cut tactically.
V0.2.X relayed on the same day, from its own session with the operator, that
three compilers should be usable, the reference compiler, the self-hosted
compiler interpreted and the self-hosted compiler native, and that the two
lines will probably merge when native code generation and self-hosting share
one executable. That remark is relayed, and this plan records it as such.

**This line's reading.** The self-hosted compiler is the fourteen stage
programs under `src/selfhost/kel/` and the Rust host code that runs them and
threads each stage's output into the next. The host code is Rust and is
already native. "Compiled to native code" therefore means that each stage
runs as native code produced by this line's backend instead of on the
bytecode virtual machine, under the same host code. This reading is this
line's and is proceeded on unless the operator objects.

## What exists, measured on 2026-10-28

The facts below were read from the tree at `ef839279`. Line numbers are
from that tree.

- **Each stage already runs natively, one at a time.**
  `native_codegen/tests/stage_differential.rs` lowers each of the fourteen
  stage sources, runs it on seeded shared input, and compares every yield
  and the whole shared segment with the virtual machine. It also chains
  `reconstruct.kel` into `codegen.kel` through the shared segment.
- **The driver is written against the concrete `Vm` type.** In
  `src/selfhost/`, each stage builds its machine inline, about fifteen sites
  in `mod.rs` and others in `typecheck.rs`, `text_flow.rs`,
  `data_relocation.rs`, `analysis_machine.rs` and `wire_framing.rs`. Each
  drives the stage through `call_with_shared`, `resume_with_shared`,
  `set_shared`, `get_shared` and `shared_data_bytes`. No trait or other
  executor boundary exists. Partial factoring exists in
  `selfhost_host::try_drive_parse_records_with` and
  `wire_framing::with_compiled_wire`.
- **Hand-offs pass through the host.** No buffer is passed from one stage to
  the next. The host reads a stage's yields and shared slots into Rust
  structures and seeds the next stage's fresh buffer by slot index, resolved
  through the stage's `data_layout`.
- **The stages run in this order for `compile --compiler self-hosted`.**
  - A type gate through `lexer.kel`, `parse.kel`, `reconstruct.kel` and
    `verify_types.kel`.
  - A first lexer pass, and probe rounds when generic templates exist.
  - The lexer fused into `parse.kel`, through an eight-token window slid
    before each resume.
  - `codegen.kel` in relocation mode, only when an address plan moves.
  - `reconstruct.kel` and `codegen.kel` once per chunk.
  - `analyze.kel` per stream chunk, and as a command machine for call order
    and text flow.
  - The command-line front end serializes the module through `wire.kel` and
    `wire_schema.kel`, or through `src/crypto_frame/frame.kel` when signing
    or encrypting. The six verify stages and `wire_debug.kel` do not run
    during compilation.
- **No stage calls a host native on the unsigned path.** `frame.kel` calls
  ten, registered as closures in `src/crypto_frame/mod.rs`.
- **The output is checked.** `self_hosted_compile` compares the self-hosted
  module with the reference compiler's through `artifact_compare::difference`,
  field by field. On the unsigned path the serialized bytes are not compared
  with the reference serializer. On the signed and encrypted paths they are.

A defect found while gathering these facts is repaired with this plan. The
stage differential's reconstruct-to-codegen bridge carried the AST widths as
constants of 1,024 and 256 and claimed them verified equal, while both stages
had declared 4,096 and 1,024 since the capacity raise of 2026-10-26. It
copied a prefix, and its small subject fit, so nothing failed. It now reads
each width from both layouts, asserts they agree, and asserts that the node
table's width is the driver's `AST_NODE_CAP`.

## The design

**Step 1, an executor boundary in the driver. V0.2.X's, by request.** Root
`src/` is V0.2.X's, so this line asks for it and does not edit it. The driver
would run each stage through a trait with the operations it already uses on
`Vm`: start with an argument, resume with a reply, set and get a shared slot
by index, and report the shared size. The virtual machine implements it
first, with no change of behaviour, so the existing self-hosted gates
measure the refactor. Each stage keeps its own construction site, and the
trait replaces only the concrete type.

**Step 2, a native executor. This line's.** `native_codegen` implements the
trait over a lowered stage. A stage is lowered once per process and cached,
since the driver recompiles several stages on every use. Slot-index access
goes through the same `data_layout` offsets the stage differential already
uses. The protocol differs in one place, which this step must settle. The
virtual machine distinguishes a call from a resume and reports a `Reset`,
while a native stage takes one call per turn. The stage differential's
`lockstep` already reconciles the two, one turn to one native call, with a
`Reset` resumed again under the same reply, and the executor adopts that
rule. The ten `frame.kel` natives are bound to the same Rust closures through
the JIT's symbol mapping.

**Step 3, the three compilers.** A selector chooses the reference compiler,
the self-hosted compiler on the virtual machine, or the self-hosted compiler
on native stages. Where the command lives depends on question 16's
delegation from `keleusma` to `kel-native`, which V0.2.X builds. Until it
lands, this line exposes the native route from `kel-native` alone.

**Step 4, the check that it works as intended.** Over the whole corpus, the
native self-hosted compiler's module must equal both the reference
compiler's, through `artifact_compare`, and the interpreted self-hosted
compiler's. A divergence between the two self-hosted routes would be a native
backend defect, since they run the same stage programs. Their serialized
bytes must also be equal. The existing per-stage differentials remain the
first line of evidence, and this check is the end-to-end one.

## Order and dependencies

1. This plan and the bridge repair, one pull request.
2. A request to V0.2.X for step 1, sent with this plan.
3. Step 2 can begin against the virtual machine's own interface through a
   local adapter in test code, so the native executor's protocol is tested
   before step 1 lands. It is wired into the driver once step 1 lands.
4. Steps 3 and 4 follow.

## Progress

- **2026-10-29, step 2 begun.** `native_codegen/src/stage.rs` holds
  `NativeStage`, and `native_codegen/tests/native_stage_executor.rs` drives
  the lexer into the parser through it and through a virtual machine adapter
  with one host function, and the two agree. All fourteen stages build. The
  measured build times, lowering and JIT compilation together, run from 12 ms
  for `verify_datalayout.kel` to 1764 ms for `parse.kel`, about 5.1 s for all
  fourteen on this machine. Binding the host natives of `frame.kel` remains,
  and wiring into the driver waits on step 1.
- **2026-10-29, host natives bound.** `NativeStage::with_natives` binds C
  functions by arity and refuses a mismatch when the stage is built. A program
  calling one two-argument native agrees with the virtual machine's closure.
  The natives of `frame.kel` are private closures over the driver's signing
  state, so that binding waits on V0.2.X exposing them, requested in sequence
  137 together with step 1.
- **2026-10-30, step 1 landed upstream.** V0.2.X's pull request 628, absorbed
  in absorption 118, adds `stage_executor::StageExecutor` and
  `selfhost::with_stage_factory`. The factory is offered each stage module
  before the driver builds it, and `frame.kel`'s ten natives reach a returned
  executor through `register_native` as boxed closures. The trait is shaped
  like the virtual machine. A native implementation needs an entry for each
  chunk the driver calls through `call_function`, a bridge from the boxed
  closures to the native code's calls, and the conversion of a yielded word
  to a `VmState`.
- **2026-10-30, step 3, native stages in the driver.**
  `native_codegen/src/stage_factory.rs` answers the factory. A stage's turns
  run natively, copying the driver's shared buffer in and out so the canary
  still holds, and its layout queries and `call_function` go to an inner
  virtual machine of the same module. `native_stage_factory.rs` compiles four
  programs byte-identical to the virtual machine and to the reference, and
  validates them with the same verdicts. A stage calling a host native is
  declined. Remaining are a bridge from the driver's closure natives to C
  calls for `frame.kel`, a cache of built stages, a selector for the three
  compilers, and the corpus-wide end-to-end check.
- **2026-10-30, the closure bridge.** Each native a stage calls is bound to a
  C trampoline by arity and slot, and the driver's closures are lent to the
  trampolines for each turn. `frame.kel` signs byte-identically and verifies
  natively. A native's error fails the turn rather than reaching the script.
  Remaining are the stage cache, the selector and the corpus-wide check.
- **2026-10-30, the cache and the corpus.** Each distinct stage module is
  compiled once per factory, and a turn runs on the driver's shared buffer
  directly, since copying it dominated. A warm native compile of
  `10_multbyte.kel` takes 1.19 s against 1.46 s on the virtual machine. All 69
  corpus sources in the subset compile byte-identically with native stages.
  The selector remains.
- **2026-10-30, the selector.** `kel-native build` and `inspect` take
  `--compiler rust`, the default, `self-hosted` or `self-hosted-native`. The
  self-hosted routes run `self_hosted_compile`, the native one with
  `NativeStageFactory` installed, and report how many stages ran natively and
  how many were declined. A source the reference accepts and a self-hosted
  route cannot compile exits 72 with the hint to retry with `--compiler rust`.
  Since question 16 `keleusma native` reaches the same command. The three
  compilers build byte-identical object files, headers and bindings for a
  streaming program, as `kel_native_cli.rs` shows, and the corpus check of
  step 4 is `native_stage_factory.rs`. Every step of this plan is done.

## Risks and open points

- **Cost.** A native stage is lowered and compiled in process. If that cost
  dominates a compilation, an ahead-of-time archive of the stages, built by
  `kel-native`, replaces the in-process lowering. That choice is deferred
  until it is measured.
- **Capacity.** The stages' shared tables are large after the capacity raise.
  The native frame, region and stack bounds are already computed per stage,
  so a stage that needs more than a host thread offers is refused rather
  than run. `parse.kel` already runs on a 64 MB thread under the virtual
  machine.
- **The verify stages.** They do not run during compilation, so they are
  outside this requirement as read here. Native validation through them
  would be a separate item.
- **Merging the lines.** The relayed remark expects one executable. Nothing
  in this plan prevents it, and nothing here does it.
