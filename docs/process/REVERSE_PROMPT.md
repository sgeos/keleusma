# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-10
**Status**: Two of three crates published to crates.io. `keleusma 0.1.0` is the remaining publication step.

## Published

- `keleusma-arena 0.2.0` is live at https://crates.io/crates/keleusma-arena and https://docs.rs/keleusma-arena/0.2.0/. The 0.2.0 release adds the epoch-tagged stale-pointer detection surface (`ArenaHandle<T>`, `Arena::reset` returning `Result<(), EpochSaturated>`, `from_raw_parts`, `Stale`) on top of the preserved 0.1.0 mark/rewind/peak surface.
- `keleusma-macros 0.1.0` is live at https://crates.io/crates/keleusma-macros and https://docs.rs/keleusma-macros/0.1.0/. Implementation-detail crate; users depend on `keleusma` and consume the derive through `keleusma::KeleusmaType`.

## Outstanding TODO

**Publish `keleusma 0.1.0` to crates.io.**

The publication-readiness state is the following.

- `cargo publish -p keleusma --dry-run` succeeds against the registry-resolved dependencies. Package is 91 files, 1.3 MiB unpacked, 330.5 KiB compressed. Verification compiles cleanly after Cargo downloads `keleusma-arena 0.2.0` and `keleusma-macros 0.1.0` from crates.io.
- All `.kel` files referenced through `include_str!` are confirmed in the package: `examples/piano_roll.kel`, `examples/piano_roll_2.kel`, and the eight `examples/scripts/*.kel`.
- Cargo.toml metadata is complete: description, keywords (5), categories (3), license (`0BSD`), homepage, repository, documentation, readme, rust-version 1.87.
- `LICENSE` and `CHANGELOG.md` are present at the repository root and are included in the package.
- Feature-gated `sdl3` dep is correctly marked optional; `[[example]] piano_roll` is gated by `required-features = ["sdl3-example"]`. Workspace builds and tests are SDL3-free.
- 520 workspace tests pass; clippy, format, rustdoc clean.

The agent does not perform `cargo publish`. The operator runs `cargo publish -p keleusma`. After the index propagates, the V0.1.0 release is complete.

One informational caveat: `cargo publish` for 0.1.0 is permanent. The version can be yanked but not unpublished, and a yanked version cannot be republished with the same number. Any subsequent fix requires a 0.1.1 release. The state of the repository at the most recent commit on `main` is what ships as `keleusma 0.1.0`.

## Verification

The most recent verification matrix (run before this compaction):

```bash
cargo test --workspace                                   # 520 pass
cargo clippy --workspace --all-targets -- -D warnings    # clean
cargo fmt --check                                        # clean
cargo doc --no-deps -p keleusma                          # clean
cargo doc --no-deps -p keleusma-arena                    # clean
cargo doc --no-deps -p keleusma-macros                   # clean
cargo build -p keleusma --target thumbv7em-none-eabihf   # clean (no_std + alloc)
rustup run 1.85 cargo check -p keleusma-arena            # MSRV clean
cargo publish -p keleusma --dry-run                      # clean
```

## Intended Next Step

Operator runs `cargo publish -p keleusma`. Agent awaits prompt before proceeding to any post-publication work.

## Recent Session Context

This sprint closed the V0.1.0 publication path. Five tasks (V0.1-M3-T48 through T52) addressed the publication-readiness gaps surfaced by `cargo publish --dry-run` and by review of crate-level metadata. The notable architectural decision was moving `KString` out of `keleusma-arena` into the `keleusma` main crate so the allocator owns the generic mechanism (`ArenaHandle<T>`) and the runtime owns the `&str`-specific policy. Detailed historical entries live in [TASKLOG.md](./TASKLOG.md) and [the CHANGELOG](../../CHANGELOG.md); this document records only the current handoff state.
