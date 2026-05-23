# StdDSL::Shell Audit

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

Assessment of the V0.2.1 `stddsl::Shell` bundle against typical devops and sysadmin workloads delivered as signed-and-encrypted `loop main` scripts. Identifies present capabilities, missing capabilities, and recommended additions.

## Present capabilities

The bundle ships five natives, registered through `vm.register_library(stddsl::Shell)`.

| Native | Signature | Purpose |
|--------|-----------|---------|
| `shell::getenv` | `(name: Text) -> Option<Text>` | Read an environment variable; `Some(value)` or `None` |
| `shell::has_env` | `(name: Text) -> bool` | Test whether an environment variable is set |
| `shell::run` | `(cmd: Text) -> (Word, Text)` | Execute `cmd` through `sh -c`; returns `(exit_code, stdout)`. Non-zero exit code is not an error |
| `shell::run_checked` | `(cmd: Text) -> Text` | Execute `cmd` through `sh -c`; returns stdout. Non-zero exit code surfaces as `NativeError` |
| `shell::exit` | `(code: Word) -> ()` | Terminate the host process with the given exit code |

The five natives cover the core operations a script needs to drive shell command execution, read configuration from environment variables, and terminate cleanly. Combined with the bundled `println` from `utility_natives`, scripts can produce output, run external processes, and exit.

## Capability gaps for devops and sysadmin loop daemons

The bundle as shipped is adequate for many one-shot tasks. For long-running daemon-shaped workloads (loop main with tick-counter convention), several common idioms are missing.

### Critical for daemon operation

**Sleep or delay.** A loop daemon without sleep spins at maximum CPU. Currently the only way to pace iterations is to delegate to `shell::run("sleep 1")` which spawns a subprocess per iteration. This is operationally expensive (one fork plus one exec per tick).

```
Recommended: shell::sleep_ms(milliseconds: Word) -> ()
```

Without this, a loop daemon either consumes 100 percent CPU or pays the subprocess overhead.

**Current time.** Scripts need to read the current time to make tick-rate decisions, generate timestamps for log records, or schedule actions. Currently no native exists.

```
Recommended: shell::now_unix_ms() -> Word
```

Returns the Unix timestamp in milliseconds. The script computes elapsed time by subtracting two readings.

**Write to stdout and stderr.** Currently scripts use `println` from utility_natives which writes a single value to stdout with a trailing newline. There is no way to write to stderr, and no way to write multiple values inline.

```
Recommended:
  shell::write(text: Text) -> ()
  shell::write_err(text: Text) -> ()
  shell::writeln(text: Text) -> ()
  shell::writeln_err(text: Text) -> ()
```

The `_err` variants route to stderr, which is essential for log-shaped output that should not be confused with the script's data output.

### Important for sysadmin scripts

**File existence check.** Scripts often need to check whether a file exists before reading it. Currently the only mechanism is `shell::run_checked("test -f path")` which is operationally wasteful.

```
Recommended: shell::file_exists(path: Text) -> bool
```

**Read file content.** Reading a config file or log line. Currently requires shelling out to `cat`.

```
Recommended:
  shell::read_file(path: Text) -> Result<Text>
  shell::read_lines(path: Text) -> Result<Array<Text>>
```

Returns either the file content or an error variant. `read_lines` for the common per-line iteration case.

**Write to file.** Logging or state persistence. Currently requires shelling out to `tee` or similar.

```
Recommended:
  shell::write_file(path: Text, content: Text) -> Result<()>
  shell::append_file(path: Text, content: Text) -> Result<()>
```

The `append` variant is the canonical log-writing path.

**Command-line argument access.** Scripts often need to inspect their own argv to alter behaviour by invocation. Currently no mechanism.

```
Recommended:
  shell::arg_count() -> Word
  shell::arg(index: Word) -> Option<Text>
```

The strict-mode CLI rejects bytecode that fails the signing policy, so command-line arguments are operator-trusted; exposing them to the script is safe.

**Process information.** Daemon scripts may need to know their own PID for logging or pidfile creation.

```
Recommended: shell::pid() -> Word
```

**Hostname.** Multi-host deployments often vary behaviour by hostname.

```
Recommended: shell::hostname() -> Text
```

### Convenience but not critical

**Set environment variable.** Some scripts need to set env vars before spawning subprocesses through `shell::run`. Currently the only path is the subprocess inheriting the parent CLI's env.

```
Recommended: shell::setenv(name: Text, value: Text) -> ()
```

**Change directory.** For scripts that operate on a specific working directory.

```
Recommended:
  shell::pwd() -> Text
  shell::cd(path: Text) -> Result<()>
```

**Run with timeout.** Subprocess execution with a maximum wall-clock duration.

```
Recommended: shell::run_timeout(cmd: Text, ms: Word) -> Result<(Word, Text)>
```

## Net assessment

The current bundle is **insufficient** for typical devops and sysadmin daemon workloads in three specific ways:

1. **Sleep is missing.** A loop daemon cannot pace itself without spawning a subprocess per iteration.
2. **Time is missing.** Scripts cannot reason about elapsed time without external help.
3. **File I/O is missing.** Reading configuration, writing logs, and checking file existence all require subprocess delegation.

These three gaps make the current bundle awkward for the daemon use case the CLI loop runner is designed to support. A script can technically work around all three by going through `shell::run`, but the overhead is substantial and the syntax is ugly.

The bundle is **adequate** for one-shot script workloads (atomic fn main): read env vars, run a command, exit. The signed-and-encrypted delivery model is fully usable in that mode today.

## Recommendation

Three priority additions for V0.2.x:

1. `shell::sleep_ms(milliseconds: Word) -> ()`
2. `shell::now_unix_ms() -> Word`
3. `shell::write_file(path: Text, content: Text) -> Result<()>` and `shell::append_file(path: Text, content: Text) -> Result<()>` and `shell::read_file(path: Text) -> Result<Text>`

These three close the daemon-workload gaps. Estimated effort: half a day for all three, including tests.

Two secondary additions worth doing:

4. `shell::write_err(text: Text) -> ()` and `shell::writeln_err(text: Text) -> ()` for stderr output.
5. `shell::file_exists(path: Text) -> bool` for the common existence-check pattern.

The remaining items (pid, hostname, args, setenv, cd, pwd, run_timeout) are nice-to-have but not essential. They can ship in V0.2.x point releases if operator demand surfaces.

## Net judgment on the daemon use case

After the three priority additions, `stddsl::Shell` is rich enough to write substantial devops and sysadmin loop daemons in Keleusma. The use cases that work today: signed-and-encrypted artefacts deployed to operator-controlled hosts, executing operational logic in a productive-divergent loop, exiting cleanly via `shell::exit`. The use cases that need the additions: anything that paces itself, anything that touches the filesystem directly, anything that logs to stderr.

This audit is informational. No changes were made to the bundle as part of this report. The recommendations are for future work.
