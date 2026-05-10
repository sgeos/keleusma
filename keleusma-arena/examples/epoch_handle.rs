//! Epoch-tagged stale-pointer detection through `ArenaHandle` and
//! `KString`.
//!
//! Demonstrates the load-bearing properties of the epoch system:
//!
//! 1. A handle allocated in the current epoch dereferences successfully.
//! 2. After `Arena::reset` advances the epoch, a handle from the prior
//!    epoch fails to dereference with a typed `Stale` error rather than
//!    returning a dangling reference.
//! 3. A new allocation in the new epoch succeeds and produces a fresh
//!    handle whose epoch matches.
//!
//! Run with: `cargo run --example epoch_handle`

use keleusma_arena::{Arena, KString};

fn main() {
    let mut arena = Arena::with_capacity(4096);

    // Epoch 0: allocate a string handle.
    let greeting = KString::alloc(&arena, "hello, arena").expect("alloc");
    assert_eq!(arena.epoch(), 0);
    assert_eq!(greeting.epoch(), 0);
    println!("epoch {}: {}", arena.epoch(), greeting.get(&arena).unwrap());

    // Reset the arena. The epoch counter advances and the handle from
    // epoch 0 is now stale.
    arena.reset().expect("reset");
    assert_eq!(arena.epoch(), 1);
    match greeting.get(&arena) {
        Err(_) => println!(
            "epoch {}: prior handle correctly reported Stale",
            arena.epoch()
        ),
        Ok(_) => panic!("expected Stale after reset"),
    }

    // Epoch 1: a new allocation produces a fresh handle that
    // dereferences successfully. The shape is the same as before; the
    // arena's storage has been reused without freeing or reallocating.
    let farewell = KString::alloc(&arena, "and again").expect("alloc");
    assert_eq!(farewell.epoch(), 1);
    println!("epoch {}: {}", arena.epoch(), farewell.get(&arena).unwrap());

    // Observability. The arena reports remaining epochs before
    // saturation. Saturation is a hard halt that requires unsafe
    // recovery via `force_reset_epoch`.
    println!(
        "epochs remaining before saturation: {}",
        arena.epoch_remaining()
    );
}
