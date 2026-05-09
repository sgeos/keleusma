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
let _bottom = arena.bottom_handle().allocate(layout).unwrap();
let _top = arena.top_handle().allocate(layout).unwrap();
```

## Static-Buffer Use

For embedded targets without a global allocator, hand the arena a statically allocated buffer.

```rust
use keleusma_arena::Arena;

static mut BUFFER: [u8; 4096] = [0u8; 4096];

fn make_arena() -> Arena {
    let buffer: &'static mut [u8] = unsafe {
        core::slice::from_raw_parts_mut(BUFFER.as_mut_ptr(), BUFFER.len())
    };
    Arena::from_static_buffer(buffer)
}
```

## Collections, Marks, Budgets, Stats

```rust
use keleusma_arena::{Arena, Budget};
use allocator_api2::vec::Vec as ArenaVec;

let arena = Arena::with_capacity(4096);

// Arena-backed collection.
let mut v: ArenaVec<i32, _> = ArenaVec::new_in(arena.bottom_handle());
v.push(1);

// LIFO discipline through marks.
let mark = arena.bottom_mark();
unsafe { arena.rewind_bottom(mark); }

// Budget contract.
let budget = Budget::new(2048, 1024);
assert!(arena.fits_budget(&budget));

// Observability.
let _peak = arena.bottom_peak();
```

## Naming

The canonical handle types are `BottomHandle` and `TopHandle`, matching a vertical-buffer model where the bottom end starts at low addresses and grows up while the top end starts at high addresses and grows down. Code that prefers a CPU-memory mental model may treat the bottom end as a stack-like region and the top end as a heap-like region, with the corresponding mental aliases `StackHandle` and `HeapHandle`. The arena imposes no semantic distinction between the two ends.

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
