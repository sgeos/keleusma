//! Cost-model calibration binary for the STM32N6570-DK.
//!
//! Boots embassy, enables DWT_CYCCNT on the Cortex-M55, runs the
//! keleusma-bench OPCODE_SPECS suite against the on-board CPU, and
//! emits each measurement through defmt RTT in a structured format
//! the host-side runner parses into a Rust source fragment.
//!
//! Flash and run:
//!
//! ```bash
//! cargo run --release --bin bench-n6 \
//!     --target thumbv8m.main-none-eabihf \
//!     --no-default-features --features stm32n6570dk-platform
//! ```
//!
//! Output format. Each measurement emits a single line of the form
//! `BENCH name=<spec> cycles_per_pattern=<f64> cycles_per_op=<u32>`
//! followed by a `BENCH_DONE` marker. The host runner matches these
//! lines exactly so the parser stays tolerant of unrelated defmt
//! output (boot banner, panic messages, etc.).
//!
//! DWT_CYCCNT setup. The bench enables the cycle counter directly
//! through volatile MMIO on DEMCR (`0xE000_EDFC`) and DWT.CTRL
//! (`0xE000_1000`). The cortex-m crate is on the dep list but the
//! direct register pokes avoid a dependency on the crate's
//! `peripherals_take` machinery, which would require additional
//! setup.
//!
//! CPU clock assumption. The N6's Cortex-M55 runs at 800 MHz nominal
//! after the bootloader has configured the clock tree. The bench
//! reports this clock in the output so the host runner can record
//! it in the fragment header.

#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::mem::MaybeUninit;

use defmt::info;
use embassy_executor::Spawner;
use embedded_alloc::LlffHeap as Heap;
use {defmt_rtt as _, panic_probe as _};

use keleusma_bench::counter::DwtCycCnt;
use keleusma_bench::{BenchConfig, OPCODE_SPECS, measure_one_with_config};

/// Global-allocator wrapper that intercepts zero-byte requests. Same
/// shape as the three-task binary; see `three_task_n6.rs` for the
/// rationale.
struct ZeroSizeOk<A>(A);

unsafe impl<A: GlobalAlloc> GlobalAlloc for ZeroSizeOk<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return layout.align() as *mut u8;
        }
        unsafe { self.0.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return layout.align() as *mut u8;
        }
        unsafe { self.0.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.size() == 0 {
            return;
        }
        unsafe { self.0.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size == 0 {
            return layout.align() as *mut u8;
        }
        if layout.size() == 0 {
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

/// Heap backing store. With [`BenchConfig::embedded_default`] each
/// spec's chunk holds 1,000 pattern repetitions of up to four ops
/// (24 KB at the current `Op` size of 6 bytes), plus the arena
/// (16 KB), plus rkyv-archived chunk metadata. 128 KB covers the
/// per-spec transients with margin for `linked_list_allocator`
/// fragmentation across the seventeen sequential spec builds.
const HEAP_SIZE: usize = 128 * 1024;
static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];

/// N6 Cortex-M55 nominal CPU clock after the bootloader configures
/// the PLL. Recorded in each measurement line and the trailing
/// `BENCH_DONE` marker so the host runner can stamp the generated
/// fragment header. The hardware's actual instantaneous clock can
/// differ under thermal throttling; the operator confirms the value
/// matches the deployment configuration.
const N6_CPU_HZ: u64 = 800_000_000;

/// Enable the DWT cycle counter. Requires writing the TRCENA bit in
/// DEMCR (`0xE000_EDFC`) to grant DWT access, then setting CYCCNTENA
/// in DWT.CTRL (`0xE000_1000`). The sequence is the canonical
/// "enable DWT_CYCCNT" boilerplate documented in the ARMv7-M and
/// ARMv8-M architecture reference manuals.
fn enable_dwt_cycle_counter() {
    const DEMCR: u32 = 0xE000_EDFC;
    const DWT_CTRL: u32 = 0xE000_1000;
    const DWT_CYCCNT: u32 = 0xE000_1004;
    const DEMCR_TRCENA: u32 = 1 << 24;
    const DWT_CTRL_CYCCNTENA: u32 = 1 << 0;
    // SAFETY: The DEMCR and DWT registers are at fixed memory-mapped
    // addresses on Cortex-M cores with the DWT peripheral. Writing
    // TRCENA grants debug-trace access; writing CYCCNTENA starts the
    // cycle counter. Both are documented configuration registers
    // with no side effects beyond the bit-flag semantics.
    unsafe {
        let demcr = DEMCR as *mut u32;
        core::ptr::write_volatile(demcr, core::ptr::read_volatile(demcr) | DEMCR_TRCENA);
        let ctrl = DWT_CTRL as *mut u32;
        core::ptr::write_volatile(ctrl, core::ptr::read_volatile(ctrl) | DWT_CTRL_CYCCNTENA);
        // Reset the counter to zero so the first read after the
        // bench's measurement loop sees a fresh interval.
        core::ptr::write_volatile(DWT_CYCCNT as *mut u32, 0);
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialise the heap. Safety: same as `three_task_n6.rs`.
    unsafe {
        let start = (&raw mut HEAP_MEM) as usize;
        HEAP.0.init(start, HEAP_SIZE);
    }

    // Bring up the chip. The bench does not use any platform
    // peripherals beyond the CPU itself, so the embassy init brings
    // up the clock tree and that is sufficient.
    let _p = embassy_stm32::init(Default::default());

    info!("=== Keleusma bench harness (N6 / Cortex-M55) ===");
    info!("CPU clock assumption: {=u64} Hz", N6_CPU_HZ);

    enable_dwt_cycle_counter();
    info!("DWT_CYCCNT enabled");

    let counter = DwtCycCnt::new(N6_CPU_HZ);
    let total = OPCODE_SPECS.len();
    info!("running {=usize} benchmark specs", total);

    let config = BenchConfig::embedded_default();
    for (i, spec) in OPCODE_SPECS.iter().enumerate() {
        let m = measure_one_with_config(&counter, spec, config);
        // Bits of f64 emitted directly so the host can reconstruct
        // the exact measurement without losing precision through a
        // decimal text intermediary. defmt cannot format f64
        // directly under no_std; the bit pattern is the safest
        // round-trip.
        let bits = m.cycles_per_pattern.to_bits();
        info!(
            "BENCH idx={=usize}/{=usize} name={=str} bits={=u64} per_op={=u32}",
            i + 1,
            total,
            m.name,
            bits,
            m.cycles_per_op
        );
    }

    info!("BENCH_DONE cpu_hz={=u64} counter_hz={=u64}", N6_CPU_HZ, N6_CPU_HZ);

    // Halt. The probe-rs runner detects the BENCH_DONE marker and
    // disconnects; the firmware loops until reset.
    loop {
        embassy_time::Timer::after_millis(1000).await;
    }
}
