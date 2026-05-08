# keleusma-arena

> Simple and boring memory allocator for exciting applications.

A dual-end bump-allocated arena for embedded Rust. A single contiguous buffer holds two bump pointers that grow toward each other from opposite ends. Allocation is constant-time. Allocation fails when the two pointers would meet. The arena's discipline supports both ad-hoc embedded use and static analysis of memory bounds.

## Why

`keleusma-arena` is the memory substrate of the Keleusma scripting language, extracted as a standalone crate for general-purpose embedded use. It is designed for environments where memory is fixed, predictability matters, and the budget is part of the contract.

| Property | This crate |
|---|---|
| Storage shape | Single contiguous buffer. Fixed at construction. |
| Allocation timing | Constant-time always. No path that allocates from the global allocator at use-time. |
| Failure mode | Returns `AllocError` on overflow. The host explicitly handles it. |
| Region structure | Two bump pointers growing from opposite ends, sharing a budget. |
| Static bound | Generic `Budget` type. Producers compute a budget from any analysis. The arena verifies admissibility. |
| Targeting | `core` only without `alloc`. The static-buffer constructor admits `&'static mut [u8]` for link-time-allocated buffers. |
| Code footprint | Small. Single allocation strategy, audit-friendly. |

## Quick Start

```rust
use keleusma_arena::Arena;
use core::alloc::Layout;

// Heap-backed (alloc feature on by default).
let arena = Arena::with_capacity(4096);

// Bottom end allocation.
let layout = Layout::new::<u64>();
let p = allocator_api2::alloc::Allocator::allocate(&arena.bottom_handle(), layout)
    .unwrap();

// Top end allocation.
let q = allocator_api2::alloc::Allocator::allocate(&arena.top_handle(), layout)
    .unwrap();

assert_eq!(arena.bottom_used(), 8);
assert_eq!(arena.top_used(), 8);
```

## Static-Buffer Use

For embedded targets without a global allocator, use a statically allocated buffer.

```rust
use keleusma_arena::Arena;

static mut BUFFER: [u8; 4096] = [0u8; 4096];

fn make_arena() -> Arena {
    // SAFETY: BUFFER is accessed exclusively through the arena.
    let buffer: &'static mut [u8] = unsafe {
        core::slice::from_raw_parts_mut(BUFFER.as_mut_ptr(), BUFFER.len())
    };
    Arena::from_static_buffer(buffer)
}
```

For targets with multiple memory regions, such as the Game Boy Advance with IWRAM, EWRAM, and VRAM, construct one arena per region.

## Collection Integration

The handles implement `allocator_api2::alloc::Allocator`, so standard allocator-aware collections work directly.

```rust
use keleusma_arena::Arena;
use allocator_api2::vec::Vec as ArenaVec;

let arena = Arena::with_capacity(4096);
let mut v: ArenaVec<i32, _> = ArenaVec::new_in(arena.bottom_handle());
v.push(1);
v.push(2);
v.push(3);
```

## Budget Contract

The arena exposes a generic `Budget` type and a `fits_budget` method for compile-time bounds analysis.

```rust
use keleusma_arena::{Arena, Budget};

let arena = Arena::with_capacity(4096);
let budget = Budget::new(2048, 1024);  // bottom_bytes, top_bytes
assert!(arena.fits_budget(&budget));
```

The Keleusma runtime computes its budget through static analysis of bytecode and uses `fits_budget` to enforce its bounded-memory guarantee. Independent users may compute budgets through profiling, manual analysis, or any other mechanism.

## Mark and Rewind

Each end supports a LIFO mark and rewind discipline.

```rust
use keleusma_arena::Arena;
use core::alloc::Layout;

let arena = Arena::with_capacity(1024);
let mark = arena.bottom_mark();
let _scratch = allocator_api2::alloc::Allocator::allocate(
    &arena.bottom_handle(),
    Layout::new::<[u64; 16]>(),
).unwrap();
// SAFETY: No live references to the scratch allocation remain.
unsafe { arena.rewind_bottom(mark); }
assert_eq!(arena.bottom_used(), 0);
```

Rewind and per-end reset are unsafe because they invalidate the contents of the rewound region. The caller is responsible for ensuring no live references remain.

## Observability

```rust
use keleusma_arena::Arena;

let arena = Arena::with_capacity(4096);
// ... allocate ...
println!("bottom used: {} bytes", arena.bottom_used());
println!("top used: {} bytes", arena.top_used());
println!("free: {} bytes", arena.free());
println!("bottom peak: {} bytes", arena.bottom_peak());
println!("top peak: {} bytes", arena.top_peak());
```

Peak watermarks are tracked since arena creation or the last `clear_peaks` call. Useful for sizing analysis and post-mortem inspection.

## Comparison with bumpalo

| Property | bumpalo | keleusma-arena |
|---|---|---|
| Storage | Linked list of chunks. Auto-grows. | Single contiguous buffer. Fixed at construction. |
| Allocation timing | Constant-time fast path. Chunk allocation occasional. | Constant-time always. |
| Failure mode | Effectively only fails on global allocator exhaustion. | Returns `AllocError` on overflow. Fail-fast. |
| Region structure | Single bump pointer. | Dual-end. Two pointers with a shared budget. |
| Static bound | None. | `Budget` contract. |
| `core`-only | No. Requires `alloc`. | Yes. `alloc` is opt-in. |
| Target audience | General-purpose Rust. | Embedded, safety-critical, real-time, certifiable. |

The two crates serve different niches. `bumpalo` is the right choice when memory is plentiful and growth is acceptable. `keleusma-arena` is the right choice when memory is fixed, predictability is essential, and the budget is part of the contract.

## Features

- `alloc` (default). Enables `Arena::with_capacity` and `allocator-api2`'s `vec` and other alloc-dependent modules. Disable for `core`-only targets.

## License

MIT.

## Crate Family

- [`keleusma`](https://crates.io/crates/keleusma). The Keleusma scripting language and runtime.
- [`keleusma-macros`](https://crates.io/crates/keleusma-macros). Procedural macros used by the runtime.
- `keleusma-arena`. This crate.

The arena is the original substrate that the runtime is built on. It is published as a standalone crate so that embedded users can adopt the discipline without taking on the full language runtime.
