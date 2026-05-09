//! Basic dual-end allocation with collections.
//!
//! Run with: `cargo run --example basic`

use allocator_api2::vec::Vec as ArenaVec;
use keleusma_arena::Arena;

fn main() {
    let arena = Arena::with_capacity(4096);

    // Stack-like region: push integers as they come.
    let mut stack: ArenaVec<i32, _> = ArenaVec::new_in(arena.stack_handle());
    for i in 0..10 {
        stack.push(i);
    }

    // Heap-like region: allocate distinct buffers.
    let mut heap: ArenaVec<f64, _> = ArenaVec::new_in(arena.heap_handle());
    for i in 0..5 {
        heap.push((i as f64) * 0.5);
    }

    println!("capacity:    {} bytes", arena.capacity());
    println!("stack used:  {} bytes", arena.bottom_used());
    println!("heap used:   {} bytes", arena.top_used());
    println!("free:        {} bytes", arena.free());
    println!("stack peak:  {} bytes", arena.bottom_peak());
    println!("heap peak:   {} bytes", arena.top_peak());

    println!("stack: {:?}", stack.as_slice());
    println!("heap:  {:?}", heap.as_slice());
}
