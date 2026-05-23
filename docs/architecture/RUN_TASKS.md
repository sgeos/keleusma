# Multi-Script Runner: `keleusma run-tasks`

> **Navigation**: [Architecture](./README.md) | [Documentation Root](../README.md)

**Status**: design proposal, not yet implemented. This document is the agreed contract for the V0.2.x `run-tasks` subcommand. The implementation lands as a separate piece once the contract is reviewed.

## Audience

Operators running multi-script daemon workloads on a single host. Two concrete patterns motivate the feature.

- **Multi-daemon on one host**. Operations workloads where a sensor poller, a log writer, and a watchdog all need to run on the same machine with shared state. The current alternative is one CLI process per script (no shared state, three times the resident memory) or a custom Rust host.
- **Cooperative-RTOS-shape workload on a desktop**. The [`examples/rtos/`](../../examples/rtos/) microkernel ships a substantial scheduler with WCET admission, supervised restart, and an event queue. Operators who want that shape on a desktop or server (not a STM32N6570-DK) currently have to write their own host. `run-tasks` lifts the RTOS pattern into the CLI for the desktop case.

The feature is deliberately opinionated. Operators with hard-real-time scheduling or preemption requirements should continue to write their own host using the `keleusma` library directly; the "write your own host" footnote in the CLI README applies. `run-tasks` aims at the common cooperative-scheduling case.

## Subcommand shape

```
keleusma run-tasks <manifest.toml> [--quiet]
```

The manifest argument is required. The `--quiet` flag suppresses non-error stderr output from the scheduler itself; per-task script output continues to flow to stdout and stderr as usual.

The subcommand sits alongside the existing `run`, `compile`, `keygen`, and `repl` subcommands. It does not modify any of them.

## Manifest format

The manifest is TOML. A typical multi-daemon manifest looks like the following.

```toml
[scheduler]
tick_interval = "10ms"          # Scheduler iteration cadence.
shutdown_grace = "5s"            # Grace period on SIGINT before forced exit.

[[task]]
name = "sensor"
bytecode = "tasks/sensor.kel.bin"
period = "100ms"                 # Period for fixed-cadence tasks.
restart = "on_error"             # one of: never, on_error, always
restart_limit = 5                # Maximum restarts within the window.
restart_window = "1m"            # Window over which the limit is counted.
arena_capacity = "64KB"          # Override the auto-sized arena.
priority = 0                     # Tie-breaker between simultaneously ready tasks.

[[task]]
name = "logger"
bytecode = "tasks/logger.kel.bin"
period = "1s"
restart = "always"

[[task]]
name = "watchdog"
bytecode = "tasks/watchdog.kel.bin"
period = "5s"
restart = "never"

[events]
# Operator-declared event ids the manifest understands.
# Tasks reference events by symbolic name in scripts; the manifest binds names
# to numeric ids the scheduler uses on the wire.
data_ready = 1
shutdown_requested = 2
```

### Manifest validation

Validation runs before any task starts.

| Check | Reject reason |
|-------|---------------|
| TOML well-formed | Parser error |
| Each `[[task]]` carries `name` and `bytecode` | Missing required field |
| Bytecode file exists | File-not-found |
| Bytecode loads under the active signing and encryption policy | Wire-format or policy rejection |
| Task entry chunk is `loop main(wakeup: Word) -> (Word, Word)` | Wrong entry shape |
| Task names are unique within the manifest | Duplicate name |
| Event names are unique within the manifest | Duplicate event |
| Durations parse against the same humanized-duration grammar as `--tick-interval` | Duration parse error |
| `restart_window` is at least `1s` and at most `1h` | Window out of range |
| `restart_limit` is at least 1 and at most 1000 | Limit out of range |
| `arena_capacity` parses (decimal optionally followed by `KB`/`MB`) | Capacity parse error |
| Task count fits a static maximum (initially 16) | Too many tasks |

Validation is fail-closed. A malformed manifest is rejected before any task script runs.

## Task entry shape

Each task is a `loop main(wakeup_reason: Word) -> (Word, Word)` matching the RTOS convention in [`examples/rtos/SPEC.md`](../../examples/rtos/SPEC.md) section 3.3.

The scheduler invokes the task with a `wakeup_reason` parameter. The task yields a `(reason, payload)` tuple. The scheduler reads the tuple to decide when to resume the task.

### Yield reasons

| Reason | Name | Payload | Semantics |
|--------|------|---------|-----------|
| 0 | Wait | Monotonic milliseconds | Sleep until the absolute deadline. |
| 1 | EventWait | Event id | Block until the named event is signalled. |
| 2 | Yield | Unused | Yield without a wakeup condition; scheduler picks the next ready task. |
| 3 | Periodic | Unused | Sleep until the manifest-declared period has elapsed since the task last ran. |

The Periodic reason is new versus the RTOS spec. It lets a manifest-configured task ignore the wall-clock and let the manifest's `period` field drive the cadence. A task that yields Periodic gets resumed every `period` regardless of how long the iteration took. This is the daemon-shaped use case most operators want.

### Wakeup reason on resume

When the scheduler resumes a task, it passes a `wakeup_reason` value back through the resume payload.

| Value | Semantics |
|-------|-----------|
| 0 | First invocation; the task has not run before. |
| 1 | Wakeup from a Wait or Periodic deadline. |
| 2 | Wakeup from an EventWait. The event id is read by the task through `kernel::last_event_id()`. |
| 3 | Wakeup from a voluntary Yield. |

Tasks can branch on the wakeup reason to take different actions for different wake conditions.

## Scheduler model

Cooperative, sleep-until-driven, monotonic-clock based. The model is the RTOS example's scheduler lifted into `keleusma-cli` with two adjustments.

1. The `tick_interval` is a manifest-supplied period for the scheduler loop itself, separate from per-task periods. This lets the scheduler bound its own wake frequency on desktop hosts where polling is cheap and the manifest may declare sub-millisecond task periods.
2. The Periodic yield reason lets tasks declare cadence per manifest field rather than computing absolute deadlines in script code. The RTOS example computes deadlines in script code; the desktop case is friendlier when the manifest owns the cadence.

### Scheduling algorithm

Pseudocode for the dispatch loop.

```
loop {
    let now = monotonic_now_ms();

    // 1. Refresh ready set: any task whose deadline has elapsed becomes Ready.
    for task in tasks {
        if task.state == SleepingUntil(deadline) && now >= deadline {
            task.state = Ready;
            task.last_wakeup_reason = WakeupReason::DeadlineElapsed;
        }
    }

    // 2. Drain the event queue: any task whose awaited event was posted becomes Ready.
    while let Some(event) = events.pop() {
        for task in tasks {
            if task.state == WaitingFor(event.id) {
                task.state = Ready;
                task.last_wakeup_reason = WakeupReason::EventFired;
                task.last_event_id = event.id;
            }
        }
    }

    // 3. Pick the highest-priority Ready task (lowest priority number wins).
    let candidate = tasks.iter_mut()
        .filter(|t| t.state == Ready)
        .min_by_key(|t| t.priority);

    let task = match candidate {
        Some(t) => t,
        None => {
            // No ready task. Sleep until the earliest deadline or the next event.
            let next_wake = earliest_deadline();
            sleep_until_or_event(next_wake);
            continue;
        }
    };

    // 4. Dispatch the task.
    match task.vm.resume(Value::Int(task.last_wakeup_reason as i64)) {
        Ok(VmState::Yielded(Value::Tuple(t))) if t.len() == 2 => {
            let reason = extract_int(&t[0]);
            let payload = extract_int(&t[1]);
            task.state = match reason {
                0 => SleepingUntil(payload as u64),
                1 => WaitingFor(payload as u8),
                2 => Ready,
                3 => SleepingUntil(now + task.period_ms),
                _ => { error("task yielded unknown reason"); Finished },
            };
        }
        Ok(VmState::Reset) => {
            task.state = Ready;
        }
        Err(e) => {
            on_task_error(task, e);
        }
        _ => task.state = Finished,
    }
}
```

The scheduler is single-threaded. There are no locks because cooperative scheduling means the kernel runs exclusively between dispatches. Operating-system signals (`SIGINT`, `SIGTERM`) are caught by a small signal-handler that posts a synthetic `shutdown` event the scheduler reads on its next iteration.

### Worst-case latency claim

For task `T` with WCET-to-yield `W_T` cycles, the worst-case dispatch latency is

```
latency(T) = scheduler_overhead + max(W_other for other tasks ready at the same instant)
```

This is the same load-bearing property as the RTOS example. Every `W_other` is verifier-proven. The scheduler overhead is small and measurable. The property is preserved as long as every task admits a finite WCMU and WCET bound.

## Event queue

The event queue is a fixed-capacity FIFO. Tasks signal events through a host native; the scheduler drains the queue between dispatches and wakes any tasks blocking on the matched event id.

### Native functions for tasks

| Native | Signature | Semantics |
|--------|-----------|-----------|
| `kernel::post_event` | `(id: Word, payload: Word) -> ()` | Push `(id, payload)` onto the event queue. Returns immediately. |
| `kernel::last_event_id` | `() -> Word` | Read the id of the event that woke this task. Defined only when `wakeup_reason == 2` (EventFired); returns 0 otherwise. |
| `kernel::last_event_payload` | `() -> Word` | Read the payload of the event that woke this task. Defined only when `wakeup_reason == 2`. |
| `kernel::now_ms` | `() -> Word` | Monotonic milliseconds. Distinct from `shell::now_unix_ms` because the scheduler's clock is monotonic, not wall-clock. |
| `kernel::task_id` | `() -> Word` | The numeric id the scheduler assigned to this task. Useful for logging. |
| `kernel::task_name` | `() -> Text` | The string name from the manifest. |

The natives are registered automatically by the scheduler for every task. Their signatures participate in compile-time validation through a `KERNEL_SIGNATURES` constant prepended to every task source at compile time (analogous to the existing `Shell::SIGNATURES`).

Tasks cannot subscribe to events programmatically; they yield with reason 1 (EventWait) and the scheduler matches against the manifest's declared event names. Operators who want richer subscription semantics should write their own host.

### Event queue capacity

The event queue capacity is fixed at compile time of the CLI. The initial choice is 64 entries. When the queue is full, `kernel::post_event` returns immediately but the event is discarded with a stderr warning (unless `--quiet`). Operators with bursty workloads should size the queue or use a different host.

## Restart policy

Per-task restart policy declared in the manifest. Three modes.

| Mode | Behaviour |
|------|-----------|
| `never` | Task error or normal termination is fatal for that task. Other tasks continue. The scheduler logs the termination and removes the task from the ready set. |
| `on_error` | Task is restarted on any `VmError`. Normal termination (loop body exits, which is unusual for `loop main`) is treated as terminal. |
| `always` | Task is restarted on both error and termination. The script can voluntarily exit through the loop's natural fall-through and the scheduler will respawn it. |

A restart re-allocates the task's arena and re-instantiates the VM from the bytecode. The task's data segment values do not survive a restart; the task observes a fresh allocation.

### Restart rate limiting

To avoid restart storms, each task carries a sliding-window restart count. If the count exceeds `restart_limit` within `restart_window`, the task is treated as `never` for the remainder of the runner's lifetime; the scheduler logs the disabling and continues with the other tasks. Defaults: `restart_limit = 5`, `restart_window = 1m`.

## Security model

Each task is a separately-loaded bytecode artefact. Each goes through the same policy gates as `keleusma run`. The same enrolled-key stores and the same strict-mode discovery applies.

Specifically:

| Policy | Behaviour |
|--------|-----------|
| Strict signing active | Every task's bytecode must be signed by an enrolled signer; unsigned task bytecode causes manifest rejection. |
| Strict encryption active | Every task's bytecode must be encrypted to an enrolled recipient; unencrypted task bytecode causes manifest rejection. |
| Permissive mode | The manifest can carry per-task `verifying_key` and `decryption_key` fields with paths; absence allows unsigned and unencrypted bytecode. |

The manifest itself is not signed. An operator who needs end-to-end policy on the manifest should rely on filesystem permissions (root-owned, mode 0644) and consider the manifest part of the trusted configuration. A future iteration may add manifest signing under the same Ed25519 scheme.

Tasks share the same process and the same operating-system credentials. A misbehaving task can observe (but not modify) other tasks' memory only by reading the process's address space at the operating-system level; the CLI does not impose hardware-isolated memory between tasks. Operators needing per-task isolation should run separate processes.

## Manifest example: three-daemon deployment

A realistic three-task deployment combining sensor polling, log writing, and a heartbeat.

```toml
[scheduler]
tick_interval = "10ms"
shutdown_grace = "5s"

[events]
sensor_threshold_exceeded = 1
shutdown_requested = 99

[[task]]
name = "sensor_poller"
bytecode = "tasks/sensor_poller.kel.bin"
period = "100ms"
restart = "on_error"
priority = 0

[[task]]
name = "log_writer"
bytecode = "tasks/log_writer.kel.bin"
restart = "always"
priority = 1
# Note: no `period`. log_writer is event-driven; it yields with reason 1
# (EventWait) on `sensor_threshold_exceeded` and sleeps until signalled.

[[task]]
name = "heartbeat"
bytecode = "tasks/heartbeat.kel.bin"
period = "5s"
restart = "never"
priority = 2
```

The corresponding `sensor_poller.kel` (abbreviated, illustrative):

```keleusma
use shell::now_unix_ms
use shell::run_checked
use kernel::post_event

data state {
    last_reading: Word = 0,
}

loop main(wakeup_reason: Word) -> (Word, Word) {
    let reading = parse_sensor(shell::run_checked("sensor_cmd"));
    state.last_reading = reading;
    if reading > 1000 {
        kernel::post_event(1, reading);  // sensor_threshold_exceeded
    };
    // Yield with reason 3 (Periodic) — scheduler reads the period from manifest.
    let _ = yield (3, 0);
    (0, 0)
}
```

The corresponding `log_writer.kel` (abbreviated):

```keleusma
use shell::append_file
use kernel::last_event_payload
use shell::now_unix_ms

loop main(wakeup_reason: Word) -> (Word, Word) {
    if wakeup_reason == 2 {
        let payload = kernel::last_event_payload();
        let entry = format_entry(shell::now_unix_ms(), payload);
        let _ = shell::append_file("/var/log/sensor.log", entry);
    };
    let _ = yield (1, 1);  // Wait for event id 1.
    (0, 0)
}
```

The corresponding `heartbeat.kel` (abbreviated):

```keleusma
use println

loop main(wakeup_reason: Word) -> (Word, Word) {
    let _ = println("heartbeat");
    let _ = yield (3, 0);  // Periodic
    (0, 0)
}
```

## Output format

The scheduler emits structured stderr lines for operational visibility. Tasks' own stdout and stderr writes are not modified.

| Line shape | Meaning |
|------------|---------|
| `[scheduler] launching N tasks` | Startup. |
| `[scheduler] task <name> WCET <cycles> WCMU <bytes>` | Per-task verifier-computed bounds, printed at startup. |
| `[scheduler] task <name> restarted (reason: <reason>)` | Restart event. |
| `[scheduler] task <name> disabled after <N> restarts in <window>` | Rate-limit triggered. |
| `[scheduler] task <name> terminated (reason: <reason>)` | Voluntary or fatal termination. |
| `[scheduler] event queue full; dropped event id <id>` | Overflow. |
| `[scheduler] shutdown requested, draining tasks` | SIGINT received. |
| `[scheduler] shutdown complete` | All tasks finished or grace period elapsed. |

The `--quiet` flag suppresses every `[scheduler]` line except errors.

## Stop and signal semantics

The scheduler installs handlers for `SIGINT` and `SIGTERM`. On signal, it posts an `event_id = shutdown_requested` (id 99 by convention; manifest can override) and starts the shutdown grace period.

During the grace period, the scheduler keeps dispatching tasks normally so they can finish in-flight work. Tasks that want to participate in graceful shutdown should EventWait on the shutdown event and exit cleanly. After the grace period elapses, any remaining tasks are forcibly terminated through `Vm::halt` and the process exits.

A task that calls `shell::exit(code)` terminates the entire process immediately, as in the existing CLI behaviour.

## Relationship to `examples/rtos/`

The `run-tasks` runner reuses the architectural ideas from `examples/rtos/` but does not depend on its code. Specifically:

| RTOS concept | Reused in `run-tasks`? |
|---|---|
| Cooperative scheduler with sleep-until dispatch | Yes |
| `(reason, payload)` yield convention with Wait/EventWait/Yield | Yes, plus Periodic |
| Per-task supervised restart on `VmError` | Yes, with rate limiting |
| Event queue with `post_event` and EventWait | Yes |
| Per-task WCET admission at load | Yes |
| `Platform` trait abstraction over hardware backends | No (CLI is std-only) |
| Embassy/Cortex-M target | No |
| Watchdog feed | No |

The RTOS example remains the reference for embedded targets; `run-tasks` is the desktop analogue.

A future refactor might lift the scheduler core into a shared crate consumed by both `keleusma-cli` (for `run-tasks`) and the RTOS example. The initial commit duplicates the small scheduler in `keleusma-cli` for simplicity; refactoring is a follow-on.

## Open questions and future work

These are explicit deferrals worth tracking but not blocking V0.2.x landing.

1. **Manifest signing**. The manifest itself is currently filesystem-trusted. A future iteration may add Ed25519 signing under the same scheme used for bytecode.
2. **Per-task isolation**. Tasks share the process and address space. Operators needing memory isolation between tasks should run separate processes. A future iteration may add `unshare`-style isolation on platforms that support it.
3. **Dynamic task addition**. The initial implementation reads the manifest at startup and does not support adding or removing tasks at runtime. A future iteration may add a control socket or a `kernel::add_task` native.
4. **Hot reload**. A signal-driven manifest reload that brings up new tasks and tears down removed tasks is on the wishlist. Initial implementation does not include it.
5. **Priority levels and preemption**. The cooperative model is non-preemptive. Operators needing preemption should write their own host.
6. **Per-task resource caps beyond WCMU**. The arena capacity is bounded; the cooperative scheduler also bounds wall-clock through the per-iteration deadline. A misbehaving task that takes longer than its WCET bound predicted is dispatched to completion before the scheduler advances. A future iteration may add a soft cap that kills runaway tasks.
7. **Event payload typing**. Events currently carry a single `Word` payload. A future iteration may broaden to typed payloads through a manifest-declared event schema.
8. **Task-to-task ABI compatibility checking**. Tasks declare events by numeric id in the manifest; if two manifests disagree on the id-to-name mapping, the system silently misbehaves. A future iteration may add a manifest-shared event schema with versioning.

## Cross-references

- [`examples/rtos/SPEC.md`](../../examples/rtos/SPEC.md) for the cooperative scheduler reference design that this proposal lifts.
- [`keleusma-cli/README.md`](../../keleusma-cli/README.md) for the existing single-script loop runner and the tick-interval rate limiter.
- [`docs/guide/SECURITY_POLICY.md`](../guide/SECURITY_POLICY.md) for the strict-mode signing and encryption gates that apply to each task's bytecode.
- [`docs/guide/SHELL_AUDIT.md`](../guide/SHELL_AUDIT.md) for the bundled shell natives the tasks rely on for filesystem and subprocess access.
- [`docs/architecture/EXECUTION_MODEL.md`](./EXECUTION_MODEL.md) for the per-Vm execution model that each task instantiates independently.
- [`docs/architecture/SUB_COROUTINES.md`](./SUB_COROUTINES.md) for the V0.5.0+ sub-coroutine primitive that may eventually replace the current event-queue mechanism with a more structured concurrency model.
