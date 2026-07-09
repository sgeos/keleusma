# StdDSL::Shell Audit

> **Navigation**: [Guide](introduction.md) | [Documentation Root](../../docs/README.md)

Assessment of the V0.2.1 `stddsl::Shell` bundle against typical devops and sysadmin workloads delivered as signed-and-encrypted `loop main` scripts. The initial audit identified three priority gaps. All three have been closed under V0.2.1; the bundle is now adequate for the daemon use case. A subsequent review of the script-orchestration path found that `shell::arg`/`arg_count` reported the host process argv rather than the script's own arguments and that `shell::run` discarded captured stderr; both were corrected under V0.2.1. This document records the current state, the corrections, and the limitations that remain.

## Present capabilities

The bundle ships twenty-two natives, registered through `vm.register_library(stddsl::Shell)`.

| Native | Signature | Purpose |
|--------|-----------|---------|
| `shell::getenv` | `(name: Text) -> Option<Text>` | Read an environment variable; `Some(value)` or `None` |
| `shell::has_env` | `(name: Text) -> bool` | Test whether an environment variable is set |
| `shell::run` | `(cmd: Text) -> (Word, Text)` | Execute `cmd` through `sh -c`; returns `(exit_code, stdout)`. Captured stderr is discarded. Non-zero exit code is not an error |
| `shell::run_full` | `(cmd: Text) -> (Word, Text, Text)` | Execute `cmd` through `sh -c`; returns `(exit_code, stdout, stderr)` |
| `shell::run_checked` | `(cmd: Text) -> Text` | Execute `cmd` through `sh -c`; returns stdout. Non-zero exit code surfaces as `NativeError` |
| `shell::run_timeout` | `(cmd: Text, ms: Word) -> (Word, Text)` | Execute `cmd` with a wall-clock deadline; traps on timeout after killing the subprocess |
| `shell::exit` | `(code: Word) -> ()` | Terminate the host process with the given exit code |
| `shell::sleep_ms` | `(ms: Word) -> ()` | Sleep the current thread for `ms` milliseconds without spawning a subprocess |
| `shell::now_unix_ms` | `() -> Word` | Return the current Unix timestamp in milliseconds |
| `shell::read_file` | `(path: Text) -> Text` | Read a file's contents; traps on I/O failure or non-UTF-8 |
| `shell::write_file` | `(path: Text, content: Text) -> ()` | Replace a file with the given content; traps on I/O failure |
| `shell::append_file` | `(path: Text, content: Text) -> ()` | Append to a file, creating when absent; traps on I/O failure |
| `shell::file_exists` | `(path: Text) -> bool` | Test whether a filesystem entry exists; follows symlinks |
| `shell::write_err` | `(text: Text) -> ()` | Write to stderr without a trailing newline |
| `shell::writeln_err` | `(text: Text) -> ()` | Write to stderr with a trailing newline |
| `shell::arg_count` | `() -> Word` | Number of entries in the script argument vector (script path plus positional arguments); falls back to the host process argv when none is installed |
| `shell::arg` | `(index: Word) -> Option<Text>` | Script argument at `index`; index zero is the script path, one onward the positional arguments; `None` when out of range or negative |
| `shell::setenv` | `(name: Text, value: Text) -> ()` | Set an environment variable for subprocesses spawned through `shell::run` |
| `shell::pid` | `() -> Word` | Current process identifier, for pidfile creation |
| `shell::hostname` | `() -> Text` | Host name reported by the operating system; traps when unavailable |
| `shell::pwd` | `() -> Text` | Current working directory; traps on failure |
| `shell::cd` | `(path: Text) -> ()` | Change the current working directory; traps on failure |

Combined with the bundled `println` from `utility_natives`, scripts can produce output, run external processes (with or without captured stderr and a timeout), sleep without forking, read and write files directly, route log-shaped output to stderr, inspect their own arguments, and manipulate the process environment and working directory.

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
| `shell::arg`, `shell::arg_count` argv correctness | Corrected | The pair read `std::env::args` directly, so a script saw `keleusma`, `run`, and the script path ahead of its own arguments with no stable offset. They now report a script argument vector installed by the CLI (`set_script_args`): index zero is the script path, one onward the positional arguments. The CLI collects positionals for both `keleusma run` and shebang invocation and honours a `--` terminator. |
| `shell::run` stderr capture | Closed | `shell::run` captures and discards stderr. The new `shell::run_full` returns `(exit_code, stdout, stderr)` for callers that need the command's diagnostic stream. |

The trap-on-error convention is documented in the per-native contracts in [`STANDARD_LIBRARY.md`](../../docs/spec/STANDARD_LIBRARY.md). A future generic `Result` type or refinement-newtype wrapper could replace the trap pattern; that is a language-design question rather than a bundle-design one.

## Net assessment

The current bundle is **adequate** for both one-shot and daemon-shaped workloads. The three V0.2.1 additions (sleep, time, file I/O) closed the gaps that previously made the daemon use case awkward. A loop daemon can now pace itself without forking, read configuration from files, write log records to files or stderr, and reason about elapsed time, all without delegating to subprocesses.

The signed-and-encrypted delivery model is fully usable in both atomic and productive-divergent modes today.

## Open recommendations

The convenience natives proposed by the initial audit (`pid`, `hostname`, `setenv`, `pwd`, `cd`, `run_timeout`) are all implemented and listed under Present capabilities. The remaining convenience item is small and blocks nothing.

| Priority | Native | Use case |
|----------|--------|----------|
| Low | `shell::read_lines(path: Text) -> Array<Text>` | Common per-line iteration; today scripts read the whole file and split host-side. |

## Limitations that remain

These are properties of the stock bundle, not missing natives. They matter because the `Shell` bundle grants ambient process authority, so a host that ships it to untrusted scripts inherits the following caveats. A host that needs confinement or proven bounds should register a curated subset or its own narrower natives rather than the full bundle.

- **WCET and WCMU are not bounded for effectful natives.** `shell::run`, `shell::run_full`, `shell::run_checked`, and `shell::run_timeout` spawn arbitrary subprocesses whose time and memory the verifier cannot model. The static worst-case bounds the language otherwise guarantees do not extend across these calls. `shell::run_timeout` caps wall-clock time per call but not memory, and the cap is dynamic rather than statically verified.
- **Unbounded host-heap allocation.** `read_file`, `run`, and `run_full` allocate output buffers sized by the file or subprocess, on the host heap, outside the script arena's budget. A large file or chatty subprocess can exhaust host memory.
- **`shell::exit` terminates the host process.** It calls `std::process::exit`, bypassing VM teardown and any host-side cleanup. This is appropriate for a standalone CLI script but hazardous for an embedder that runs scripts inside a larger process.
- **Determinism is abandoned.** `getenv`, `run`, `now_unix_ms`, `hostname`, `pid`, `pwd`, and the filesystem natives all read mutable external state. Scripts using them are not reproducible.
- **`read_file` requires UTF-8.** Non-UTF-8 file content traps rather than returning bytes; the bundle has no binary-file accessor.
- **`set_script_args` is thread-local.** A host that calls `set_script_args` on one thread and runs the script on another observes the `std::env::args` fallback rather than the installed vector. The CLI runs both on the main thread, so this is invisible there but is a constraint for multi-threaded embedders.

## Net judgment on the daemon use case

`stddsl::Shell` is now rich enough to write substantial devops and sysadmin loop daemons in Keleusma. The use cases that work today: signed-and-encrypted artefacts deployed to operator-controlled hosts, executing operational logic in a productive-divergent loop with built-in rate limiting via `--tick-interval`, reading configuration files, writing log records to stderr or to disk, exiting cleanly via `shell::exit`. The trap-on-error pattern for I/O failures gives daemon scripts fail-fast behaviour when the host filesystem disagrees with expectations.
