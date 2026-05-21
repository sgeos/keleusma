//! Architecture-specific cycle-counter abstractions.
//!
//! The [`CycleCounter`] trait is the extension point for adding new
//! target architectures. Each implementation reads the architecture's
//! cycle-counter register and returns a monotonic `u64` count of
//! cycles since some implementation-defined reference. Differences
//! between two reads taken on the same logical CPU give cycle counts.
//!
//! Built-in implementations cover x86_64 (RDTSC), AArch64 (CNTVCT_EL0),
//! and a portable [`Instant`]-based fallback that converts wall-clock
//! nanoseconds to approximate cycles via a calibration constant.
//!
//! To add a new architecture, implement [`CycleCounter`] for a new
//! struct, add a `cfg` arm to [`default_counter`], and update the
//! README. The rest of the benchmark engine is architecture-independent.

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

/// Read the operative assumed CPU clock frequency in Hz. Honors the
/// `KELEUSMA_BENCH_CPU_HZ` environment variable and falls back to
/// [`DEFAULT_ASSUMED_CPU_HZ`].
pub fn assumed_cpu_hz() -> f64 {
    std::env::var("KELEUSMA_BENCH_CPU_HZ")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|hz| hz.is_finite() && *hz > 0.0)
        .unwrap_or(DEFAULT_ASSUMED_CPU_HZ)
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
pub struct InstantFallback {
    epoch: Instant,
}

impl InstantFallback {
    /// Construct a new fallback counter rooted at the current
    /// instant.
    pub fn new() -> Self {
        Self {
            epoch: Instant::now(),
        }
    }
}

impl Default for InstantFallback {
    fn default() -> Self {
        Self::new()
    }
}

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
/// back to [`InstantFallback`] when the host architecture has no
/// built-in support.
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

#[cfg(test)]
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
        std::hint::black_box(sum);
        let b = counter.read();
        assert!(b >= a, "counter should be monotonic");
    }

    #[test]
    fn default_counter_has_nonempty_name() {
        let counter = default_counter();
        assert!(!counter.name().is_empty());
    }
}
