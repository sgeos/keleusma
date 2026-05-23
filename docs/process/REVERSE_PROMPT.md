# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-22
**Status**: V0.2.1 CLI tick-interval feature complete. The productive-divergent loop runner now supports drift-compensated rate limiting through three coordinated surfaces (CLI flag, script-side setter native, getter native). Loop runner enhancements ready for commit alongside the prior V0.2.1 signing-and-encryption work.

## Summary of work this session

The session implemented the tick-interval feature for the CLI loop runner.

### Implementation

Three coordinated surfaces share the same `Arc<AtomicU64>` for the tick-interval nanoseconds.

1. **CLI flags**. `--tick-interval <duration>` and `--quiet` added to the `run` subcommand. The duration string is parsed through the new `keleusma-cli/src/duration.rs` module, which accepts humanized formats (`Nms`, `Ns`, `Nm`, `Nh`, `Nd`, `Nw`) with a four-week maximum and rejects composite forms such as `1h30m`. The flag value is stored on a `LoopRunnerConfig` struct that flows from `run_subcommand` through `run_file` to `execute_bytecode` and `execute_source`.

2. **Script-side natives**. `shell::set_tick_interval(duration: Text) -> ()` and `shell::tick_interval() -> Text` registered through `vm.register_native_closure` inside `drive_to_completion`. Both natives capture the same `Arc<AtomicU64>` as the CLI flag. The setter routes through the same duration parser; a parse error surfaces as `VmError::NativeError` so a daemon that calls the setter at the top of the loop body fails fast.

3. **Drift compensation**. `drive_loop_main` now measures iteration time with `Instant::now()` and sleeps for `max(0, interval - elapsed)` after each yield or reset transition. When elapsed exceeds the interval, the runner emits a stderr warning naming both values and resumes immediately. The `--quiet` flag suppresses the warning. The zero-interval default preserves the prior spin-as-fast-as-possible behaviour.

### REPL fix

`execute_source_repl` was promoted to a dedicated path that uses `DEFAULT_ARENA_CAPACITY` directly. The prior call site routed through `execute_source`, which auto-sized the arena per expression. Auto-sizing was the wrong behaviour for the REPL because ad-hoc expressions have no meaningful WCMU bound.

### Verification

Three smoke tests exercised the feature end to end.

1. Script-side `shell::set_tick_interval("50ms")` over three iterations with `shell::tick_interval()` printing the value: ran in 224ms wall clock (150ms of sleep plus startup).
2. CLI flag `--tick-interval 100ms` over three iterations: ran in 420ms wall clock (300ms of sleep plus startup).
3. Intentional overrun via `shell::run_checked("sleep 0.1")` at `--tick-interval 10ms`: stderr warning fired naming `108ms` and `10ms`; `--quiet` suppressed the warning.

Composite duration rejected with a clear diagnostic. Over-limit interval `5w` rejected with a clear diagnostic pointing at cron or noop yield cycles for longer cadences.

### Documentation

- `keleusma-cli/README.md` gains a "Productive-divergent loop runner" section with the unit table, the `--tick-interval` and `--quiet` flag descriptions, and a worked script example.
- `docs/guide/SECURITY_POLICY.md` gains a "Daemon deployments and tick-interval cadences" section covering fail-fast setter placement, memory residency as a feature, and the cron-or-noop-cycles pattern for cadences longer than four weeks.
- `docs/guide/METRICS.md` gains a "Steady-state at sleep cadence" subsection in the Loop daemon workload section.
- `print_help` output in `keleusma-cli/src/main.rs` lists the new flags.

## Verification

- `cargo test --workspace`: 843 + 2 + 17 + 17 + 17 + 3 + 53 + 37 + 6 + 20 + 7 = 1022 tests across the workspace passing. The 11 new duration parser tests are part of the 20-test keleusma-cli suite.
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.

## Outstanding concerns

None blocking. Two observations for future operator attention.

1. The duration parser supports only single-unit forms. Operators who want composite forms must rewrite (`1h30m` becomes `90m`). The diagnostic is clear and points at the rewrite, but operators new to the CLI may take a moment to recognise the constraint.

2. The CLI-side natives bypass the type checker's static signature validation because they are not declared with a parenthesised signature in the `use` statement. The script writer must invoke them through the qualified path (`shell::set_tick_interval(...)`) rather than the unqualified name because the typechecker's lookup is exact. This is consistent with how `shell::exit` works today and is documented inline in the worked example.

## Recommended next step

The session's code and documentation are ready for commit. Commit alongside the prior V0.2.1 signing-and-encryption layers under a single feature commit, then push.

If the operator wants a longer-running session, the next adjacent piece is the SHELL_AUDIT.md recommendations: `shell::sleep_ms`, `shell::now_unix_ms`, `shell::read_file` / `shell::write_file` / `shell::append_file`. Sleep and time are the highest-value additions because they reduce the operational overhead of the loop daemon pattern.
