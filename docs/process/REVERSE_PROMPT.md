# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-22
**Status**: V0.2.1 CLI follow-on complete. Three coordinated additions land alongside the prior tick-interval work. The branch carries one commit pending.

## Summary of work this session

Three CLI items from the prior session's deferred-work audit are addressed in a single commit.

### Yield-main runner

The CLI's loop runner gains a third entry shape, `yield main(tick: Word) -> Word`. The new `drive_yield_main` shares the tick-counter protocol with `drive_loop_main`. The distinction is termination: a yield-main script eventually returns instead of yielding, at which point the runner terminates cleanly and prints the returned value when non-Unit. The `--tick-interval` flag applies to yield-main entries too.

### Shell-audit critical natives

Eight new natives in `stddsl::Shell`:

| Native | Signature |
|--------|-----------|
| `shell::sleep_ms` | `(Word) -> ()` |
| `shell::now_unix_ms` | `() -> Word` |
| `shell::read_file` | `(Text) -> Text` |
| `shell::write_file` | `(Text, Text) -> ()` |
| `shell::append_file` | `(Text, Text) -> ()` |
| `shell::file_exists` | `(Text) -> bool` |
| `shell::write_err` | `(Text) -> ()` |
| `shell::writeln_err` | `(Text) -> ()` |

All file I/O traps on failure via `VmError::NativeError`, matching the existing `shell::run_checked` convention. Introducing a generic `Result<T, E>` type was rejected on scope grounds. Ten new unit tests cover the no-side-effect natives and a write-read-append round trip against a tempdir.

### Compile-time signature validation

The `stddsl::Shell::SIGNATURES` constant carries source-form `use` declarations for the thirteen bundle natives. The CLI's `CLI_NATIVE_SIGNATURES` adds two more for `shell::set_tick_interval` and `shell::tick_interval`. The CLI prepends both to every script source before parsing, so call-site type and arity mismatches surface at compile time rather than runtime.

Math and Audio bundle signatures are deferred because the auto-widening behaviour at the native boundary conflicts with strict signature checking. A script that writes `math::sin(1)` (Word literal) currently runs at runtime via auto-widening; introducing the signature `(Float) -> Float` would reject the call at compile time. Resolving the conflict is a language-design question rather than a bundle-design one.

### Verification

End-to-end integration test exercises all three features in concert: a yield-main script that calls six of the new shell natives runs cleanly under signature validation, with the runtime printing the terminal return value.

- `cargo test --workspace`: 853 main lib tests passing (10 new), 1032 across the workspace.
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.

## Known limitations

The compile-error line offset is the largest rough edge. Because the CLI prepends a sixteen-line preamble to every source, compile errors at line N in the CLI correspond to line N minus the preamble length in the user-visible source. Operators correlate by hand until span-offset correction lands. Documented in the CLI README.

The yield-main runner reuses the loop-main tick semantics. Scripts that want multiple yields express them inline with separate `yield` expressions in the body rather than through a tick-driven loop. The tick parameter carries the host-side counter; the script can ignore it or use it.

The signature preamble covers Shell and CLI tick-interval natives only. Math and Audio bundle natives retain the existing untyped behaviour because of the auto-widening conflict described above.

## Recommended next step

The work is ready for commit. The three pieces compose into a single feature commit because they share both the test surface and the operator-facing release note.

If the operator wants a longer-running session, the next adjacent piece is span-offset correction for the preamble. The mechanism would be a new lexer entry point that takes a (line, column) offset and applies it to every token's span, plus a corresponding adjustment in the error formatter. Approximately half a day's work; not on the critical path because operators can subtract the preamble length by inspection.

A second adjacent piece is the Math and Auto bundle signatures with a softer matching mode that admits Word where Float is expected at native call boundaries. This would close the last untyped corner of the standard library.
