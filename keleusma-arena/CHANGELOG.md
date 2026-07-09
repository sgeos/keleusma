# Changelog

All notable changes to `keleusma-arena` will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1] - 2026-07-08

Purely additive over 0.3.0. Seven new public methods, no removals or changes, so
existing 0.3.0 callers compile unchanged. This release is consumed by the parent
`keleusma` runtime, which uses the ephemeral-address predicate and the fallible
constructor; it was published because `keleusma` 0.2.1 depends on this surface.

### Added

- `Arena::resize_persistent_capacity(new_size) -> Result<(), ResizeError>`, a preserving in-place resize of the persistent region. Unlike `resize_persistent`, which fully resets the dual-headed region, this preserves the persistent prefix's contents and relocates the dual-headed (bottom) region by the size delta. It errors without mutation on a dual-headed overlap or an oversize request, and advances the epoch so any handle into the old dual-headed region fails closed. It supports growing or shrinking a live persistent region, for example a REPL restoring a saved session image.
- `Arena::try_with_capacity(capacity) -> Result<Self, AllocError>`, the fallible counterpart of `with_capacity`. Returns the allocation error rather than panicking when the backing buffer cannot be allocated, for hosts that must handle allocation failure without unwinding. Requires the `alloc` feature, like `with_capacity`.
- `Arena::zero_persistent_range(start, len) -> Result<(), ResizeError>`, zeroes the sub-range `[start, start + len)` of the persistent region and errors without mutation on an out-of-range request. A narrower form of `zero_persistent`, which zeroes the whole region; it does not touch the dual-headed region, the bump pointers, or the epoch.
- `Arena::addr_is_ephemeral(addr) -> bool`, reports whether an address falls in the ephemeral dual-headed region rather than the persistent prefix. Lets a caller decide whether a stored raw address is cleared at the next reset.
- `ArenaHandle::as_non_null() -> NonNull<T>`, returns the raw non-null pointer backing the handle, for callers that need pointer access without going through `get`.
- `ArenaHandle::len() -> usize` and `ArenaHandle::is_empty() -> bool`, length accessors for a handle to a slice or string, reading the fat-pointer length without dereferencing.

### Tested

- The existing 0.3.0 tests continue to pass unchanged; the additions are covered by the parent runtime's suites and the arena unit tests.

## [0.3.0] - 2026-05-19

Adds a persistent (`.data`) region inside the arena that is preserved across every form of reset. The dual-headed (bottom plus top) layout is unchanged; the persistent region occupies a configurable prefix of the buffer. The 0.2.0 surface is preserved unchanged when `persistent_capacity == 0`; this release is purely additive at the API level for arenas that opt into the new region.

### Added

- `Arena::persistent_capacity` getter. Returns the size in bytes of the persistent region. Default is zero.
- `Arena::dual_headed_capacity` getter. Returns `capacity - persistent_capacity`.
- `Arena::resize_persistent(new_size) -> Result<(), ResizeError>`. Assigns the persistent region size, fully resets the dual-headed region, and advances the epoch. The shape of the API supports the pooling use case where one oversized arena is reassigned to a different script before each use.
- `Arena::persistent_ptr() -> NonNull<u8>`. Returns a stable non-null pointer to the start of the persistent region. The caller manages access discipline; the arena type is not `Sync`.
- `Arena::zero_persistent`. Overwrites the persistent region with zeros. Does not touch the dual-headed region, the bump pointers, or the epoch.
- `Arena::zero_dual_headed -> Result<(), EpochSaturated>`. Overwrites the dual-headed region with zeros and fully resets it. Advances the epoch.
- `Arena::zero_all -> Result<(), EpochSaturated>`. Overwrites the entire backing buffer with zeros, resets the bump pointers, and advances the epoch. Leaves the persistent capacity unchanged.
- `ResizeError` enum with `ExceedsCapacity` and `EpochSaturated` variants, returned by `resize_persistent`.

### Changed

- The bottom region now starts at offset `persistent_capacity` rather than offset zero. With `persistent_capacity == 0`, the layout is identical to 0.2.0 and existing callers see no behavioural change.
- `Arena::reset`, `Arena::reset_unchecked`, `Arena::reset_bottom`, and `Arena::force_reset_epoch` now rewind the bottom pointer to `persistent_capacity` rather than to zero. The persistent region is preserved across all of them. With `persistent_capacity == 0` the behaviour is identical to 0.2.0.
- `Arena::bottom_used` and `Arena::bottom_peak` now report values relative to the start of the bottom region (offset `persistent_capacity`) rather than absolute offsets from the buffer base. With `persistent_capacity == 0` the values are identical to 0.2.0.

### Tested

- Nine new tests covering the persistent surface: default capacity, resize-and-shift, oversize rejection, reset preserves contents, `zero_persistent` scope, `zero_dual_headed` scope, `zero_all` scope, epoch advance on resize, and the pooling pattern.
- The existing 0.2.0 tests continue to pass unchanged.

### Notes

- This release is consumed by the parent `keleusma` crate as part of the `compile` and `verify` feature-gating work and the new `private` data declaration surface. Downstream applications that depend on `keleusma-arena` directly and never set a non-zero persistent capacity see no behavioural change.

## [0.2.0] - 2026-05-10

Adds the epoch system for stale-pointer detection on safe arena handles. The 0.1.0 surface is preserved unchanged; this release is purely additive at the API level.

### Added

- `Arena::reset` returning `Result<(), EpochSaturated>`. Advances the epoch counter, clears both the bottom and top regions, and refuses if the counter has saturated. This is the safe full-reset operation; the older `reset_bottom` and `reset_top` remain available as unsafe per-end resets.
- `Arena::reset_unchecked` and `Arena::reset_top_unchecked`, the unsafe variants suitable for callers who hold an active borrow into the arena and have ensured that no allocator-bound collection retains storage at the moment of reset. Both advance the epoch.
- `Arena::force_reset_epoch`, an unsafe recovery path for the saturated case. The caller ensures that no `ArenaHandle` from any prior epoch is still in use.
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
