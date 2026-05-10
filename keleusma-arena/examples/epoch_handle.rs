//! Epoch-tagged stale-pointer detection through `ArenaHandle`.
//!
//! Demonstrates the load-bearing properties of the epoch system:
//!
//! 1. A handle constructed in the current epoch dereferences
//!    successfully.
//! 2. After `Arena::reset` advances the epoch, a handle from the
//!    prior epoch fails to dereference with a typed `Stale` error
//!    rather than returning a dangling reference.
//! 3. A new allocation in the new epoch produces a fresh handle
//!    whose epoch matches.
//!
//! `ArenaHandle<T>` is the generic mechanism. Higher-level helpers
//! (for example a string-handle wrapper in a downstream crate)
//! allocate typed storage in the arena and wrap the resulting
//! pointer through `ArenaHandle::from_raw_parts`.
//!
//! Run with: `cargo run --example epoch_handle`

use core::alloc::Layout;
use core::ptr::NonNull;

use allocator_api2::alloc::Allocator;
use keleusma_arena::{Arena, ArenaHandle};

/// Allocate a `u64` in the arena's top region, write `value`, and
/// return a stale-detecting handle.
fn alloc_u64(arena: &Arena, value: u64) -> ArenaHandle<u64> {
    let layout = Layout::new::<u64>();
    let raw = arena
        .top_handle()
        .allocate(layout)
        .expect("arena exhaustion");
    let typed: NonNull<u64> = raw.cast();
    // SAFETY: `typed` is freshly allocated unique storage of the
    // correct layout for `u64`.
    unsafe { typed.as_ptr().write(value) };
    // SAFETY: `typed` references storage in `arena`'s top region
    // freshly allocated under the current epoch and never aliased.
    unsafe { ArenaHandle::from_raw_parts(typed, arena.epoch()) }
}

fn main() {
    let mut arena = Arena::with_capacity(4096);

    // Epoch 0: allocate and wrap.
    let answer = alloc_u64(&arena, 42);
    assert_eq!(arena.epoch(), 0);
    assert_eq!(answer.epoch(), 0);
    println!(
        "epoch {}: answer = {}",
        arena.epoch(),
        answer.get(&arena).unwrap()
    );

    // Reset the arena. The epoch counter advances and the prior
    // handle is now stale.
    arena.reset().expect("reset");
    assert_eq!(arena.epoch(), 1);
    match answer.get(&arena) {
        Err(_) => println!(
            "epoch {}: prior handle correctly reported Stale",
            arena.epoch()
        ),
        Ok(_) => panic!("expected Stale after reset"),
    }

    // Epoch 1: a new allocation produces a fresh handle. The
    // arena's storage has been reused without freeing or
    // reallocating the underlying buffer.
    let again = alloc_u64(&arena, 7);
    assert_eq!(again.epoch(), 1);
    println!(
        "epoch {}: again = {}",
        arena.epoch(),
        again.get(&arena).unwrap()
    );

    println!(
        "epochs remaining before saturation: {}",
        arena.epoch_remaining()
    );
}
