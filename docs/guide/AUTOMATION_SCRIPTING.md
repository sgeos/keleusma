# Keleusma as a Scripting and Automation Tool

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

This reference consolidates the operator-facing story for using the `keleusma` command-line tool to write devops and sysadmin automation. The numbered guide chapters teach the language; this document gathers the pieces that turn a script into a deployable command, a streaming filter, or a long-running daemon, and explains how to distribute that artefact as signed and optionally encrypted bytecode.

The material here is drawn together from several chapters and reference documents that each cover one facet. Where a topic has a fuller treatment elsewhere, this document links to it rather than restating it.

## Audience

Operators and script authors who want to run Keleusma programs as standalone tools rather than embed the runtime in a Rust host. Readers who want to embed the runtime should start at [Chapter 31](./31_embedding_orientation.md) instead.

## Three ways to run a script

A source script runs in three interchangeable forms. All three accept the same script arguments.

| Form | Command | Platform |
|------|---------|----------|
| Explicit subcommand | `keleusma run script.kel` | All |
| Extension shorthand | `keleusma script.kel` | All |
| Shebang executable | `./script.kel` after `chmod +x` | macOS and Linux |

The shebang form requires the first source line to read `#!/usr/bin/env keleusma`. The lexer skips that line while preserving source line numbers in diagnostics, so the same file remains compilable on Windows, where it runs through `keleusma run`. [Chapter 2](./02_installing_and_running.md) introduces the shebang at tutorial pace. A compiled bytecode file may also carry a shebang, covered in [Chapter 25](./25_from_source_to_bytecode.md).

### Script arguments

A script reads its own arguments through the `shell` bundle. `shell::arg(0)` returns the script path and `shell::arg(1)` onward the positional arguments the launcher passed after it, mirroring the `$0` and `$1` convention of POSIX shells. `shell::arg_count()` reports the number of entries, counting argument zero.

The CLI collects positional arguments for both `keleusma run script.kel a b c` and the shebang form `./script.kel a b c`. A `--` terminator marks the end of CLI options, after which every token is treated as a script argument even when it begins with a dash. An unrecognized leading-dash token before `--` is rejected as a flag typo rather than passed through.

```
./report.kel --since 2026-01-01 -- --raw
```

In that invocation the CLI consumes nothing it does not recognize, and the script receives `--since`, `2026-01-01`, and `--raw` as positional arguments one through three.

## Entry kinds map to deployment shapes

A script declares one of three entry kinds. The CLI inspects the compiled entry block and drives it accordingly. The entry kind is the script author's primary lever for the deployment shape. [Chapter 15](./15_three_function_categories.md), [Chapter 16](./16_yield.md), and [Chapter 17](./17_loop_function.md) cover the language semantics; the table below maps each to its operational role.

| Entry kind | Declaration | Termination | Operational shape |
|------------|-------------|-------------|-------------------|
| Atomic | `fn main() -> Word` | Runs to completion in one call | One-shot command. Suited to cron jobs, build steps, and manual invocation. |
| Staged | `yield main(tick: Word) -> Word` | Returns rather than yields | Cooperative task that pauses and resumes across ticks, then finishes. |
| Daemon | `loop main(tick: Word) -> Word` | Only on `shell::exit` or a termination signal | Long-running service. Returning is treated as an error. |

A few operational consequences follow from how the CLI drives each kind.

- An atomic `fn main` has its returned value printed to standard output. The process exit status is not taken from the return value. A script that needs a specific exit code calls `shell::exit(code)`. The repository link-checker uses exactly this pattern.
- A `loop main` daemon is rate-limited with `--tick-interval`, which accepts humanized durations such as `100ms`, `1s`, `1m`, `1h`, `1d`, and `1w`, up to a maximum of four weeks. The runtime is genuinely idle between iterations, so a long-cadence daemon costs page-fault avoidance rather than computation. See [`METRICS.md`](./METRICS.md) for the memory-residency analysis and [`SECURITY_POLICY.md`](./SECURITY_POLICY.md) for the operator guide to daemon cadences.

## Delegating work through the shell bundle

Without host-registered natives the language admits only pure total functions and the productive-divergent loop. The CLI registers the `shell` bundle, which is what makes orchestration possible. A script delegates work to ordinary command-line programs, branches on the `Word` exit code each returns, accumulates across the run in a mutable `private data` segment, and sets its own process exit status.

| Native | Purpose |
|--------|---------|
| `shell::run(cmd) -> (Word, Text)` | Run a command through `sh -c`; returns exit code and stdout. Stderr is discarded. |
| `shell::run_full(cmd) -> (Word, Text, Text)` | As above, returning exit code, stdout, and stderr. |
| `shell::run_checked(cmd) -> Text` | Run a command; trap on a non-zero exit. |
| `shell::run_timeout(cmd, ms) -> (Word, Text)` | Run a command with a wall-clock deadline. |
| `shell::read_file`, `shell::write_file`, `shell::append_file` | File input and output. |
| `shell::writeln_err`, `shell::write_err` | Log-shaped output to stderr. |
| `shell::arg`, `shell::arg_count` | The script's own arguments. |
| `shell::exit(code)` | Terminate the process with an exit status. |

The full list, signatures, and per-native contracts are in [`STANDARD_LIBRARY.md`](../spec/STANDARD_LIBRARY.md). The bundle's capability assessment and its standing limitations are in [`SHELL_AUDIT.md`](./SHELL_AUDIT.md).

### Worked example: the Markdown link-checker

The repository's own Markdown link-checker, [`scripts/check-md-links.kel`](../../scripts/check-md-links.kel), is a complete worked example of the orchestrator pattern and runs in continuous integration. It delegates the partial text-scanning work to POSIX tools through `shell::run`, drives control flow on the returned `Word`, accumulates failures in a `private data` segment described in [Chapter 18](./18_data_segment.md), and propagates the result through `shell::exit`. The constructs that make the orchestration total, the partial-operation family, are covered in [Chapter 23](./23_big_numbers.md).

## Distributing scripts as signed and encrypted bytecode

A finished script can be compiled to bytecode and delivered as a tamper-evident, optionally confidential artefact. The two policies are independent. Neither, signing only, encryption only, or both may be active.

| Step | Command |
|------|---------|
| Generate a signing keypair | `keleusma keygen --seed sign.seed --public sign.pub` |
| Generate an encryption keypair | `keleusma keygen --kind encryption --seed dest.seed --public dest.pub` |
| Compile, sign, and encrypt | `keleusma compile script.kel --signing-key sign.seed --encryption-key dest.pub -o script.kel.bin` |
| Run, verifying and decrypting | `keleusma run script.kel.bin --verifying-key sign.pub --decryption-key dest.seed` |

Signing requires the entry function to carry the `signed` modifier; otherwise the toolchain produces unsigned bytecode and refuses the signing key. Encryption uses X25519 key agreement so the artefact is sealed to a recipient's public key and opened with the matching private seed. The signing and encryption design is covered in [Chapter 26](./26_signed_modules_and_hot_swap.md) and the wire format in [`WIRE_FORMAT.md`](../spec/WIRE_FORMAT.md).

### Strict-mode key stores

On a managed host the trust decision is taken out of the operator's hands. In strict signing mode the CLI loads trusted public keys from a system directory and refuses source files, unsigned bytecode, and bytecode signed by keys outside the store. The `--verifying-key` argument is rejected so an unprivileged operator cannot relax the policy. Strict encryption mode behaves analogously for the decryption-key store. The directories, environment variables, and threat model are documented in [`SECURITY_POLICY.md`](./SECURITY_POLICY.md).

A signed and encrypted bytecode artefact that also carries a shebang is directly executable through the operating system shell while remaining tamper-evident and confidential. This is the distributable-runbook delivery shape, described for courier-delivered media in [`SECURITY_POLICY.md`](./SECURITY_POLICY.md).

## Running several scripts under supervision

For workloads that need more than one script, the `run-tasks` subcommand drives a set of scripts from a TOML manifest through a cooperative scheduler with an event queue, supervised restart, and per-task signing and encryption policy. It lifts the cooperative-RTOS pattern from [`examples/rtos/`](../../examples/rtos/) onto the desktop and server.

```
keleusma run-tasks fleet.toml --quiet
```

The manifest declares a scheduler tick interval, a `[[task]]` table for each script with a name, a bytecode path, and a restart policy of `never`, `on_error`, or `always`, and an optional event table. The manifest format, validation rules, and scheduler semantics are documented in [`RUN_TASKS.md`](../architecture/RUN_TASKS.md).

## Where the static guarantees do not hold

The language otherwise rejects programs whose worst-case execution time or memory cannot be statically bounded. The orchestration natives are an explicit boundary where that guarantee does not extend.

- A subprocess spawned through `shell::run` and its siblings has time and memory the verifier cannot model. The static worst-case bounds do not cross these calls. `shell::run_timeout` caps wall-clock time per call but not memory.
- `shell::read_file` and the run natives allocate output buffers on the host heap, outside the script arena's budget.
- The orchestration natives read mutable external state, so a script that uses them is not reproducible.
- `shell::exit` terminates the host process directly.

These properties belong to the ambient-authority `shell` bundle, not to the language. A deployment that needs confinement or proven bounds registers a curated subset or its own narrower natives in a custom host rather than shipping the full bundle to untrusted scripts. The complete limitation list is in [`SHELL_AUDIT.md`](./SHELL_AUDIT.md).

## Related reading

- [Chapter 2: Installing Keleusma and the Interactive Prompt](./02_installing_and_running.md)
- [Chapter 15: The Three Function Categories](./15_three_function_categories.md), [Chapter 16: Yield](./16_yield.md), [Chapter 17: The loop Function](./17_loop_function.md)
- [Chapter 18: The Data Segment](./18_data_segment.md)
- [Chapter 25: From Source to Bytecode](./25_from_source_to_bytecode.md), [Chapter 26: Signed Modules and Hot Code Swap](./26_signed_modules_and_hot_swap.md)
- [`SHELL_AUDIT.md`](./SHELL_AUDIT.md), [`SECURITY_POLICY.md`](./SECURITY_POLICY.md), [`METRICS.md`](./METRICS.md)
- [`STANDARD_LIBRARY.md`](../spec/STANDARD_LIBRARY.md), [`RUN_TASKS.md`](../architecture/RUN_TASKS.md)
