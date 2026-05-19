//! Std-backed three-task demonstrator.
//!
//! Boots the kernel constructed by
//! `keleusma_rtos::setup::three_task_kernel` against the
//! [`StdPlatform`]. Drives the kernel's async `run` to
//! completion through a minimal `block_on`.
//!
//! Run with `cargo run --release --bin three-task-std`.
//! Ctrl-C to stop. The kernel runs forever; a later iteration
//! will add a wall-clock budget for CI.

use core::future::Future;
use core::pin::pin;
use core::task::{Context, Poll, Waker};

use keleusma_rtos::setup::three_task_kernel;
use keleusma_rtos::{Platform, StdPlatform};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut kernel = three_task_kernel::<StdPlatform>()?;

    println!("=== Keleusma RTOS demonstrator ===");
    println!(
        "Platform: {} (gpio_pin_count={}, sensor_channel_count={})",
        StdPlatform::NAME,
        StdPlatform::RESOURCES.gpio_pin_count,
        StdPlatform::RESOURCES.sensor_channel_count,
    );
    println!(
        "Tasks: led (500ms), sensor (100ms), heartbeat (5000ms), event_listener (on event 1), faulty (1500ms; faults every 5th iteration)"
    );
    println!("Kernel posts event 1 every 2500ms. Faulty task restarts under supervised policy.");
    println!("Press Ctrl-C to stop.");
    println!();

    block_on(kernel.run());
    Ok(())
}

/// Minimal `block_on` for the std demonstrator. Drives a
/// single top-level future to completion on the calling
/// thread. The std platform's `sleep_until` future never
/// goes pending (the thread sleep blocks synchronously inside
/// `poll`), so the busy-loop branch is structurally
/// reachable but never observed at runtime. The embassy port
/// uses the embassy executor instead.
fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = pin!(f);
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    loop {
        if let Poll::Ready(out) = f.as_mut().poll(&mut cx) {
            return out;
        }
        core::hint::spin_loop();
    }
}
