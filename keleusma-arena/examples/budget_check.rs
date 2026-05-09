//! Compute a memory budget and verify it against the arena.
//!
//! The Budget type is producer-agnostic. Real producers compute budgets
//! through static analysis, profiling, or measurement. This example
//! demonstrates the contract using hand-computed bounds.
//!
//! Run with: `cargo run --example budget_check`

use keleusma_arena::{Arena, Budget};

fn main() {
    // Budget computed by some upstream analysis.
    let budget = Budget::new(2048, 1024);

    // Adequate arena.
    let large = Arena::with_capacity(4096);
    println!(
        "large arena fits budget? {} (capacity {}, budget total {})",
        large.fits_budget(&budget),
        large.capacity(),
        budget.total()
    );

    // Inadequate arena.
    let small = Arena::with_capacity(2048);
    println!(
        "small arena fits budget? {} (capacity {}, budget total {})",
        small.fits_budget(&budget),
        small.capacity(),
        budget.total()
    );

    // Round-trip: size an arena from the budget plus a 25 percent safety
    // margin.
    let safety_factor = 5; // multiply by 5/4 = 25 percent margin
    let sized_capacity = (budget.total() * safety_factor) / 4;
    let sized = Arena::with_capacity(sized_capacity);
    println!(
        "sized arena fits budget? {} (capacity {}, budget total {})",
        sized.fits_budget(&budget),
        sized.capacity(),
        budget.total()
    );
}
