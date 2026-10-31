#![cfg(all(feature = "compile", feature = "verify"))]
//! The `i32` row of the target-descriptor runtime grid.
//!
//! Every target descriptor the compiler accepts, compiled against the corpus in
//! `tests/target_descriptor_axis/harness.rs` and loaded on the eight runtimes
//! whose word is `i32`, asking whether any cell reaches a run-time
//! `InvalidBytecode`. The harness module documents the axis, the cell outcomes
//! and the reason the grid is split across four binaries.

extern crate alloc;

#[path = "target_descriptor_axis/harness.rs"]
mod harness;

use harness::{Tally, assert_every_grid_file_is_present, assert_grid_row, sweep_word};

/// The `i32` row of the grid, paired with every implemented address on both
/// float runtimes.
#[test]
fn no_runtime_and_module_width_pair_reaches_invalid_bytecode() {
    let mut t = Tally::default();
    sweep_word::<i32>(&mut t);
    assert_grid_row(&t, "i32", false);
}

/// The other three rows still exist and still sweep their word types.
#[test]
fn every_word_row_of_the_grid_is_present() {
    assert_every_grid_file_is_present();
}
