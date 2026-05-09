//! Basic dual-end allocation with collections.
//!
//! Run with: `cargo run --example basic`

use allocator_api2::vec::Vec as ArenaVec;
use keleusma_arena::Arena;

fn main() {
    let arena = Arena::with_capacity(4096);

    // Bottom end: stack-like usage. Push integers as they come.
    let mut bottom: ArenaVec<i32, _> = ArenaVec::new_in(arena.bottom_handle());
    for i in 0..10 {
        bottom.push(i);
    }

    // Top end: heap-like usage. Allocate distinct buffers.
    let mut top: ArenaVec<f64, _> = ArenaVec::new_in(arena.top_handle());
    for i in 0..5 {
        top.push((i as f64) * 0.5);
    }

    println!("capacity:    {} bytes", arena.capacity());
    println!("bottom_used: {} bytes", arena.bottom_used());
    println!("top_used:    {} bytes", arena.top_used());
    println!("free:        {} bytes", arena.free());
    println!("bottom_peak: {} bytes", arena.bottom_peak());
    println!("top_peak:    {} bytes", arena.top_peak());

    println!("bottom: {:?}", bottom.as_slice());
    println!("top:    {:?}", top.as_slice());
}
