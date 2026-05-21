//! Bare-metal three-task demonstrator for the STM32N6570-DK.
//!
//! Boots embassy, installs the platform's peripheral handles,
//! constructs the same three-task kernel the std demonstrator
//! uses, and drives `Kernel::run` under the embassy executor.
//!
//! Flash and run:
//!
//! ```bash
//! cargo run --release --bin three-task-n6 \
//!     --target thumbv8m.main-none-eabihf \
//!     --no-default-features --features stm32n6570dk-platform
//! ```
//!
//! Set BOOT0 to the development position so probe-rs can take
//! control. The kernel runs forever; reset or detach the probe
//! to stop. The defmt RTT channel emits the boot banner, the
//! kernel-construction trace, and the periodic heartbeat log.
//! See `MANUAL.md` for hardware setup, log interpretation, and
//! troubleshooting.
//!
//! Memory budget. The kernel uses `alloc` collections (`Vec`,
//! `String`, `format!`) and leaks one arena per task. The
//! global allocator runs out of a 320 KB static region inside
//! AXISRAM2; the three per-task arenas (16 KB each, see
//! [`TASK_ARENA_CAPACITY`]) are leaked into the same region via
//! `Box::leak(Arena::with_capacity(...))`. The total fits inside
//! the RAM region declared in `memory.x` (384 KB) with margin
//! for stack and embassy executor state.

#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::mem::MaybeUninit;

use defmt::info;
use embassy_executor::Spawner;
use embedded_alloc::LlffHeap as Heap;
use keleusma_rtos::setup::three_task_kernel_with_arena_capacity;
use keleusma_rtos::{Platform, Stm32N6570DkPlatform};
use {defmt_rtt as _, panic_probe as _};

/// Global-allocator wrapper that intercepts zero-byte requests
/// before they reach the linked-list allocator.
///
/// `linked_list_allocator` (under `embedded-alloc::LlffHeap`)
/// returns an allocation error for zero-byte layouts, which
/// Rust's `alloc` crate then escalates to a
/// `memory allocation of N bytes failed` panic with N == 0.
/// The Rust allocator contract permits returning a dangling
/// well-aligned pointer for zero-byte requests; the wrapper
/// returns one for every `GlobalAlloc` entry point. `realloc`
/// is overridden as well so any zero-byte shrink turns into a
/// dangling pointer and any zero-old-byte grow allocates fresh.
struct ZeroSizeOk<A>(A);

unsafe impl<A: GlobalAlloc> GlobalAlloc for ZeroSizeOk<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return layout.align() as *mut u8;
        }
        // Safety: forwarded to the wrapped allocator; the
        // caller's invariants apply unchanged.
        unsafe { self.0.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return layout.align() as *mut u8;
        }
        // Safety: same justification as `alloc`. Forwarded
        // explicitly rather than relying on the default trait
        // impl so the zero-byte guard always wins.
        unsafe { self.0.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.size() == 0 {
            return;
        }
        // Safety: forwarded to the wrapped allocator; the
        // caller's invariants apply unchanged.
        unsafe { self.0.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size == 0 {
            unsafe { self.dealloc(ptr, layout) };
            return layout.align() as *mut u8;
        }
        if layout.size() == 0 {
            // Old layout was zero-byte (dangling pointer); we
            // need a fresh allocation of `new_size`.
            let new_layout = match Layout::from_size_align(new_size, layout.align()) {
                Ok(l) => l,
                Err(_) => return core::ptr::null_mut(),
            };
            return unsafe { self.0.alloc(new_layout) };
        }
        unsafe { self.0.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static HEAP: ZeroSizeOk<Heap> = ZeroSizeOk(Heap::empty());

/// Heap backing store. Sized to cover three leaked task
/// arenas (3 * [`TASK_ARENA_CAPACITY`] = 48 KB) plus the
/// compile-pipeline transient state for each task (`Vec`,
/// `String`, `BTreeMap`, AST/bytecode/module structures), with
/// margin for `linked_list_allocator` fragmentation between the
/// sequential task builds. Reduced from 256 KB to 192 KB to
/// accommodate the further FLASH-region growth from the V0.2
/// interval-lattice and refinement-elision infrastructure.
const HEAP_SIZE: usize = 192 * 1024;
static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];

/// Per-task arena capacity for the embedded build. The
/// demonstrator scripts have small operand stacks and short
/// call chains, so 16 KB per arena is comfortably above their
/// runtime working set while keeping the global heap footprint
/// modest. The std demonstrator continues to use
/// [`keleusma::vm::DEFAULT_ARENA_CAPACITY`] (64 KB) via the
/// no-argument `three_task_kernel` wrapper.
const TASK_ARENA_CAPACITY: usize = 16 * 1024;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialise the heap. Safety: the heap memory is a static
    // buffer with the size matching the allocator's expected
    // bounds; the allocator takes exclusive control of the
    // region for the rest of the program. Edition 2024 forbids
    // `&mut` references to mutable statics; `&raw mut` produces
    // a raw pointer without going through a reference, which is
    // the documented idiom for one-time heap initialisation.
    unsafe {
        let start = (&raw mut HEAP_MEM) as usize;
        HEAP.0.init(start, HEAP_SIZE);
    }

    // Bring up the chip and pull out the peripheral handles the
    // platform owns.
    let p = embassy_stm32::init(Default::default());
    Stm32N6570DkPlatform::install(p.PG10);

    info!("=== Keleusma RTOS three-task demonstrator (N6) ===");
    info!(
        "Platform: {=str} (gpio_pin_count={}, sensor_channel_count={})",
        Stm32N6570DkPlatform::NAME,
        Stm32N6570DkPlatform::RESOURCES.gpio_pin_count,
        Stm32N6570DkPlatform::RESOURCES.sensor_channel_count,
    );
    info!("Tasks: led (500ms), sensor (100ms), heartbeat (5000ms)");

    // Boot-time per-task WCET report. Computes the per-iteration
    // cycle bound for each task under both the bundled
    // `NOMINAL_COST_MODEL` (relative weights) and the
    // target-specific `MEASURED_COST_MODEL` (CPU cycles on the
    // Cortex-M55 at 800 MHz, from the keleusma-bench measurement on
    // this exact hardware family). The numbers are reported in
    // parallel so operators see both at boot; downstream scheduler
    // decisions consume the measured value where the bench
    // calibration is trustworthy and the nominal value where
    // platform-portable ordering is wanted.
    //
    // Gated on `keleusma-verify` because the report calls
    // `keleusma::verify::wcet_stream_iteration_with_cost_model`,
    // which is only available when the verifier ships in the image.
    #[cfg(feature = "keleusma-verify")]
    {
        info!("--- WCET (per iteration) ---");
        for (name, bytecode) in [
            ("led", keleusma_rtos::setup::BIN_LED),
            ("sensor", keleusma_rtos::setup::BIN_SENSOR),
            ("heartbeat", keleusma_rtos::setup::BIN_HEARTBEAT),
        ] {
            match keleusma_rtos::cost_model::report_measured_wcet(bytecode) {
                Some((nominal, measured)) => info!(
                    "task `{=str}`: NOMINAL {=u32} cycles  MEASURED {=u32} cycles",
                    name, nominal, measured
                ),
                None => info!("task `{=str}`: no Stream chunk; WCET report skipped", name),
            }
        }
    }

    // Boot-time signature self-test. Runs only when the
    // `keleusma-signatures` feature is on; otherwise the path is
    // compiled away. A failure here means the cryptographic
    // verifier links into the image but does not function on this
    // target; the firmware refuses to enter the scheduler loop.
    #[cfg(feature = "keleusma-signatures")]
    {
        match keleusma_rtos::setup::run_signed_self_test() {
            Ok(()) => info!("signed self-test: verify_module_signature succeeded"),
            Err(reason) => {
                defmt::error!("signed self-test failed: {=str}", reason);
                loop {
                    embassy_time::Timer::after_millis(1000).await;
                }
            }
        }
    }

    let mut kernel =
        match three_task_kernel_with_arena_capacity::<Stm32N6570DkPlatform>(TASK_ARENA_CAPACITY) {
            Ok(k) => k,
            Err(e) => {
                defmt::error!("kernel construction failed: {=str}", e.as_str());
                loop {
                    embassy_time::Timer::after_millis(1000).await;
                }
            }
        };

    info!("kernel: scheduler entering loop");
    kernel.run().await;
}
