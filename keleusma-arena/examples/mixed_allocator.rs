//! Use the arena alongside the global allocator.
//!
//! The arena is not the program's global allocator. The standard
//! `Vec`, `String`, `Box`, and friends continue to use the global
//! allocator. The arena is reserved for scoped or per-frame work that
//! benefits from a fixed budget and predictable allocation timing.
//!
//! This example processes a batch of inputs. Each input expands into a
//! transient buffer of derived values that lives for one batch
//! iteration. The transient buffer is allocated from the arena. The
//! summary results are accumulated in a normal `Vec` that uses the
//! global allocator and persists across iterations.
//!
//! Run with: `cargo run --example mixed_allocator`

use allocator_api2::vec::Vec as ArenaVec;
use keleusma_arena::Arena;

fn main() {
    // Standard Vec uses the global allocator. Persists across the run.
    let mut summaries: Vec<i64> = Vec::new();

    // Arena holds transient per-iteration scratch. 2 KB is plenty.
    let mut arena = Arena::with_capacity(2 * 1024);

    let inputs: Vec<i64> = vec![10, 20, 30, 40, 50];

    for &input in &inputs {
        // Reset the arena at the start of each iteration. Allocations
        // from the previous iteration are reclaimed in bulk.
        arena.reset().expect("epoch saturated");

        // Transient buffer in the arena. Holds derived values for this
        // iteration only.
        let mut derived: ArenaVec<i64, _> = ArenaVec::new_in(arena.stack_handle());
        for k in 1..=4 {
            derived.push(input * k);
        }

        // Compute the summary using the transient buffer.
        let sum: i64 = derived.iter().sum();

        // Push the summary into the persistent Vec. This allocation
        // goes through the global allocator, not the arena.
        summaries.push(sum);

        println!(
            "input={} arena_used={} summary={}",
            input,
            arena.bottom_used(),
            sum
        );
    }

    // After the loop, the arena's transient data is gone. The
    // summaries Vec is intact.
    println!();
    println!("summaries: {:?}", summaries);
    println!("arena peak usage: {} bytes", arena.bottom_peak());
}
