# keleusma-arena

> Simple and boring memory allocator for exciting applications.

A dual-end bump-allocated arena for embedded Rust. Single contiguous buffer. Two pointers growing toward each other from opposite ends. Constant-time allocation. Fail-fast on exhaustion. `core`-only when the `alloc` feature is off.

## History

Born as the memory substrate of the Keleusma scripting language and extracted as a standalone crate so embedded users can adopt the discipline without the language runtime.

## Philosophy

Boring code that does exciting things. The arena's storage shape, allocation strategy, and failure mode are the simplest possible. The discipline that emerges supports real-time predictability, certifiable memory bounds, and zero-allocation hot paths on platforms with fixed memory.

- Single allocation strategy. No chunk lists, no fallback paths.
- Fixed at construction. No surprise growth at use-time.
- Fail-fast. Returns `AllocError` on overflow.
- `core`-only without `alloc`.
- Two ends with one budget.

## Niche

- Embedded systems with link-time-allocated buffers.
- Targets with multiple distinct memory regions. Construct one arena per region.
- Real-time and safety-critical workloads where fixed bounds and constant-time allocation are required.
- Game engines and simulation loops that reset the arena per frame.
- Programs that want a compile-time memory budget contract.

## Quick Start

```rust
use keleusma_arena::Arena;
use core::alloc::Layout;
use allocator_api2::alloc::Allocator;

let arena = Arena::with_capacity(4096);
let layout = Layout::new::<u64>();

// Stack-end and heap-end allocation. The arena imposes no semantic
// distinction; these are conventional aliases for the two ends.
let _stack_alloc = arena.stack_handle().allocate(layout).unwrap();
let _heap_alloc = arena.heap_handle().allocate(layout).unwrap();
```

## Static-Buffer Use

For embedded targets without a global allocator, hand the arena a statically allocated buffer.

```rust
use keleusma_arena::Arena;

static mut BUFFER: [u8; 4096] = [0u8; 4096];

// `addr_of_mut!` obtains a raw pointer without creating a reference
// to the static, which is required under edition 2024.
let buffer: &'static mut [u8] = unsafe {
    core::slice::from_raw_parts_mut(
        core::ptr::addr_of_mut!(BUFFER) as *mut u8,
        4096,
    )
};
let arena = Arena::from_static_buffer(buffer);
```

## Aligned and Unaligned Allocation

Aligned allocations go through the `Allocator` trait with a `Layout` that carries the desired alignment. Unaligned byte allocations have direct convenience methods.

```rust
use keleusma_arena::Arena;
use core::alloc::Layout;
use allocator_api2::alloc::Allocator;

let arena = Arena::with_capacity(4096);

// Three packed bytes. No padding for alignment.
let _a = arena.alloc_bottom_bytes(3).unwrap();

// A pointer-aligned allocation. The arena pads as needed.
let _p = arena.stack_handle().allocate(Layout::new::<*const u8>()).unwrap();
```

## Collections, Marks, Stats

```rust
use keleusma_arena::Arena;
use allocator_api2::vec::Vec as ArenaVec;

let arena = Arena::with_capacity(4096);

// Arena-backed collection.
let mut stack: ArenaVec<i32, _> = ArenaVec::new_in(arena.stack_handle());
stack.push(1);

// LIFO discipline through marks.
let mark = arena.bottom_mark();
unsafe { arena.rewind_bottom(mark); }

// Observability.
let _peak = arena.bottom_peak();
```

## Budget Contract

The arena exposes a generic `Budget` type and a `fits_budget` method for compile-time bounds analysis.

```rust
use keleusma_arena::{Arena, Budget};

let arena = Arena::with_capacity(4096);
let budget = Budget::new(2048, 1024);
assert!(arena.fits_budget(&budget));
```

For a concrete example of computing a budget from a static analysis and using it to verify admissibility, see the Keleusma scripting runtime, which computes a `Budget` from bytecode worst-case memory usage analysis and uses `fits_budget` to enforce the bounded-memory guarantee at module load time. The Keleusma project is the original consumer of this crate and demonstrates the discipline end-to-end.

## Epoch and Stale-Pointer Detection

`Arena::reset` advances an internal `epoch` counter and clears both regions in one operation. The safe wrapper `ArenaHandle<T>` captures the epoch at construction and validates it on access through `handle.get(&arena)`, which returns `Result<&T, Stale>`. A handle from a prior epoch is detected at the access site and produces a typed `Stale` error rather than returning a dangling reference.

The crate exposes `ArenaHandle<T>` as the generic mechanism. Higher-level helpers — for example a string handle in a downstream crate — allocate typed storage in the arena and wrap the resulting pointer through the unsafe `ArenaHandle::from_raw_parts(ptr, epoch)` constructor. The example below allocates a single `u64` and demonstrates the lifecycle.

```rust
use core::alloc::Layout;
use core::ptr::NonNull;
use allocator_api2::alloc::Allocator;
use keleusma_arena::{Arena, ArenaHandle};

let mut arena = Arena::with_capacity(4096);

// Allocate a u64 from the arena's top region and wrap in a handle.
let layout = Layout::new::<u64>();
let raw = arena.top_handle().allocate(layout).unwrap();
let typed: NonNull<u64> = raw.cast();
unsafe { typed.as_ptr().write(42) };
let handle: ArenaHandle<u64> = unsafe {
    ArenaHandle::from_raw_parts(typed, arena.epoch())
};
assert_eq!(*handle.get(&arena).unwrap(), 42);

// Reset advances the epoch and invalidates the prior handle.
arena.reset().unwrap();
assert!(handle.get(&arena).is_err());
```

See `examples/epoch_handle.rs` for the runnable variant. For an arena-backed `&str` handle that encapsulates the byte copy and `*mut str` construction, see the `KString` newtype in the [`keleusma`](https://crates.io/crates/keleusma) crate; that name is intentionally kept in the parent crate because it carries Keleusma-specific semantics around dynamic-string flow.

The epoch counter is `u64` and saturates at `u64::MAX`. The safe `Arena::reset` returns `EpochSaturated` once the counter cannot advance further; recovery is through `Arena::force_reset_epoch`, which is unsafe because the caller must certify that no `ArenaHandle` from any prior epoch is still in use. The unsafe variants `Arena::reset_unchecked` and `Arena::reset_top_unchecked` are available for callers who hold an active borrow into the arena and have certified the same condition for that borrow.

The epoch model is opt-in. Callers who prefer the 0.1.0-style mark-and-rewind discipline can continue to use `bottom_mark`, `top_mark`, `rewind_bottom`, `rewind_top`, `reset_bottom`, and `reset_top` without ever constructing an `ArenaHandle`; those operations remain available with their original semantics.

## Naming

The canonical handle types are `BottomHandle` and `TopHandle`, matching a vertical-buffer model where the bottom end starts at low addresses and grows up while the top end starts at high addresses and grows down. Code that prefers a CPU-memory mental model may use the `stack_handle()` and `heap_handle()` method aliases. The arena imposes no semantic distinction between the two ends.

## Comparison with bumpalo

| | bumpalo | keleusma-arena |
|---|---|---|
| Storage | Linked chunks. Auto-grows. | Single contiguous buffer. Fixed. |
| Failure | Effectively only on global allocator exhaustion. | `AllocError` on overflow. Fail-fast. |
| Region structure | One bump pointer. | Two, sharing a budget. |
| `core`-only | No. | Yes, when `alloc` feature is off. |
| Static-buffer constructor | No. | Yes. |
| Budget contract | No. | Generic `Budget` type. |

`bumpalo` is the right choice when memory is plentiful and growth is acceptable. `keleusma-arena` is the right choice when memory is fixed and predictability is the contract.

## Features

- `alloc` (default). Enables `Arena::with_capacity` and the `allocator-api2` collection types. Disable for `core`-only targets.

## License

BSD Zero Clause License (0BSD). See LICENSE.
