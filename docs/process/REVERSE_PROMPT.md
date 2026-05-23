# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-23
**Status**: V0.2.1 multi-script runner complete. Six commits land the design proposal, the implementation, the limitation closures, and the documentation. No open contract gaps remain.

## Summary of work since the last reverse-prompt update

Six commits across two days delivered `keleusma run-tasks <manifest.toml>`. Each commit was reviewable independently so the operator could redirect between them.

### `93b0173` design proposal

`docs/architecture/RUN_TASKS.md` as the agreed contract. TOML manifest schema, cooperative scheduler model lifted from `examples/rtos/`, RTOS-shape task entry with the four yield reasons (Wait, EventWait, Yield, Periodic), fixed-capacity event queue, supervised restart with sliding-window rate limit, per-task signing and encryption policy, eight open questions deferred.

### `67c1f9a` memory residency and OS-portable deployment

Three additions covering the deployment shape the operator stated. Memory residency and allocation discipline called out as load-bearing properties for root deployments on critical hardware. Operating-system-agnostic process contract plus per-platform recipes for Linux (systemd, OpenRC, runit, s6), FreeBSD (rc.d), OpenBSD (rc.d), macOS (launchd), and Windows (NSSM wrapper). Stop and signal semantics expanded to cover SIGHUP and Windows console control events.

### `f53b988` initial implementation

`keleusma-cli/src/runtasks/` module with three files. Manifest parser with 11 unit tests. Cooperative scheduler with monotonic-clock dispatch, event queue, restart policy. Cross-platform signal handling via `signal-hook`. NOTIFY_SOCKET protocol detection with READY=1, STATUS, STOPPING=1, WATCHDOG=1 emission. Six kernel natives registered per task. CLI wiring through a new `run-tasks` subcommand. Two new dependencies: `toml 0.8` and `signal-hook 0.3`. End-to-end smoke tests of single-task, two-task with event coordination, and three manifest-rejection paths.

### `4a5ed96` close three known limitations

Native re-registration on restart now works through a new `EventAtomics` struct held on the Task struct; the same Arcs survive restart and the new VM's natives observe the same shared state. The `kernel::last_event_id` and `kernel::last_event_payload` natives now return real values by writing into per-task atomics in the scheduler's event-fired path. Linux abstract NOTIFY_SOCKET addresses now work through `std::os::linux::net::SocketAddrExt::from_abstract_name`. Adjacent fix: arena auto-sizing now takes the max of the operator's `arena_capacity` and the module's auto-computed WCMU bound, so scripts with higher WCMU than the manifest default admit cleanly.

### Present commit: contract gaps and documentation

Three gaps against the design contract closed.

- WCET and WCMU bounds printed per task at load. The verifier-computed bounds are the certification evidence operators copying into deployment records expect to see at startup.
- POSIX-conventional exit codes. SIGINT and SIGTERM are tracked through separate atomic flags; clean drain returns 130 or 143 respectively. Natural shutdown returns 0. Manifest validation and task-load failures return 1.
- CLI README documents the run-tasks subcommand with a minimal-manifest example, a three-task production manifest, the signal contract, and a pointer to the architecture document for per-platform recipes.

## Verification

- `cargo test --workspace`: 1032+ tests passing.
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- End-to-end manual tests:
  - Single-task daemon under `--tick-interval`-style periodic cadence drains cleanly on SIGINT with exit code 130, on SIGTERM with exit code 143.
  - Two-task producer plus consumer coordination via `kernel::post_event` and EventWait yield reason; consumer observes correct event metadata through the natives.
  - Crash-restart loop with `restart_limit = 3`: scheduler restarts three times then disables the task; subsequent dispatches skip the disabled task and the scheduler exits when nothing remains.
  - Manifest validation rejection paths return exit code 1 with clean diagnostics.

## Deferred work

Ten items remain in `docs/architecture/RUN_TASKS.md` section "Open questions and future work". None blocks V0.2.1 landing; each was explicitly marked deferred in the design proposal.

| # | Item | Notes |
|---|------|-------|
| 1 | Manifest signing | Substantial; needs an Ed25519 signing scheme for the TOML itself. |
| 2 | Per-task isolation | Requires per-OS work (Linux namespaces, FreeBSD jails, equivalents). |
| 3 | Dynamic task addition | Control socket or new `kernel::add_task` native. |
| 4 | Hot reload via SIGHUP | Signal handler is installed but performs no action; manifest re-read and graceful teardown are the open work. |
| 5 | Priority levels and preemption | Out of scope by design; operators needing preemption write their own host. |
| 6 | Soft resource caps beyond WCMU | The arena and cooperative model already bound resources; a kill-runaway-task cap is the open work. |
| 7 | Typed event payloads | Events currently carry a single `Word`; a manifest-declared event schema is the open work. |
| 8 | Task-to-task ABI compatibility checking | Tasks declare event ids by number; schema versioning would catch mismatches. |
| 9 | Native Windows Service Control Manager integration | The NSSM wrapper path works today; native integration is a separate effort. |
| 10 | Notification-protocol convention on non-systemd supervisors | `NOTIFY_SOCKET` works on Linux systemd; other supervisors do not define an equivalent. |

Other CLI deferred items not specific to `run-tasks`:

- Mutable `shared`/`private data` persistence across REPL evaluations. Requires arena snapshot-and-restore in the VM or incremental module loading.
- Generic `Result<T, E>` type. Language-design question deferred deliberately; the trap-on-error pattern works for the bundled shell natives.
- `shell::read_lines`. Contingent on a dynamic-length Array type or equivalent.

## Recommended next step

The V0.2.1 CLI surface is feature-complete against every documented contract and the operator's stated deployment shape. No open contract gaps. The ten run-tasks deferrals and the three broader CLI deferrals are individually substantial and individually scoped; any one of them is an appropriate next session if the operator's workload concretely calls for it.

If the operator wants to keep advancing the CLI surface, the highest-leverage remaining items are probably hot reload via SIGHUP (#4 in the run-tasks list) and the generic `Result<T, E>` type (broader CLI list). Both have load-bearing implications for the design but neither is on the critical path for V0.2.1.

If the operator wants to look further, V0.3.0 self-hosting (per `docs/roadmap/V0_3_0_SELF_HOSTING.md`) is the next planned major work, with the NES-6502 research material (`tmp/research/nes-6502/`) waiting to inform the V0.5+ language extensions.
