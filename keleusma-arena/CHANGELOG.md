# Changelog

All notable changes to `keleusma-arena` will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
