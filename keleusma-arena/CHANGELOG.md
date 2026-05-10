# Changelog

All notable changes to `keleusma-arena` will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-05-10

Adds the epoch system for stale-pointer detection on safe arena handles. The 0.1.0 surface is preserved unchanged; this release is purely additive at the API level.

### Added

- `Arena::reset` returning `Result<(), EpochSaturated>`. Advances the epoch counter, clears both the bottom and top regions, and refuses if the counter has saturated. This is the safe full-reset operation; the older `reset_bottom` and `reset_top` remain available as unsafe per-end resets.
- `Arena::reset_unchecked` and `Arena::reset_top_unchecked`, the unsafe variants suitable for callers who hold an active borrow into the arena and have certified that no allocator-bound collection retains storage at the moment of reset. Both advance the epoch.
- `Arena::force_reset_epoch`, an unsafe recovery path for the saturated case. The caller certifies that no `ArenaHandle` from any prior epoch is still in use.
- `Arena::epoch` and `Arena::epoch_remaining` for observability.
- `EpochSaturated` error type returned by the safe reset path when the epoch counter cannot advance further. `u64::MAX` epochs are sufficient for almost all deployments; the type exists to make the saturating refusal explicit.
- `ArenaHandle<T: ?Sized>`, a generic safe wrapper around an arena pointer that captures the epoch at construction. The `get(&arena)` accessor returns `Result<&T, Stale>`, so a handle from a prior epoch is detected at access rather than producing undefined behavior.
- `ArenaHandle::from_raw_parts(ptr: NonNull<T>, epoch: u64) -> Self`, an unsafe constructor for downstream crates that allocate typed storage in the arena and want to wrap the resulting pointer in a stale-detecting handle.
- `Stale` error type returned by `ArenaHandle::get` when the captured epoch no longer matches the arena's current epoch.

### Tested

- Six new tests covering the epoch surface: handle round-trip via `ArenaHandle::from_raw_parts`, handle access after reset (`Stale`), `Copy` semantics on the handle, epoch saturation refusal, `force_reset_epoch` recovery, and `reset_top_unchecked` preserving bottom-region collections.
- The existing 0.1.0 tests continue to pass unchanged.

### Notes

- This is a feature release. SemVer-correct under 0.x because the addition is large enough that downstream consumers should opt in explicitly through their version requirement. Existing users on `keleusma-arena = "0.1"` are not auto-upgraded; bump the requirement to `"0.2"` to consume the new surface.
- The 0.1.0 public surface is preserved unchanged. Existing call sites compile against 0.2.0 without modification.

## [0.1.0] - 2026-05-08

Initial release.

### Added

- `Arena` type with three constructors. `with_capacity` allocates a 16-byte-aligned heap buffer when the `alloc` feature is enabled. `from_static_buffer` borrows a `&'static mut [u8]`. `from_buffer_unchecked` accepts a raw pointer and length under the caller's lifetime guarantee.
- `BottomHandle` and `TopHandle` allocation handles implementing `allocator_api2::alloc::Allocator`. Method aliases `stack_handle` and `heap_handle` exist for ergonomics.
- `Budget` struct and `Arena::fits_budget` for a generic memory budget contract independent of any specific producer.
- `BottomMark` and `TopMark` snapshot types for LIFO discipline. Safe `bottom_mark` and `top_mark` accessors. Unsafe `rewind_bottom`, `rewind_top`, `reset_bottom`, `reset_top` operations.
- Peak watermark tracking with `bottom_peak`, `top_peak`, and `clear_peaks`.
- `alloc_bottom_bytes` and `alloc_top_bytes` for unaligned byte allocations.
- Address-aware alignment computation that handles arbitrary buffer base alignment.
- Examples covering basic dual-end usage, frame-loop reset pattern, budget-contract verification, and use alongside the global allocator.
- README with ecosystem positioning, philosophy, niche, and comparison with `bumpalo`.

### Tested

- 22 unit tests covering construction, allocation, alignment, exhaustion, reset, peak tracking, mark and rewind, budget admissibility, integration with `allocator_api2::vec::Vec`, and edge cases.
- Tests pass under stable Rust 1.85 (edition 2024 minimum) and under miri's stacked-borrows model with one test (`arena_from_static_buffer`) ignored due to deliberate Vec leak that does not represent real-world `'static` buffer use.

### Licensed

- BSD Zero Clause License (`0BSD`).
