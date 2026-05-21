//! Architecture-specific cycle-counter abstractions.
//!
//! The [`CycleCounter`] trait is the extension point for adding new
//! target architectures. Each implementation reads the architecture's
//! cycle-counter register and returns a monotonic `u64` count of
//! cycles since some implementation-defined reference. Differences
//! between two reads taken on the same logical CPU give cycle counts.
//!
//! Built-in implementations cover x86_64 (RDTSC), AArch64 (CNTVCT_EL0),
//! Cortex-M (DWT_CYCCNT) under no_std, and an `Instant`-based fallback
//! that converts wall-clock nanoseconds to approximate cycles. The
//! Cortex-M and `Instant` paths are mutually exclusive: `Instant` is
//! only compiled under the `std` feature; `DwtCycCnt` is selected by
//! `default_counter` under no_std on Cortex-M targets.
//!
//! To add a new architecture, implement [`CycleCounter`] for a new
//! struct, add a `cfg` arm to [`default_counter`], and update the
//! README. The rest of the benchmark engine is architecture-independent.

#[cfg(feature = "std")]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::time::Instant;

/// Read the host's monotonic cycle counter. Implementations must be
/// reentrant and must return values that increase monotonically on
/// the same logical CPU.
pub trait CycleCounter: Send + Sync {
    /// Return the current cycle count. The absolute value is not
    /// meaningful; only differences between two reads on the same
    /// CPU produce useful counts.
    fn read(&self) -> u64;

    /// Short identifier for this counter implementation, used in
    /// generated output to record which counter measured the values.
    fn name(&self) -> &'static str;

    /// Conversion factor from one `read()` count to one CPU cycle.
    ///
    /// For counters that tick at CPU clock speed (x86_64 invariant
    /// TSC) this is `1.0`. For counters that tick at a fraction of
    /// CPU clock (AArch64 CNTVCT_EL0 on Apple Silicon at 24 MHz,
    /// where one tick is approximately 134 CPU cycles on a 3.2 GHz
    /// core) this is `assumed_cpu_hz / counter_hz`. For the
    /// nanosecond-resolution `Instant` fallback this is
    /// `assumed_cpu_hz / 1_000_000_000`.
    ///
    /// The implementation reads [`assumed_cpu_hz`] for the operating
    /// assumption about CPU clock speed. The default is documented
    /// per counter; the `KELEUSMA_BENCH_CPU_HZ` environment variable
    /// overrides it. Operators on hosts whose CPU clock differs
    /// from the default should set the variable before running the
    /// bench, otherwise the emitted cost model will be calibrated
    /// for the wrong host.
    fn cpu_cycles_per_count(&self) -> f64;

    /// Counter tick rate in Hz. Used in the emitted fragment's
    /// header for transparency.
    fn frequency_hz(&self) -> u64;
}

/// Default assumed CPU clock for the host. Used by counters whose
/// own tick rate differs from CPU clock and that therefore need a
/// scaling factor to convert ticks to CPU cycles. The default is
/// Apple M1 Max P-core nominal (3.228 GHz). Override per-host with
/// the `KELEUSMA_BENCH_CPU_HZ` environment variable.
pub const DEFAULT_ASSUMED_CPU_HZ: f64 = 3_228_000_000.0;

/// Read the operative assumed CPU clock frequency in Hz. Under `std`,
/// honors the `KELEUSMA_BENCH_CPU_HZ` environment variable. Under
/// no_std the environment variable is unavailable, and the function
/// returns [`DEFAULT_ASSUMED_CPU_HZ`] directly. Embedded targets that
/// need a different assumption build the bench with an architecture-
/// specific counter whose `cpu_cycles_per_count` returns `1.0`
/// (DWT_CYCCNT ticks at CPU clock by construction).
#[cfg(feature = "std")]
pub fn assumed_cpu_hz() -> f64 {
    std::env::var("KELEUSMA_BENCH_CPU_HZ")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|hz| hz.is_finite() && *hz > 0.0)
        .unwrap_or(DEFAULT_ASSUMED_CPU_HZ)
}

/// no_std build: the environment-variable override is unavailable.
/// Always returns [`DEFAULT_ASSUMED_CPU_HZ`]. Embedded counters
/// whose `cpu_cycles_per_count` is `1.0` (DWT_CYCCNT) do not consult
/// this value; the default is documented for completeness.
#[cfg(not(feature = "std"))]
pub fn assumed_cpu_hz() -> f64 {
    DEFAULT_ASSUMED_CPU_HZ
}

/// x86_64 cycle counter using the RDTSC instruction.
///
/// RDTSC reads the time-stamp counter, a processor-internal cycle
/// counter that increments at the processor's nominal frequency. On
/// modern Intel and AMD CPUs the TSC is invariant, meaning it
/// continues to increment at a constant rate independent of frequency
/// scaling, C-states, and P-states. This makes RDTSC a stable
/// cycle-count primitive. For older CPUs without invariant TSC, the
/// reading may drift relative to actual cycles under frequency
/// scaling.
#[cfg(target_arch = "x86_64")]
pub struct Rdtsc;

#[cfg(target_arch = "x86_64")]
impl CycleCounter for Rdtsc {
    fn read(&self) -> u64 {
        // SAFETY: `_rdtsc` is safe on all x86_64 CPUs that support
        // SSE2, which is mandatory in the x86_64 architecture.
        unsafe { core::arch::x86_64::_rdtsc() }
    }

    fn name(&self) -> &'static str {
        "x86_64 RDTSC"
    }

    fn cpu_cycles_per_count(&self) -> f64 {
        // Modern x86_64 invariant TSC ticks at the processor's nominal
        // frequency; one TSC tick is one CPU cycle by construction on
        // hosts with invariant TSC. Older x86_64 without invariant TSC
        // would need a separate calibration, but this bench targets
        // production x86_64 hosts.
        1.0
    }

    fn frequency_hz(&self) -> u64 {
        // The TSC ticks at the CPU's nominal clock. Without a portable
        // way to read the nominal frequency from CPUID, return the
        // operating assumption. Operators on x86_64 hosts whose nominal
        // clock differs from the default should set `KELEUSMA_BENCH_CPU_HZ`.
        assumed_cpu_hz() as u64
    }
}

/// AArch64 cycle counter using the CNTVCT_EL0 register.
///
/// CNTVCT_EL0 is the virtual counter at EL0, accessible from
/// userspace. It increments at the architectural counter frequency,
/// which is typically lower than the CPU clock frequency. The
/// resulting "cycles" are coarser than CPU cycles but stable across
/// frequency scaling. For benchmark purposes this is acceptable
/// because the measurement compares opcodes against each other under
/// the same counter; the absolute conversion to CPU cycles requires
/// reading CNTFRQ_EL0 separately.
#[cfg(target_arch = "aarch64")]
pub struct CntvctEl0;

#[cfg(target_arch = "aarch64")]
impl CycleCounter for CntvctEl0 {
    fn read(&self) -> u64 {
        let value: u64;
        // SAFETY: CNTVCT_EL0 is unprivileged on all AArch64
        // implementations and reads as a u64. The asm sequence has
        // no side effects beyond reading the register.
        unsafe {
            core::arch::asm!(
                "mrs {0}, cntvct_el0",
                out(reg) value,
                options(nomem, nostack, preserves_flags)
            );
        }
        value
    }

    fn name(&self) -> &'static str {
        "AArch64 CNTVCT_EL0"
    }

    fn cpu_cycles_per_count(&self) -> f64 {
        // CNTVCT_EL0 ticks at the architectural virtual counter
        // frequency (CNTFRQ_EL0). On Apple Silicon this is 24 MHz,
        // far below CPU clock. Convert ticks to CPU cycles using the
        // operator-supplied or default assumption.
        let counter_hz = self.frequency_hz() as f64;
        if counter_hz > 0.0 {
            assumed_cpu_hz() / counter_hz
        } else {
            1.0
        }
    }

    fn frequency_hz(&self) -> u64 {
        let freq: u64;
        // SAFETY: CNTFRQ_EL0 is unprivileged on all AArch64
        // implementations and reads as a u64.
        unsafe {
            core::arch::asm!(
                "mrs {0}, cntfrq_el0",
                out(reg) freq,
                options(nomem, nostack, preserves_flags)
            );
        }
        freq
    }
}

/// Cortex-M cycle counter using the DWT_CYCCNT register.
///
/// DWT (Data Watchpoint and Trace) is a debug peripheral present on
/// most ARMv7-M and ARMv8-M cores including the Cortex-M55 on the
/// STM32N6570-DK. DWT_CYCCNT ticks at CPU clock by construction, so
/// one tick is one CPU cycle and `cpu_cycles_per_count` returns
/// `1.0`. The counter must be enabled before use; the embedded
/// binary that constructs the counter is responsible for setting
/// DEMCR.TRCENA and DWT.CTRL.CYCCNTENA.
///
/// `frequency_hz` returns the CPU clock supplied at construction.
/// The bench's host-side runner records this value in the generated
/// fragment header so the resulting cost model carries the
/// calibration explicitly.
#[cfg(target_arch = "arm")]
pub struct DwtCycCnt {
    /// CPU clock in Hz at the time of measurement. Recorded in the
    /// emitted fragment header.
    pub cpu_hz: u64,
}

#[cfg(target_arch = "arm")]
impl DwtCycCnt {
    /// Construct a new counter rooted at the given CPU clock.
    pub fn new(cpu_hz: u64) -> Self {
        Self { cpu_hz }
    }
}

#[cfg(target_arch = "arm")]
impl CycleCounter for DwtCycCnt {
    fn read(&self) -> u64 {
        // DWT_CYCCNT is at address 0xE000_1004 on Cortex-M. Reading
        // it directly through volatile MMIO avoids pulling in the
        // cortex-m crate as a transitive dep of keleusma-bench.
        // The register is 32-bit; widen to u64. The bench's
        // wrapping_sub already handles u64 wrap; for u32 reads the
        // wrap interval is approximately 5 seconds at 800 MHz, which
        // is longer than any single bench measurement.
        const DWT_CYCCNT_ADDR: u32 = 0xE000_1004;
        // SAFETY: DWT_CYCCNT is a memory-mapped read-only register at
        // a fixed address documented in the ARMv7-M and ARMv8-M
        // architecture reference manuals. The address is valid on
        // Cortex-M cores with the DWT peripheral, which the bench
        // binary verifies at startup. Reading is side-effect-free.
        unsafe { core::ptr::read_volatile(DWT_CYCCNT_ADDR as *const u32) as u64 }
    }

    fn name(&self) -> &'static str {
        "Cortex-M DWT_CYCCNT"
    }

    fn cpu_cycles_per_count(&self) -> f64 {
        // DWT_CYCCNT ticks at CPU clock. One count is one CPU cycle.
        1.0
    }

    fn frequency_hz(&self) -> u64 {
        self.cpu_hz
    }
}

/// Portable fallback cycle counter using [`Instant::now`] and a
/// nominal cycles-per-nanosecond conversion. The conversion assumes a
/// 1 GHz reference clock, treating each elapsed nanosecond as one
/// nominal cycle. This is not accurate for any specific host but
/// provides a usable counter on architectures without a built-in
/// implementation, supporting cross-platform development workflows.
///
/// Generated cost models from the fallback counter should be marked
/// as approximate. Hosts with real cycle-counter hardware should
/// prefer the architecture-specific implementations.
#[cfg(feature = "std")]
pub struct InstantFallback {
    epoch: Instant,
}

#[cfg(feature = "std")]
impl InstantFallback {
    /// Construct a new fallback counter rooted at the current
    /// instant.
    pub fn new() -> Self {
        Self {
            epoch: Instant::now(),
        }
    }
}

#[cfg(feature = "std")]
impl Default for InstantFallback {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl CycleCounter for InstantFallback {
    fn read(&self) -> u64 {
        let elapsed = self.epoch.elapsed();
        elapsed.as_nanos() as u64
    }

    fn name(&self) -> &'static str {
        "Instant nanoseconds"
    }

    fn cpu_cycles_per_count(&self) -> f64 {
        // One read returns a nanosecond count. CPU cycles per
        // nanosecond is the CPU clock in GHz, i.e. assumed_cpu_hz / 1e9.
        assumed_cpu_hz() / 1_000_000_000.0
    }

    fn frequency_hz(&self) -> u64 {
        1_000_000_000
    }
}

/// Return a boxed default counter for the host architecture. Selects
/// the architecture-specific implementation when available, falling
/// back to [`InstantFallback`] under `std` on unknown architectures.
/// On Cortex-M (`target_arch = "arm"`) the default counter is not
/// supplied here because DWT_CYCCNT requires a CPU clock value that
/// the bench binary knows at construction time; embedded callers
/// construct [`DwtCycCnt`] directly.
#[cfg(feature = "std")]
pub fn default_counter() -> Box<dyn CycleCounter> {
    #[cfg(target_arch = "x86_64")]
    {
        Box::new(Rdtsc)
    }
    #[cfg(target_arch = "aarch64")]
    {
        Box::new(CntvctEl0)
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        Box::new(InstantFallback::new())
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn default_counter_reads_monotonically() {
        let counter = default_counter();
        let a = counter.read();
        // Spin briefly to advance the counter.
        let mut sum: u64 = 0;
        for i in 0..10_000u64 {
            sum = sum.wrapping_add(i);
        }
        core::hint::black_box(sum);
        let b = counter.read();
        assert!(b >= a, "counter should be monotonic");
    }

    #[test]
    fn default_counter_has_nonempty_name() {
        let counter = default_counter();
        assert!(!counter.name().is_empty());
    }
}
