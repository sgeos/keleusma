# Changelog

All notable changes to `keleusma-cli` will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-05-21

First publicly released line. V0.1.x circulated as a pre-release alongside the parent `keleusma` crate. The crate version is locked one-to-one with the major-minor of `keleusma`.

### Added

- `keleusma run <path>` subcommand. Runs a script. Auto-detects source versus precompiled bytecode by inspecting the file contents (`KELE` magic at offset zero, or after a `#!...\n` shebang envelope). Source files compile through the full pipeline; bytecode files load through `Vm::load_bytes` or `Vm::load_signed_bytes`.
- `keleusma compile <path> -o <output>` subcommand. Compiles a source script to bytecode and writes the framed wire-format buffer to the output path.
- `--signing-key <path>` flag on `compile`. Signs the emitted bytecode with the Ed25519 seed at the given path. Requires that the entry function carries the `signed` modifier. The file format is a 32-byte raw Ed25519 seed.
- `--verifying-key <path>` flag on `run`. Repeatable. Adds the public key at the given path to the trust matrix consulted by `Vm::load_signed_bytes`. The file format is a 32-byte raw Ed25519 public key. Run rejects modules that carry `FLAG_REQUIRES_SIGNATURE` without a matching key in the matrix.
- `keleusma keygen --seed <seed-path> --public <public-path>` subcommand. Generates a fresh Ed25519 keypair. The seed file is written with `0o600` permissions on Unix. Existing files are not overwritten.
- `keleusma repl` subcommand. Interactive Read-Eval-Print Loop with line history and arrow-key cursor movement.
- Shorthand entry point. `keleusma <path>` is equivalent to `keleusma run <path>` so scripts can be executed without a subcommand prefix.
- Shebang execution support. Scripts and bytecode files that start with `#!/usr/bin/env keleusma` (or any `#!...\n` line) are admitted; the lexer and the bytecode framer skip the envelope.

### Dependencies

- `keleusma` runtime with the `shell` and `signatures` features enabled. The `shell` feature pulls the bundled `stddsl::Shell` library so the CLI's REPL and runner support `shell::getenv`, `shell::run`, and similar host-side natives. The `signatures` feature enables Ed25519 signing and verification at load time.
- `keleusma-arena` (0.3 or compatible) for the arena substrate.
- `ed25519-dalek 2` and `rand_core` for key generation. `getrandom` provides the system entropy source.

### Installation

```sh
cargo install keleusma-cli
```

Installs the `keleusma` binary to the Cargo bin directory. The binary name is `keleusma`; the crate name `keleusma-cli` exists only because `cargo install` resolves crate names against the registry.

### Notes

- The CLI is intended for development workflows: ad-hoc script execution, signed-bytecode production, and REPL exploration. Production embedders typically depend on the `keleusma` runtime crate directly rather than shipping the CLI.

### Licensed

- BSD Zero Clause License (`0BSD`).
