# keleusma-cli

[![Crates.io](https://img.shields.io/crates/v/keleusma-cli.svg)](https://crates.io/crates/keleusma-cli)
[![Docs.rs](https://docs.rs/keleusma-cli/badge.svg)](https://docs.rs/keleusma-cli)
[![License: 0BSD](https://img.shields.io/badge/license-0BSD-blue.svg)](LICENSE)

Standalone command-line frontend for Keleusma. Provides a script runner, a bytecode compiler, and an interactive REPL so users can work with Keleusma scripts without writing any Rust host code.

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

The REPL wraps each expression input as `fn main() -> T { <expression> }` and tries common return types in order: `Word`, `Float`, `bool`, `Text`, `()`. The first type that type-checks is used. For more complex return types, declare a function explicitly and call it.

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

The first iteration of the CLI has the following limitations.

The runner does not yet drive `yield` and `resume` interactively. Stream-classified `main` functions are not directly runnable through the CLI. Use `fn main() -> T { ... }` for atomic-total entry points.

The REPL session prefix accumulates declarations across the session but does not persist data segment values. Any `data` block declared in the prefix is allocated freshly on each evaluation. Persistent state across REPL evaluations is future work.

The REPL's return-type inference tries a fixed list of types. Expressions whose type is outside the list (custom enums, structs, tuples) require explicit function wrapping. Inference of the expression type prior to wrapping is future work.

The compiler does not yet expose `Target` selection at the CLI level. All compiled output uses the host runtime's target. Cross-target compilation is future work.

## File Extensions

The convention is `.kel` for source files and `.kel.bin` for compiled bytecode. The compiler defaults to writing `<source>.kel.bin` when no `--output` is given.

## License

0BSD. Same as Keleusma.
