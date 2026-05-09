//! Per-frame reset pattern for game and simulation loops.
//!
//! The arena is reset at the top of each frame. Allocations made during
//! a frame are reclaimed in bulk at the next reset. The peak watermarks
//! accumulate across frames so the worst-case memory pressure can be
//! observed across the run.
//!
//! Run with: `cargo run --example frame_loop`

use allocator_api2::vec::Vec as ArenaVec;
use keleusma_arena::Arena;

const FRAME_COUNT: usize = 5;

fn main() {
    let mut arena = Arena::with_capacity(8192);

    for frame in 0..FRAME_COUNT {
        // Reset reclaims all allocations from the previous frame.
        arena.reset().expect("epoch saturated");

        // Each frame allocates a transient buffer for visible entities
        // from the stack-like region.
        let mut entities: ArenaVec<u64, _> = ArenaVec::new_in(arena.stack_handle());
        let entity_count = (frame + 1) * 4;
        for i in 0..entity_count {
            entities.push(((frame * 100) + i) as u64);
        }

        // Each frame also allocates command buffers from the heap-like
        // region.
        let mut commands: ArenaVec<u32, _> = ArenaVec::new_in(arena.heap_handle());
        for i in 0..(entity_count / 2) {
            commands.push(i as u32);
        }

        println!(
            "frame {}: entities={} commands={} stack={} heap={}",
            frame,
            entities.len(),
            commands.len(),
            arena.bottom_used(),
            arena.top_used()
        );
    }

    println!();
    println!("after run:");
    println!("  stack peak: {} bytes", arena.bottom_peak());
    println!("  heap peak:  {} bytes", arena.top_peak());
    println!(
        "  total peak: {} bytes (use this to size the arena)",
        arena.bottom_peak() + arena.top_peak()
    );
}
