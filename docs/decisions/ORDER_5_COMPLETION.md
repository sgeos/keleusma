# Order 5 completion, Workstreams G and H

> **Navigation** [Decisions](./README.md) | [Plan](./ORDER_5_PLAN.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

The roadmap gives Order 5 the gate "The flat-machine ISA in native; native hot
swap". The requirements are those of [`ORDER_5_PLAN.md`](./ORDER_5_PLAN.md),
which records the premise this work takes for Workstream G. Only the native
half of the B28 redesign is in scope, because the bytecode half changes the
instruction set and its timing is roadmap open decision 3, the operator's.

## The gate

**Met on 2026-10-25 for the native half of the flat machine.** A coroutine
lowers with no run-time kind when its analysis permits, and every such corpus
coroutine the native route admits agrees with the virtual machine. A native
library is replaced while its host runs and agrees with the virtual machine's
own hot swap.

## Workstream G

| Id | Requirement | Executable evidence | Disposition |
|---|---|---|---|
| G1 | The native operand representation is untagged | Native operands are untagged `i64` words and composites are bytes at baked offsets. Outside a coroutine there is no run-time kind at all, because the emitter resolves each kind while lowering and refuses an unknown one. Inside a coroutine `coroutine::kind_census` counts where a run-time kind is needed. [`static_kind_lowering.rs`](../../native_codegen/tests/static_kind_lowering.rs) reports that 5995 of 170818 analysed corpus instructions need one | Implemented, as a measurement |
| G2 | A tag-free lowering exists and is proven tag-free | `LowerOptions::static_kinds` refuses a coroutine unless every operand of every reachable instruction has one kind and no instruction needs a guard. Every kind the emitter then stores is a constant. 18 of the 30 corpus coroutines are static. The harness drives the 14 of those that take one `Word` and call no host native, and they are exactly the 14 self-hosted stage sources. Each lowers with the option, emits no guard and agrees with the VM after the optimiser. One more static module is refused by the coroutine route, and three take other parameters or call natives | Implemented |
| G3 | Every kind-polymorphic instruction is resolved statically or refused | Outside a coroutine by construction. Inside one, the census classifies each instruction, and the option refuses the first that needs a run-time kind. `14_frame_log.kel` is refused with the option, at a kind guard, and lowers without it | Implemented |

The option adds an admission check and changes no emitted instruction. A
coroutine it admits therefore lowers to the same code as without it, and the
earlier differentials over the stage sources, which run real input, apply to
that code unchanged.

## Workstream H

| Id | Requirement | Executable evidence | Disposition |
|---|---|---|---|
| H1 | A native library is replaceable while its host runs | [`native_hot_swap.rs`](../../native_codegen/tests/native_hot_swap.rs) exports two versions of a stream under one prefix and links each as a shared object. A C host loads the first with `dlopen`, runs four steps, loads the second, releases the old instance, confirms it is inactive, unloads the old library with `dlclose` and starts the new one on the same shared buffer | Implemented |
| H2 | The acceptance rule is the VM's | Libraries export `<prefix>_schema_hash`, the module's `schema_hash`. The host accepts a replacement only when the hash and `shared_bytes` match. A version that renames a shared slot is refused by the host and by `Vm::replace_module` | Implemented |
| H3 | The swapped run agrees with the VM | The same versions run through `Vm::replace_module` at the same step. All nine yields and every shared byte agree. The swapped run differs from a run that swaps the first version into itself, so a host that never swapped would fail | Implemented |

## Departures and assumptions

- **The flat machine is native only.** The bytecode's arithmetic and
  comparison opcodes still dispatch on their operand's kind in the virtual
  machine. Carrying the kind in the opcode is B28's other half and is recorded
  as an open question.
- **An ambiguous local does not make a coroutine dynamic.** The virtual
  machine clears locals at `Reset`, so a local holds `Unit` at a loop head on
  one path and a value on another. 38035 analysed instructions have such a
  local. Code reads a local's kind only after pushing it as an operand, where
  the census counts it, so the local alone is counted and not refused.
- **A swap replaces a whole library at a step boundary.** The live instance
  is released and the new version starts from its entry with its own private
  image, as `replace_module` clears frames and takes new private data. State
  beyond the shared buffer is not migrated.
- **The library is its own replacement boundary.** The strategy's concern
  that inlining across modules defeats native replacement does not arise,
  because each version is one library whose only calls are to itself and to
  host natives.

## Residual limits

- Twelve corpus coroutines need a run-time kind. Ten first need it for an
  operand that may hold more than one kind and one for a kind guard. One
  further module is refused by the coroutine route.
- **Amended 2026-10-29.** The census counted every stack entry, including
  entries no instruction reads. Counting only the operands an instruction
  reads, the ten `piano_roll` coroutines are static, because their ambiguous
  value is a native result that only `PopN` and the soft-failure trap pop.
  Two coroutines and 21 instructions now need a run-time kind. The figures
  above are kept as they were recorded.
- `schema_hash` covers slot names and visibility, not slot types, so two
  layouts that differ only in a slot's type pass the acceptance rule in both
  shapes. `shared_bytes` catches a change of size and not one of type alone.
- Only `dlopen` on `aarch64-apple-darwin` was executed. Windows loading is not
  attempted.
- The stage sources were driven by this file with an empty shared buffer.
  Their real-input agreement is the default lowering's, established earlier.

## Verification

Run locally on 2026-10-25 with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1`
in `native_codegen/`, on a tree that did not change during the run. Logs are
under ignored `tmp/order-5/`.

| Check | Result |
|---|---|
| `cargo fmt --check` | status zero |
| `cargo clippy --locked --all-targets -- -D warnings`, default and `narrow-float-32` | status zero in both |
| `cargo doc --locked --no-deps` with `RUSTDOCFLAGS="-D warnings"` | status zero |
| `shellcheck tools/*.sh examples/lexer_host/run.sh` | status zero |
| Native `cargo test --locked --no-fail-fast`, default features | 831 passed, 1 failed |
| Native `cargo test --locked --no-fail-fast --features narrow-float-32` | 831 passed, 1 failed |
| Root `--test documentation_links --test comment_citations --test ci_shard_coverage --test gate_ci_correspondence` | 16 passed, status zero |

**The one failure in each configuration was the same, and it is repaired but
not covered by a second full run.** `every_comment_citation_resolves_to_something_real`
rejected a doc comment in `native_hot_swap.rs` that named the library prefix in
backticks as if it were an item. The comment was reworded. The citation guard
and both hot-swap tests were then run again in each configuration, and each
passed. The change touches one comment in one test file, so no other test can
observe it. The hosted run on the pull request covers the whole suite again.
These checks do not run the root workspace suite, the self-host tier or the
backend gate's optimised phase.
