# Prepared, not applied

Everything here is **written and unverified**. None of it has been compiled or
run. It exists in the repository rather than in a session scratchpad for one
reason, which is that a scratchpad does not survive the session that made it, and
four artefacts had accumulated there.

**Nothing in this directory is built.** Cargo compiles `src/`, `tests/`,
`benches/` and `examples/`. A file here is inert, so parking an uncompiled
artefact costs nothing and losing one costs the work that produced it.

## Why these were written without being run

Two sessions share one machine. A full gate takes roughly three and a half hours
and saturates it, so the session that does not hold the gate cannot compile. The
alternative to preparing work is idling, and preparation that names its own
verification state is worth more than idling.

**Every file here states in its own header that it is uncompiled.** That is not
decoration. An artefact that looks finished is the failure mode this directory
exists to avoid.

## Contents

| File | Install as | What it settles |
|---|---|---|
| `spike_stream_sufficiency.rs` | `tests/spike_stream_sufficiency.rs` | Whether `Stream` and `Reset` alone unblock the self-hosted stages, and the bytecode-level yield shape of every stream chunk |
| `o2_differential_arm.rs` | append to `tests/differential.rs` | Whether the lowering survives the `default<O2>` pipeline, which is the shipped configuration; every existing case runs at O0 through the JIT, which the architecture excludes from scope |
| `retcon_declarability.rs` | append to `tests/coroutine_feasibility.rs` | Whether inkwell can DECLARE the `coro.id.retcon` family, not merely find it by name; the one clause left at medium confidence in `V0_4_0_NATIVE_CODEGEN.md` R4.4 |
| `fix_workstream_label.py` | run from the worktree root | Corrects six "Workstream C" mislabels, four of which are shipped `LowerError` strings that send a consumer to the wrong workstream |

## Applying one

The Python script asserts on every anchor before writing anything, so a partial
apply is not reachable. It is **not idempotent**: running it twice leaves the
anchors unmatched and it aborts, which is the intended behaviour rather than a
defect, but do not run it against an already-corrected tree expecting a no-op.

The Rust files are plain source. After installing any of them:

```sh
cargo fmt && cargo clippy --tests -- -D warnings && cargo test
```

**Expect the first compile to fail.** These were written against the API by
reading it, not by exercising it, and this branch has already had two artefacts
fail to compile for exactly that reason: `TargetMachine::get_host_cpu_name()`
returns an `LLVMString` rather than a `&str`, and `get_declaration(&m, &[])` does
not infer `BasicTypeEnum` from an empty slice. Both were caught by reading
neighbouring code afterwards. Budget for a compile-fix pass rather than treating a
failure as a finding.

## The ledger is elsewhere

Which artefacts are **spent** is recorded under "ARTEFACT LEDGER" in
[`../../docs/decisions/NATIVE_LOWERING_INVENTORY.md`](../../docs/decisions/NATIVE_LOWERING_INVENTORY.md).
This directory holds no state of its own on purpose. One artefact previously
showed a misleading two-of-four anchor match, and re-running it would have applied
a change twice; a single authority on what is spent is what prevents that.
