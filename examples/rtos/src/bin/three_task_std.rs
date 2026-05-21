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

    // Boot-time per-task WCET report. Same structure as the N6
    // demonstrator's defmt log. On the host build for
    // aarch64-apple-darwin the measured model is the M1 Max
    // fragment calibrated at 3.228 GHz. On other host
    // architectures the measured model falls back to the bundled
    // `NOMINAL_COST_MODEL` and the two columns coincide. Gated on
    // the `keleusma-verify` feature; without it,
    // `wcet_stream_iteration_with_cost_model` is not in scope.
    #[cfg(all(feature = "keleusma-verify", feature = "keleusma-compile"))]
    {
        println!();
        println!("--- WCET (per iteration) ---");
        for (name, source) in [
            ("led", keleusma_rtos::setup::SRC_LED),
            ("sensor", keleusma_rtos::setup::SRC_SENSOR),
            ("heartbeat", keleusma_rtos::setup::SRC_HEARTBEAT),
            (
                "event_listener",
                keleusma_rtos::setup::SRC_EVENT_LISTENER,
            ),
            ("faulty", keleusma_rtos::setup::SRC_FAULTY),
        ] {
            // Task scripts reference `Status` and friends from the
            // shared prelude; the prelude must be prepended for
            // compilation to succeed.
            let combined = format!("{}\n{}", keleusma_rtos::setup::PRELUDE, source);
            match keleusma_rtos::cost_model::report_measured_wcet_from_source(&combined) {
                Some((nominal, measured)) => println!(
                    "task `{}`: NOMINAL {} cycles  MEASURED {} cycles",
                    name, nominal, measured
                ),
                None => println!("task `{}`: no Stream chunk; WCET report skipped", name),
            }
        }
    }

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
