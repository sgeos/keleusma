# StdDSL::Shell Audit

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

Assessment of the V0.2.1 `stddsl::Shell` bundle against typical devops and sysadmin workloads delivered as signed-and-encrypted `loop main` scripts. The initial audit identified three priority gaps. All three have been closed under V0.2.1; the bundle is now adequate for the daemon use case. This document records the current state and the recommendations that remain open.

## Present capabilities

The bundle ships thirteen natives, registered through `vm.register_library(stddsl::Shell)`.

| Native | Signature | Purpose |
|--------|-----------|---------|
| `shell::getenv` | `(name: Text) -> Option<Text>` | Read an environment variable; `Some(value)` or `None` |
| `shell::has_env` | `(name: Text) -> bool` | Test whether an environment variable is set |
| `shell::run` | `(cmd: Text) -> (Word, Text)` | Execute `cmd` through `sh -c`; returns `(exit_code, stdout)`. Non-zero exit code is not an error |
| `shell::run_checked` | `(cmd: Text) -> Text` | Execute `cmd` through `sh -c`; returns stdout. Non-zero exit code surfaces as `NativeError` |
| `shell::exit` | `(code: Word) -> ()` | Terminate the host process with the given exit code |
| `shell::sleep_ms` | `(ms: Word) -> ()` | Sleep the current thread for `ms` milliseconds without spawning a subprocess |
| `shell::now_unix_ms` | `() -> Word` | Return the current Unix timestamp in milliseconds |
| `shell::read_file` | `(path: Text) -> Text` | Read a file's contents; traps on I/O failure or non-UTF-8 |
| `shell::write_file` | `(path: Text, content: Text) -> ()` | Replace a file with the given content; traps on I/O failure |
| `shell::append_file` | `(path: Text, content: Text) -> ()` | Append to a file, creating when absent; traps on I/O failure |
| `shell::file_exists` | `(path: Text) -> bool` | Test whether a filesystem entry exists; follows symlinks |
| `shell::write_err` | `(text: Text) -> ()` | Write to stderr without a trailing newline |
| `shell::writeln_err` | `(text: Text) -> ()` | Write to stderr with a trailing newline |

Combined with the bundled `println` from `utility_natives`, scripts can produce output, run external processes, sleep without forking, read and write files directly, and route log-shaped output to stderr.

## Closed gaps

The initial audit identified three critical gaps and four important gaps. All seven have been closed under V0.2.1, with one design adjustment from the original recommendation.

| Original recommendation | Status | Notes |
|-------------------------|--------|-------|
| `shell::sleep_ms` | Implemented | Negative or zero inputs return immediately rather than rejecting. |
| `shell::now_unix_ms` | Implemented | Returns the Unix timestamp in milliseconds; clamped to the Word range when the system clock returns a value beyond i64 milliseconds, which is implausible in practice. |
| `shell::write`, `shell::writeln`, `shell::write_err`, `shell::writeln_err` | Partially implemented | The stderr variants are present. The stdout variants (`write` and `writeln`) were not added because the existing `println` from `utility_natives` covers the common case and the inline-write idiom is sufficiently rare to defer. |
| `shell::file_exists` | Implemented | Follows symlinks; the no-follow variant is not yet exposed but can be added if a use case surfaces. |
| `shell::read_file` | Implemented | Returns `Text` directly and traps on I/O failure or non-UTF-8 content via `NativeError`, matching the `shell::run_checked` error pattern. The originally proposed `Result<Text>` return type was reduced to a plain trap because the language does not yet have a generic Result type; the design decision was to keep the consistent trap-on-error pattern across the bundle rather than introduce Result solely for file I/O. |
| `shell::write_file`, `shell::append_file` | Implemented | Same trap-on-error pattern as `read_file`. |
| `shell::read_lines` | Not implemented | The per-line iteration use case is served by `read_file` plus host-side splitting; a dedicated native can be added when the per-line trap-on-error semantics become a real requirement. |

The trap-on-error convention is documented in the per-native contracts in [`STANDARD_LIBRARY.md`](../spec/STANDARD_LIBRARY.md). A future generic `Result` type or refinement-newtype wrapper could replace the trap pattern; that is a language-design question rather than a bundle-design one.

## Net assessment

The current bundle is **adequate** for both one-shot and daemon-shaped workloads. The three V0.2.1 additions (sleep, time, file I/O) closed the gaps that previously made the daemon use case awkward. A loop daemon can now pace itself without forking, read configuration from files, write log records to files or stderr, and reason about elapsed time, all without delegating to subprocesses.

The signed-and-encrypted delivery model is fully usable in both atomic and productive-divergent modes today.

## Open recommendations

The remaining items are convenience additions rather than gap closures. None blocks the daemon use case.

| Priority | Native | Use case |
|----------|--------|----------|
| Low | `shell::arg_count`, `shell::arg(i)` | Inspecting the script's own argv. |
| Low | `shell::pid` | Pidfile creation for service-style daemons. |
| Low | `shell::hostname` | Multi-host deployments that vary behaviour by host. |
| Low | `shell::setenv` | Setting env vars before spawning subprocesses through `shell::run`. |
| Low | `shell::pwd`, `shell::cd` | Scripts that operate on a specific working directory. |
| Low | `shell::run_timeout(cmd: Text, ms: Word) -> (Word, Text)` | Subprocess execution with a wall-clock cap. |
| Low | `shell::read_lines(path: Text) -> Array<Text>` | Common per-line iteration; today scripts read the whole file and split host-side. |

These can ship in V0.2.x point releases when operator demand surfaces. None is on a critical path.

## Net judgment on the daemon use case

`stddsl::Shell` is now rich enough to write substantial devops and sysadmin loop daemons in Keleusma. The use cases that work today: signed-and-encrypted artefacts deployed to operator-controlled hosts, executing operational logic in a productive-divergent loop with built-in rate limiting via `--tick-interval`, reading configuration files, writing log records to stderr or to disk, exiting cleanly via `shell::exit`. The trap-on-error pattern for I/O failures gives daemon scripts fail-fast behaviour when the host filesystem disagrees with expectations.
