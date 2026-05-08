# keleusma-arena

> Simple and boring memory allocator for exciting applications.

A dual-end bump-allocated arena for embedded Rust. Single contiguous buffer. Two pointers growing toward each other from opposite ends. Constant-time allocation. Fail-fast on exhaustion.

## History

Born as the memory substrate of the Keleusma scripting language, where it pairs with a static analysis pass to enforce a bounded-memory guarantee at script load time. Extracted as a standalone crate so embedded users can adopt the discipline without taking on the language runtime.

## Philosophy

Boring code that does exciting things. The arena's storage shape, allocation strategy, and failure mode are all the simplest possible. The discipline that emerges is what enables the exciting use cases, namely real-time predictability, certifiable memory bounds, and zero-allocation hot paths on platforms with fixed memory.

- Single allocation strategy. No chunk lists, no fallback paths.
- Fixed at construction. No surprise growth at use-time.
- Fail-fast. Returns `AllocError` on overflow. The host handles it.
- `core`-only without `alloc`. The static-buffer constructor needs neither.
- Two ends with one budget. The user decides what each end means.

## Ecosystem Pitch

Existing arena crates serve different niches. `bumpalo` grows dynamically. `typed-arena` is type-monomorphic. `slab` and `generational-arena` are pool allocators. None of them combine fixed-size storage with a dual-end discipline and a generic budget contract. `keleusma-arena` fills that gap.

Targets it serves well.

- Embedded systems with link-time-allocated buffers. `from_static_buffer` accepts `&'static mut [u8]`.
- Multi-region targets like the Game Boy Advance. Construct one arena per memory region, namely IWRAM, EWRAM, VRAM.
- Real-time and safety-critical workloads. Constant-time allocation, no surprise pauses, sound bounds when paired with an analysis.
- Game engines and simulation loops. Reset the arena per frame. Allocate transient values without GC pressure.
- Any program that wants a compile-time memory budget contract.

## Quick Start

```rust
use keleusma_arena::Arena;
use core::alloc::Layout;

let arena = Arena::with_capacity(4096);

// StackHandle and HeapHandle are conventional aliases for BottomHandle
// and TopHandle. Use whichever names match your mental model.
let layout = Layout::new::<u64>();
let stack_alloc = allocator_api2::alloc::Allocator::allocate(
    &arena.bottom_handle(),
    layout,
).unwrap();
let heap_alloc = allocator_api2::alloc::Allocator::allocate(
    &arena.top_handle(),
    layout,
).unwrap();
```

## Static-Buffer Use

For embedded targets without a global allocator, use a statically allocated buffer.

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

// Collection backed by the arena.
let mut v: ArenaVec<i32, _> = ArenaVec::new_in(arena.bottom_handle());
v.push(1);

// LIFO discipline through marks.
let mark = arena.bottom_mark();
// ... allocate scratch ...
unsafe { arena.rewind_bottom(mark); }

// Budget contract.
let budget = Budget::new(2048, 1024);
assert!(arena.fits_budget(&budget));

// Observability.
println!("bottom peak: {}", arena.bottom_peak());
println!("top peak: {}", arena.top_peak());
```

## Naming

`BottomHandle` and `TopHandle` are the canonical handle names, matching a vertical-buffer mental model where the bottom end starts at low addresses and grows up while the top end starts at high addresses and grows down. `StackHandle` and `HeapHandle` aliases cover code that prefers the conventional CPU-memory mental model where one end is treated as a stack and the other as a heap. The arena imposes no semantic distinction; users map their concepts to the two ends.

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

- `alloc` (default). Enables `Arena::with_capacity` and `allocator-api2`'s `vec` and other alloc-dependent modules. Disable for `core`-only targets.

## License

MIT.

## Crate Family

- `keleusma`. The Keleusma scripting language and runtime.
- `keleusma-macros`. Procedural macros used by the runtime.
- `keleusma-arena`. This crate.
