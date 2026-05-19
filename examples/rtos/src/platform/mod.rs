//! The platform abstraction.
//!
//! The kernel core depends on this trait and nothing else from
//! the outside world. Each target (std for development,
//! embassy for embedded) provides one concrete implementation
//! in a sibling file under this directory. Porting the kernel
//! to a new chip is one new `Platform` impl plus the chip-
//! specific peripheral wiring; the kernel core does not change.
//!
//! `Platform::sleep_until` is async so an embassy executor can
//! await an `embassy_time::Timer`. The std implementation runs
//! the body synchronously (single-threaded thread sleep inside
//! the future's poll).
//!
//! Adding a new platform takes three steps:
//!
//! 1. Add a new file `src/platform/<name>.rs` defining the
//!    impl. The file is feature-gated so it only compiles
//!    when the matching cargo feature is enabled.
//! 2. Add the matching `#[cfg(feature = "...")]` lines below
//!    that declare the sub-module and re-export the platform
//!    type at the crate root.
//! 3. Add the feature flag to `Cargo.toml`.

#[cfg(feature = "std-platform")]
pub mod std;

#[cfg(feature = "stm32n6570dk-platform")]
pub mod stm32n6570_dk;

#[cfg(feature = "std-platform")]
pub use self::std::StdPlatform;

#[cfg(feature = "stm32n6570dk-platform")]
pub use self::stm32n6570_dk::Stm32N6570DkPlatform;

/// Static description of a platform's peripheral resources.
///
/// Implementations populate this at compile time via the
/// [`Platform::RESOURCES`] associated constant. Scripts and the
/// kernel can query the values to size allocators, validate
/// resource indices before issuing a hardware operation, or
/// expose the counts through natives for designer-side checks.
///
/// The field set is intentionally conservative: only resources
/// the shipped natives expose are listed. Adding a new resource
/// kind adds one field here, one implementation-side constant
/// per platform, and the corresponding native function.
#[derive(Clone, Copy, Debug)]
pub struct PlatformResources {
    /// Number of GPIO pins addressable through
    /// [`Platform::gpio_set`]. Pins are indexed `0..gpio_pin_count`.
    pub gpio_pin_count: u16,
    /// Number of analogue (or simulated) sensor channels
    /// addressable through [`Platform::sensor_read`].
    pub sensor_channel_count: u8,
    /// Number of independent UART / serial controllers.
    pub uart_count: u8,
    /// Number of independent SPI controllers.
    pub spi_count: u8,
    /// Number of independent I2C controllers.
    pub i2c_count: u8,
    /// Number of independent timer peripherals. The kernel's
    /// own time source does not count against this; this is
    /// the count available for application use.
    pub timer_count: u8,
}

/// The platform abstraction implemented per target.
///
/// Implementations are zero-sized or hold only static state.
/// The kernel uses the trait through a generic parameter, so
/// method calls resolve to direct function calls at
/// monomorphisation rather than dynamic dispatch.
pub trait Platform: 'static {
    /// Static description of this platform's peripheral
    /// resources. Read at boot to print a banner and at
    /// runtime to validate resource indices.
    const RESOURCES: PlatformResources;

    /// Short human-readable platform name. Used by the boot
    /// banner. Examples: `"std-host"`, `"stm32n6570-dk"`.
    const NAME: &'static str;

    /// Monotonic time since boot, in milliseconds. The clock
    /// must not wrap during the kernel's expected lifetime.
    fn now_ms() -> u64;

    /// Sleep cooperatively until the absolute monotonic time
    /// `at_ms` has passed. On hosted std the implementation
    /// blocks the calling thread inside the future's poll; on
    /// embassy the implementation awaits an
    /// `embassy_time::Timer`. The shape is the same so the
    /// kernel's await sites do not change across platforms.
    fn sleep_until(at_ms: u64) -> impl core::future::Future<Output = ()>;

    /// Emit a host-side log line. Used by the kernel for its
    /// own diagnostics (task scheduling errors, VM error
    /// surface). Scripts do not call this directly; they emit
    /// log events through [`log_event`](Self::log_event)
    /// instead, which carries a numeric event code and one
    /// data word and therefore does not require the `text`
    /// surface feature in the runtime.
    ///
    /// On std this routes to stdout via `println!`. On embedded
    /// targets this routes to defmt, RTT, or a UART driver.
    fn log(line: &str);

    /// Emit a script-originated log event. `code` is a
    /// caller-defined event discriminant; `data` is a single
    /// associated data word that the event-specific format
    /// string interpolates (use 0 when the event carries no
    /// data).
    ///
    /// The default body is a no-op so platforms that do not
    /// surface script logging continue to satisfy the trait.
    /// Concrete implementations dispatch on `code` to a fixed
    /// per-event format string. The script side and the host
    /// side must agree on the code-to-message mapping; the
    /// convention is documented in `scripts/*.kel` alongside
    /// the call sites.
    ///
    /// The split between [`log`](Self::log) and `log_event`
    /// keeps the script-side surface free of arbitrary strings.
    /// Scripts compile without the `text` feature, which
    /// removes the lexer, parser, and runtime support for
    /// string literals from the flash image.
    fn log_event(_code: u32, _data: i64) {}

    /// Pet the hardware watchdog. Called by the kernel at the
    /// top of each scheduler iteration. The default body is a
    /// no-op so platforms without a watchdog continue to
    /// satisfy the trait; platforms that arm a hardware
    /// watchdog override this method to reset the timer.
    ///
    /// The pet cadence is "once per scheduler iteration" rather
    /// than per-dispatch because each task's slice has a
    /// verified WCET bound. The scheduler iteration is the
    /// outer liveness signal that the cooperative kernel is
    /// still making progress; a hang at the kernel level (a
    /// driver, an interrupt storm, a hardware fault) prevents
    /// the iteration from completing and the watchdog fires.
    fn feed_watchdog() {}

    /// Set a GPIO-like output. The `pin` index must satisfy
    /// `pin < RESOURCES.gpio_pin_count`; the natives layer
    /// validates this before calling, so implementations may
    /// assume the index is in range.
    fn gpio_set(pin: u8, high: bool);

    /// Read an analogue or simulated sensor channel. The
    /// `channel` index must satisfy
    /// `channel < RESOURCES.sensor_channel_count`. Returns a
    /// sixteen-bit unsigned value scaled appropriately for the
    /// application.
    fn sensor_read(channel: u8) -> u16;

    /// Write one byte to a UART controller. `controller` must
    /// satisfy `controller < RESOURCES.uart_count`. Default
    /// body is a no-op so platforms with `uart_count == 0`
    /// satisfy the trait without further implementation. The
    /// natives layer rejects calls against platforms where the
    /// count is zero before reaching this method.
    fn usart_write(_controller: u8, _byte: u8) {}

    /// Read one byte from a UART controller. Returns the byte
    /// or 0 if no byte is available. The natives layer validates
    /// `controller < RESOURCES.uart_count` before calling.
    fn usart_read(_controller: u8) -> u8 {
        0
    }

    /// Write one byte to an SPI controller. `controller` must
    /// satisfy `controller < RESOURCES.spi_count`.
    fn spi_write(_controller: u8, _byte: u8) {}

    /// Read one byte from an SPI controller.
    fn spi_read(_controller: u8) -> u8 {
        0
    }

    /// Write one byte to a slave on an I2C controller.
    /// `controller` must satisfy `controller <
    /// RESOURCES.i2c_count`. `addr` is the seven-bit slave
    /// address. Address validation is the implementation's
    /// responsibility because the meaningful range depends on
    /// the bus topology.
    fn i2c_write(_controller: u8, _addr: u8, _byte: u8) {}

    /// Read one byte from a slave on an I2C controller.
    fn i2c_read(_controller: u8, _addr: u8) -> u8 {
        0
    }

    /// Read an ADC channel. `channel` must satisfy
    /// `channel < RESOURCES.sensor_channel_count` (the ADC
    /// channel count and the analogue sensor channel count are
    /// the same number on every supported platform). The
    /// default body forwards to [`Platform::sensor_read`] so
    /// implementations that already provide a simulated sensor
    /// satisfy this method without extra code.
    fn adc_read(channel: u8) -> u16 {
        Self::sensor_read(channel)
    }
}
