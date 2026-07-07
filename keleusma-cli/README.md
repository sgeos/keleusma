# keleusma-cli

[![Crates.io](https://img.shields.io/crates/v/keleusma-cli.svg)](https://crates.io/crates/keleusma-cli)
[![Docs.rs](https://docs.rs/keleusma-cli/badge.svg)](https://docs.rs/keleusma-cli)
[![License: 0BSD](https://img.shields.io/badge/license-0BSD-blue.svg)](LICENSE)

Standalone command-line frontend for Keleusma. Provides a script runner, a bytecode compiler, and an interactive REPL so users can work with Keleusma scripts without writing any Rust host code.

If the CLI runner does not do what you need, write your own host using the `keleusma` library directly. The runtime is the product; this CLI is one example of how to embed it. The library API is stable and well-documented; a custom host can constrain or extend any aspect of the CLI's behaviour (signing policy, encryption gates, native function registration, loop runner semantics, deployment shape).

## Installation

```sh
cargo install --path . --bin keleusma
```

This installs the `keleusma` binary to your Cargo bin directory. Verify with:

```sh
keleusma --help
```

## Usage

### Run a script

```sh
keleusma run hello.kel
```

Or as a shorthand, any first argument that names an existing file is treated as a script to run:

```sh
keleusma hello.kel
```

The runner detects whether the file is source or compiled bytecode by inspecting the first bytes (after any shebang envelope). Source files are parsed, compiled, verified, and executed through the default safe constructor. Bytecode files load through `Vm::load_bytes`. Utility and math natives are pre-registered. The script's `main` function is called with no arguments. If `main` returns a value, the value is printed.

### Productive-divergent loop runner

When the entry point is declared as `loop main(tick: Word) -> Word`, the runner drives the script through the tick-counter convention. The host passes `tick = 1` on first call. The script yields a `Word` value each iteration. The host computes the next tick as `yielded.wrapping_add(1)` and resumes. Yield `0` produces next tick `1` (a reset convention). Yield `Word::MAX` wraps to `Word::MIN` (overflow indicator under signed arithmetic). Termination occurs through `shell::exit(code)` or `SIGINT`.

The `--tick-interval <duration>` flag rate-limits the loop. The flag accepts humanized durations:

| Form | Meaning |
|------|---------|
| `Nms` | milliseconds |
| `Ns`  | seconds |
| `Nm`  | minutes |
| `Nh`  | hours |
| `Nd`  | days |
| `Nw`  | weeks |

Composite forms such as `1h30m` are not accepted. Operators should express composite durations as a single unit (express `1h30m` as `90m`). Maximum admitted interval is four weeks. Longer cadences should use an external scheduler (cron, systemd timers) or noop yield cycles in the script that count internal ticks against the longer interval.

Drift is compensated. After each iteration the runner sleeps for `max(0, interval - iteration_elapsed)` so the average cadence approaches the configured interval. When an iteration exceeds the interval, the runner emits a warning on stderr naming both values and resumes immediately without sleep. The `--quiet` flag suppresses the warning.

```sh
# Run a loop daemon at one tick per second.
keleusma run watchdog.kel --tick-interval 1s

# Run a daily-cadence cleanup script; suppress overrun warnings.
keleusma run nightly.kel --tick-interval 1d --quiet
```

A script may set the interval from inside the loop through the `shell::set_tick_interval(duration: Text) -> ()` native. The complementary `shell::tick_interval() -> Text` getter returns the current value as a humanized string. Both natives share state with the CLI flag; either path drives the same atomic.

```keleusma
use shell::set_tick_interval
use shell::tick_interval
use shell::exit
use println

loop main(tick: Word) -> Word {
    // Set the cadence at the top of the script so a malformed
    // duration fails fast at the first iteration. A failure
    // surfaces as a runtime error and terminates the daemon.
    let _ = shell::set_tick_interval("100ms");
    println(shell::tick_interval());
    let _ = if tick >= 10 { shell::exit(0); };
    let _ = yield tick;
    tick
}
```

The runner does not enforce a minimum interval. Operators who need spin-wait semantics (zero sleep between iterations) should write their own host using the `keleusma` library directly. The default zero-interval behaviour spins the loop as fast as the script yields, which is appropriate for batch-shaped workloads but not for long-lived daemons. The CLI loop runner is one example of an embedding; bespoke deployments often want their own scheduler integration.

### Multi-script runner

The `run-tasks` subcommand drives several scripts under one cooperative scheduler from a TOML manifest. Use when the deployment is a multi-daemon workload (sensor poller plus log writer plus watchdog) and the operator wants shared state, supervised restart, and an event queue without writing a custom Rust host.

```sh
keleusma run-tasks /etc/keleusma/tasks.toml [--quiet]
```

Each task is a `loop main(wakeup_reason: Word) -> (Word, Word)` script that yields a `(reason, payload)` tuple. The reason codes are `0` Wait until milliseconds, `1` EventWait on an id, `2` voluntary Yield, `3` Periodic (cadence from the manifest). On resume the task receives a wakeup-reason word identifying why it woke (first call, deadline, event, or voluntary yield).

The scheduler registers six kernel natives on every task: `kernel::post_event(id, payload)` (post into the shared event queue), `kernel::last_event_id` and `kernel::last_event_payload` (read the metadata of the event that woke this task), `kernel::now_ms` (monotonic clock from scheduler start), `kernel::task_id` and `kernel::task_name` (identification). The standard `println`, Math, Audio, and Shell bundles are also registered.

A minimal manifest declares one task and accepts defaults for everything else.

```toml
[[task]]
name = "hello"
bytecode = "hello.kel.bin"
period = "1s"
restart = "on_error"
```

A representative production manifest declares the scheduler-wide knobs, multiple tasks with per-task policy, and named events the manifest binds to numeric ids.

```toml
[scheduler]
tick_interval = "10ms"
shutdown_grace = "5s"

[events]
data_ready = 1
shutdown_requested = 99

[[task]]
name = "sensor_poller"
bytecode = "tasks/sensor_poller.kel.bin"
period = "100ms"
restart = "on_error"

[[task]]
name = "log_writer"
bytecode = "tasks/log_writer.kel.bin"
restart = "always"
# No period; the script waits on the data_ready event via `yield (1, 1)`.
```

The runner prints WCET and WCMU bounds for each task at startup so operators have verification evidence in the deployment log without an extra step. Signing and encryption gates apply per task; each bytecode artefact passes through the same policy checks as `keleusma run`.

POSIX signals are honoured: SIGINT and SIGTERM begin a graceful drain with the manifest's `shutdown_grace` window, SIGHUP is reserved for future configuration reload. The runner returns conventional exit codes: 0 for natural shutdown, 130 for SIGINT clean drain, 143 for SIGTERM clean drain, 1 for manifest or task-load failure.

Designed for deployment by root on critical hardware where persistent memory residence is a feature. One process holds every task; per-task arenas are sized at startup and never re-allocated; the scheduler's main loop calls no heap allocator during steady state. Operators deploying under systemd, OpenRC, runit, FreeBSD rc.d, OpenBSD rc.d, launchd, or NSSM on Windows should consult [`docs/architecture/RUN_TASKS.md`](../docs/architecture/RUN_TASKS.md) for per-platform recipes including the `NOTIFY_SOCKET` integration the runner detects automatically.

### Shebang scripts

Both source and compiled bytecode can be Unix-executable through a shebang line.

```keleusma
#!/usr/bin/env keleusma
fn main() -> Word { 42 }
```

Mark executable and invoke directly:

```sh
chmod +x my_script
./my_script
```

The lexer skips a leading `#!` line in source. The bytecode loader strips a leading `#!...\n` envelope before validating magic and CRC, so a file produced by `cat <(printf '#!/usr/bin/env keleusma\n') script.kel.bin` is also directly executable. The CRC trailer covers only the post-strip range; the envelope is not part of the signed payload.

### Compile a script to bytecode

```sh
keleusma compile hello.kel -o hello.kel.bin
```

The compiler runs the full compile pipeline including type checking, monomorphization, and bytecode emission. The resulting bytecode is serialized through the standard wire format with framing, length, target widths, and CRC trailer. A host loading the bytecode through `Vm::load_bytes` validates the framing and re-runs structural verification.

### Generate a keypair

```sh
# Ed25519 signing keypair (default).
keleusma keygen --seed sign.seed --public sign.pub

# X25519 encryption keypair.
keleusma keygen --kind encryption --seed enc.seed --public enc.pub
```

Writes a fresh 32-byte seed to one file and the matching 32-byte public key to another. The `--kind` flag selects between `signing` (Ed25519, default) and `encryption` (X25519). On Unix the seed file is created with mode `0o600`. Existing files are not overwritten; the command refuses with a diagnostic naming the offending path so an accidental rerun cannot destroy a long-lived key identity. The seed is the private secret; treat it as a credential. The public key is freely distributable.

### Cross-target compilation

The `compile` subcommand accepts `--target <name>` to compile against a specific target descriptor rather than the host runtime's default. The target controls word, address, and float widths and validates the program against the chosen configuration.

| Name | Word | Address | Float |
|------|------|---------|-------|
| `host` (default) | 64-bit | 64-bit | binary64 |
| `wasm32` | 32-bit | 32-bit | binary64 |
| `embedded_32` | 32-bit | 32-bit | binary32 |
| `embedded_16` | 16-bit | 16-bit | binary32 |
| `embedded_8` | 8-bit | 16-bit | binary32 |

```sh
# Build for a 16-bit microcontroller target.
keleusma compile sensor.kel --target embedded_16 -o sensor.kel.bin
```

Programs that use literals or constants outside the target's representable range are rejected at compile time. The validation runs before bytecode emission so the resulting artefact is guaranteed loadable on a runtime built for the same target.

### Sign a compiled module

```sh
keleusma compile hello.kel --signing-key sign.seed -o hello.kel.bin
```

Produces signed bytecode when the source declares the entry function with the `signed` modifier (`signed fn main`, `signed yield main`, `signed loop main`). The compiler emits `FLAG_REQUIRES_SIGNATURE` in the header and the signer appends an Ed25519 signature. Without the `signed` modifier on the entry, the bytecode is unsigned even when `--signing-key` is supplied.

### Verify and run a signed module

```sh
keleusma run hello.kel.bin --verifying-key sign.pub
```

The `--verifying-key` flag is repeatable; each appearance adds a 32-byte Ed25519 public key to the runtime's trust matrix. Signed bytecode loads only when its signature verifies against at least one registered key. Loading unsigned bytecode with `--verifying-key` is an error to prevent silent acceptance of an unverified payload.

### Encrypt and run an encrypted module

```sh
# Compile, sign, AND encrypt to a specific destination workstation.
keleusma compile hello.kel \
    --signing-key sign.seed \
    --encryption-key destination.pub \
    -o hello.kel.bin

# Run the encrypted artefact on the destination workstation.
keleusma run hello.kel.bin \
    --verifying-key sign.pub \
    --decryption-key destination.seed
```

Encrypted artefacts use X25519 key agreement against the destination's public key, HKDF-SHA-256 key derivation, and AES-256-GCM authenticated encryption of the body. The Ed25519 signature covers the encrypted payload so an adversary cannot strip the encryption layer and substitute cleartext. Per-recipient asymmetric keys give compromise containment: a captured workstation reveals only its own private key.

Encryption requires signing because the wire format ties the two together. The `--encryption-key` flag requires `--signing-key` to be supplied alongside.

### Strict mode

Two independent strict-mode policies enforce host-managed key stores. Either may be active in any combination.

**Strict signing.** Place 32-byte Ed25519 public keys as `*.pub` files in one of the following:

- The directory named by `KELEUSMA_TRUSTED_KEYS_DIR`.
- `/etc/keleusma/trusted_keys` on Unix.
- `%PROGRAMDATA%\keleusma\trusted_keys` on Windows.

In strict signing mode, the CLI rejects source files, unsigned bytecode, and bytecode signed by keys not in the trust store. The `--verifying-key` argument is rejected. Set `KELEUSMA_REQUIRE_SIGNED=1` to force strict mode even with an empty trust store (fail-closed for everything).

**Strict encryption.** Place 32-byte X25519 private keys as `*.seed` files in one of:

- The directory named by `KELEUSMA_DECRYPTION_KEYS_DIR`.
- `/etc/keleusma/decryption_keys` on Unix.
- `%PROGRAMDATA%\keleusma\decryption_keys` on Windows.

In strict encryption mode, the CLI rejects unencrypted bytecode and artefacts encrypted to non-enrolled recipients. The `--decryption-key` argument is rejected. Set `KELEUSMA_REQUIRE_ENCRYPTED=1` to force strict mode.

The two policies are independent: neither, signing only, encryption only, or both may be active. See [`docs/guide/SECURITY_POLICY.md`](../docs/guide/SECURITY_POLICY.md) for the full operator guide and deployment scenarios.

Reference design records: [R42 in RESOLVED.md](../docs/decisions/RESOLVED.md) (signing infrastructure), [R49](../docs/decisions/RESOLVED.md) (strict-mode signing gate), [R50](../docs/decisions/RESOLVED.md) (encryption layer).

### Start the REPL

```sh
keleusma repl
```

The REPL accepts expressions and definitions interactively. Type an expression to evaluate it and see the result. Type a function, struct, enum, or trait declaration to add it to the session prefix. The session prefix accumulates across the REPL session; each evaluation runs against the current prefix.

REPL commands:

- `:help` shows the command list.
- `:quit` exits.
- `:reset` clears the session prefix.
- `:show` displays the current session prefix.

Example session:

```
$ keleusma repl
Keleusma REPL. Type :help for commands, :quit to exit.
> 1 + 2
3
> fn double(x: Word) -> Word { x + x }
defined: double
> double(21)
42
> :quit
```

The REPL wraps each expression input through the bundled `println` native so the value renders through the CLI's recursive value formatter. Primitives print as themselves (`42`, `1.5`, `true`, `"hello"`). Composite values format readably without the underlying `Debug` impl's wrapper noise: `Some(99)` instead of `Enum { type_name: "Option", variant: "Some", fields: [Int(99)] }`, `(1, 2, 3)` instead of `Tuple([Int(1), Int(2), Int(3)])`, `Red` instead of the enum-with-variant noise. Any type the bundled natives can produce will render through the formatter.

## Example programs

The Keleusma repository ships several embedded host examples that exercise the runtime through Rust applications. The CLI is a convenience for running standalone scripts; the example programs show what an embedder can build.

**Rogue** is the headline example. A complete roguelike video game with SDL3 graphics, dungeon generation, eight artificial-intelligence archetypes for the monsters, combat resolution, and item-effect scripts. Nineteen Keleusma scripts drive the gameplay logic; the Rust host handles rendering, input, and audio. The example demonstrates how a non-trivial application is structured around the Keleusma scripting layer with hot code reloading. To run it:

```sh
cargo run --release --example rogue --features sdl3-example
```

See [`docs/guide/ROGUE.md`](../docs/guide/ROGUE.md) for the long-form companion manual covering gameplay rules, the host-and-twelve-script architecture, the dungeon generator, and the artificial-intelligence archetypes.

Other notable examples:

- **piano_roll** (`cargo run --release --example piano_roll --features sdl3-example`): an SDL3 audio synthesizer driven by Keleusma scripts. Hot-swaps songs across a roster while playback continues. Companion manual at [`docs/guide/PIANO_ROLL.md`](../docs/guide/PIANO_ROLL.md).
- **rtos** (`cd examples/rtos && cargo run --release --bin three-task-std`): a cooperative real-time microkernel. Standalone host binary plus an STM32N6570-DK target for embedded execution. Companion manual at [`examples/rtos/MANUAL.md`](../examples/rtos/MANUAL.md).
- Standalone `.kel` scripts under [`examples/scripts/`](../examples/scripts/) demonstrate the language features in isolation.

## Limitations

The current CLI has the following limitations.

The runner supports all three entry shapes. The atomic-total form `fn main() -> T { ... }` runs to completion in a single call. The non-atomic total form `yield main(tick: Word) -> Word { ... }` and the productive-divergent form `loop main(tick: Word) -> Word { ... }` are both driven through the tick-counter convention with optional rate limiting via `--tick-interval`. The distinction is termination: a `yield main` script eventually returns instead of yielding, at which point the runner terminates cleanly and prints the returned value when non-Unit; a `loop main` script never returns, and the runner only stops on `shell::exit(code)` or `SIGINT`.

The REPL handles arbitrary expression types through a `println` wrapper that routes the value through the CLI's recursive formatter. Primitives print directly (`42`, `1.5`, `true`); composite types format readably (`Some(99)`, `(1, 2, 3)`, `Red`). `const data` declarations persist across evaluations because their values are baked into the bytecode. `shared data` blocks also persist across evaluations: the REPL snapshots every shared slot's value after each run and restores it before the next, using the same `Vm::set_data` and `Vm::get_data` interfaces the host API exposes. `Value::KStr` entries are materialised to `Value::StaticStr` before snapshot so the saved values do not carry stale arena references. Slot indices are stable because the REPL prefix is append-only, so a slot declared in eval N keeps its value through every later eval. `private data` blocks remain eval-local; persisting them would require a deep-clone API the VM does not yet expose, and is deferred. Mutating shared state uses normal assignment syntax (`state.count = state.count + 1`); the REPL detects statement-shaped input that cannot be wrapped as an expression and falls through to a statement wrapper automatically. A `shared data` field declared without an initializer starts as `Value::Unit` (the zero-byte default of the underlying enum), so the first read must follow an explicit write; this is a V0.2.x semantics property, not a REPL constraint.

The CLI prepends a fixed preamble of `use` declarations to every compiled source so the Math, Audio, and Shell bundles, plus the CLI-specific tick-interval natives, are validated at compile time. The preamble's line count is subtracted from reported error positions so operators see line numbers in the user-visible source. Errors that fall inside the preamble window are reported with a `[preamble line N]` marker. Word arguments unify with Float parameters at native call boundaries, so `math::sin(1)` works even though the signature is `(Float) -> Float`.

## File Extensions

The convention is `.kel` for source files and `.kel.bin` for compiled bytecode. The compiler defaults to writing `<source>.kel.bin` when no `--output` is given.

## License

0BSD. Same as Keleusma.
