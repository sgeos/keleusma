# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-19
**Status**: Two new cargo features land on the parent keleusma crate, both default on. The `compile` feature gates the source-to-bytecode pipeline. The `verify` feature gates the load-time verifier. The RTOS microkernel example gains matching `keleusma-compile` and `keleusma-verify` pass-through features plus build.rs precompilation when source compilation is excluded from the runtime image. Verified on hardware in three feature combinations.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Add a feature gate on keleusma for source-to-bytecode conversion so the lexer, parser, type checker, monomorphizer, and compiler can be stripped from the runtime image. | New `compile` cargo feature in the parent crate, default on. The eight compile-pipeline modules (`ast`, `compiler`, `lexer`, `monomorphize`, `parser`, `target`, `token`, `typecheck`, `visitor`) are gated behind it. With the feature off, the runtime accepts only precompiled bytecode through `Module::from_bytes` and `Vm::view_bytes_zero_copy`. |
| Add a feature gate on keleusma for load-time verification so the structural verifier and the WCET and WCMU analyses can be stripped. | New `verify` cargo feature in the parent crate, default on. The `verify` and `text_size` modules and the verifier calls inside `Vm::new`, `Vm::new_with_options`, `Vm::new_unchecked`, and `Vm::replace_module` are gated behind it. `Vm::verify_resources`, `Vm::auto_arena_capacity`, and `auto_arena_capacity_for` are gated entirely. When the feature is off, `Vm::new` skips verification and behaves equivalently to `Vm::new_unchecked` from the caller's perspective. The compiler still invokes the verifier at end of `compile_with_target` when both features are on and populates the WCET and WCMU header fields exactly as before; with `verify` off the compiler leaves those fields at 0 (auto). |
| Match the parent's feature gates on the microkernel and produce a smaller binary when source compilation is disabled. | The microkernel grew `keleusma-compile` and `keleusma-verify` pass-through features (both default on, both forwarding to the parent's namesake features). The microkernel's `build.rs` invokes the parent's compile pipeline at host build time when `keleusma-compile` is off, emitting one `OUT_DIR/<name>.kel.bin` per task script through a `[build-dependencies]` entry. `setup.rs` routes between `include_str!` plus compile-at-boot and `include_bytes!` plus `Module::from_bytes`. The bare-metal binary's `.text` size drops from 614 KB (full pipeline) to 192 KB (precompile under trust), a 69% reduction. |
| Provide reasonable defaults. | Both features are in default for backward compatibility. Existing consumers see no change. |

## Verification matrix

```bash
cargo build --release                                                           # parent, default features
cargo build --release --no-default-features                                     # parent, no features
cargo build --release --no-default-features --features compile                  # parent, compile only
cargo build --release --no-default-features --features verify                   # parent, verify only
cargo test --release --features text                                            # 575 lib tests + 17+17+3+53 integration; all pass
cargo test --release --no-default-features                                      # 0 tests, no failures
cargo test --release --no-default-features --features compile                   # 266 lib tests pass
cargo test --release --no-default-features --features verify                    # 45 lib tests pass
cargo clippy --workspace --tests --features text -- -D warnings                 # clean

(cd examples/rtos && cargo build --release --bin three-task-std)                # std default
(cd examples/rtos && cargo build --release --bin three-task-std \
    --no-default-features --features std-platform)                              # std smallest
(cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf \
    --release --bin three-task-n6 \
    --no-default-features --features stm32n6570dk-platform)                     # n6 smallest
(cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf \
    --release --bin three-task-n6 \
    --no-default-features --features stm32n6570dk-platform,keleusma-verify)     # n6 precompile + verify
(cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf \
    --release --bin three-task-n6 \
    --no-default-features --features stm32n6570dk-platform,keleusma-compile,keleusma-verify)  # n6 full
```

Bare-metal binary text sizes:

| Feature combination | `.text` |
|---------------------|--------:|
| Full pipeline (default minus `std-platform`) | 614 KB |
| Precompile and verify (`keleusma-verify`) | 211 KB |
| Precompile under trust (no `keleusma-*` features) | 192 KB |

Hardware verification on the STM32N6570-DK 2026-05-19. All three modes flashed, scheduler entered, four heartbeat ticks captured.

```
mode=trust            scheduler entry t=39 ms     four heartbeats at 41/5042/10043/15043 ms
mode=precompile+verify scheduler entry t=43 ms     four heartbeats at 46/5047/10048/15048 ms
mode=full              scheduler entry t=216 ms    four heartbeats at 218/5219/10220/15221 ms
```

## Notes

- The parent's example list in `Cargo.toml` gained explicit `required-features = ["compile", "verify"]` entries on every example that drives the compile pipeline. The lone exception is `zero_copy_include_bytes`, which loads precompiled bytecode and now builds under any feature combination.
- The parent's three integration tests `marshall`, `opaque`, and `rogue_scripts` are gated through `#![cfg(all(feature = "compile", feature = "verify"))]` at the file head. Test modules inside always-on modules (`vm`, `utility_natives`, `audio_natives`, `stddsl`) gained matching cfg gates so a no-default-features build does not try to compile them.
- The microkernel's `[workspace] resolver = "2"` declaration is now load-bearing. Without it, build-dependency features would unify with runtime-dependency features and pull the compile pipeline into the runtime image even when `keleusma-compile` is off.
- The microkernel's `[build-dependencies] keleusma` entry moved to the end of the dependency block in `Cargo.toml`. Cargo treats every key after a `[build-dependencies]` header as part of that section until a new `[xxx]` header appears, so misplacing the block in the middle of the runtime dependencies silently reassigned the optional embassy and cortex-m deps to the build graph.

## Intended Next Step

Awaiting operator prompt. The two feature gates open up substantial follow-on work, none blocking.

1. **Decide on a default for the microkernel.** Defaults currently mirror the parent (compile and verify both on). For a production-shipped image the better default is `keleusma-verify` only with precompiled bytecode. Switching the default is a one-line Cargo.toml change.
2. **Document the feature matrix in the parent's top-level `README.md`.** The CHANGELOG entry covers it; the README still mentions only `text`, `shell`, and `sdl3-example`.
3. **Move other examples to the precompiled-bytecode pattern** where they would benefit (binary-size or boot-time gains).
4. **Slim further by trimming embassy features** to the minimum surface the kernel touches. Independent of the work above.
5. **WCET banner at boot.** With `verify` on, the runtime can read `Module::wcet_cycles` and report it as certification evidence. Was previously deferred; the new feature gates make the banner straightforward to wire only when verify is enabled.
