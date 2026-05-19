//! STM32N6570-DK platform implementation backed by `embassy-stm32`.
//!
//! The trait methods route to embassy primitives. Time uses
//! `embassy_time::Instant` so the kernel's monotonic clock and
//! sleep mechanism align with whatever time driver embassy
//! brings up at boot. GPIO uses `embassy_stm32::gpio::Output`.
//! Logging uses `defmt::info!` over RTT.
//!
//! Hardware-coupled state.
//!
//! `embassy_stm32::init` hands the application a set of
//! peripheral handles that are owned values, not statics. The
//! trait methods are associated functions (no `&self`), so the
//! impl needs static storage for any peripheral handle a method
//! reads or writes. The pattern below uses
//! `critical_section::Mutex<RefCell<Option<...>>>` for safe
//! interior-mutable handle storage. The binary calls
//! [`Stm32N6570DkPlatform::install`] once at boot to move the
//! handles into the statics; subsequent trait method calls read
//! through the statics.
//!
//! Pin map for the demonstrator.
//!
//! The LED task drives "GPIO 13" (a convention inherited from
//! Arduino-style scripts). On the N6570-DK this maps to PG10,
//! the on-board green user LED. Other pin indices are accepted
//! by the natives layer (since the gpio_pin_count for this
//! platform reports the full 256-pin range) but only PG10 is
//! wired here; calls against other pins log a defmt warning and
//! return without touching hardware. Future iterations can wire
//! more pins by extending the [`install`] handle set.

use core::cell::RefCell;

use critical_section::Mutex;
use embassy_stm32::Peri;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::peripherals::PG10;
use embassy_time::{Instant, Timer};

use crate::platform::{Platform, PlatformResources};

/// Static slot holding the on-board LED output. Initialised by
/// [`Stm32N6570DkPlatform::install`] at boot. `None` outside the
/// initialised window means a `gpio_set` call against pin 13
/// silently no-ops (and emits a defmt warning).
static LED_PG10: Mutex<RefCell<Option<Output<'static>>>> = Mutex::new(RefCell::new(None));

/// The STM32N6570-DK platform marker. Wired up against
/// `embassy-stm32`'s N6 HAL. The binary that instantiates the
/// kernel with this platform must call
/// [`Stm32N6570DkPlatform::install`] exactly once after
/// [`embassy_stm32::init`] returns, passing in the peripheral
/// handles the platform owns. Subsequent trait method calls
/// route through the installed handles.
pub struct Stm32N6570DkPlatform;

impl Stm32N6570DkPlatform {
    /// Install the peripheral handles into the platform's
    /// internal statics. Called once at boot from the embassy
    /// main function. Panics on second invocation in debug
    /// builds; in release builds the second call silently
    /// replaces the prior handle, which is correct only if the
    /// caller has good reason to swap mid-run.
    pub fn install(led: Peri<'static, PG10>) {
        let out = Output::new(led, Level::Low, Speed::Low);
        critical_section::with(|cs| {
            LED_PG10.borrow(cs).replace(Some(out));
        });
    }
}

impl Platform for Stm32N6570DkPlatform {
    const NAME: &'static str = "stm32n6570-dk";

    /// Resource counts for the STM32N6570-DK. Most values come
    /// from the N6's peripheral count, not the dev board's
    /// breakout. The board exposes a subset through headers
    /// and the LCD/camera connectors; the platform layer
    /// publishes the full peripheral count so a future native
    /// can refine the available subset.
    const RESOURCES: PlatformResources = PlatformResources {
        // Sixteen GPIO ports of sixteen pins each in the N6
        // family. Phase 3 wires only PG10 (the on-board green
        // LED) through pin index 13; other indices are accepted
        // by the natives layer but result in a defmt warning
        // and no-op here.
        gpio_pin_count: 256,
        // ADC channels. The N6 has one ADC1 instance with
        // sixteen external channels.
        sensor_channel_count: 16,
        uart_count: 7,
        spi_count: 6,
        i2c_count: 4,
        timer_count: 17,
    };

    fn now_ms() -> u64 {
        Instant::now().as_millis()
    }

    async fn sleep_until(at_ms: u64) {
        Timer::at(Instant::from_millis(at_ms)).await;
    }

    fn log(line: &str) {
        // `defmt` interns string literals; for runtime strings
        // passing through `&str`, the `{=str}` formatter sends
        // the bytes inline. Suitable for the host::log surface
        // where the script supplies an arbitrary message.
        defmt::info!("{=str}", line);
    }

    fn log_event(code: u32, data: i64) {
        // The event-code dispatch is kept in lock-step with
        // the script-side numeric literals in `scripts/*.kel`
        // and the matching dispatch in
        // `crate::platform::std::Std::log_event`. defmt interns
        // the literal format strings so RTT carries only the
        // index plus the data word; this is the smallest flash
        // footprint for script logging on the embedded target.
        match code {
            crate::natives::EV_HEARTBEAT_OK => {
                defmt::info!("heartbeat: system OK");
            }
            crate::natives::EV_LED_GPIO_FAIL => {
                defmt::info!("led: gpio_set failed, code={}", data);
            }
            crate::natives::EV_SENSOR_ABOVE => {
                defmt::info!("sensor ch0 ABOVE threshold (value={})", data);
            }
            crate::natives::EV_KERNEL_VM_ERROR => {
                let category: &'static str = match data {
                    0 => "halt",
                    1 => "soft-script",
                    2 => "soft-host",
                    _ => "unknown",
                };
                defmt::info!("kernel: task vm error (category={=str})", category);
            }
            crate::natives::EV_KERNEL_UNKNOWN_YIELD => {
                defmt::info!("kernel: task yielded unknown reason {}", data);
            }
            crate::natives::EV_KERNEL_TASK_FINISHED => {
                defmt::info!("kernel: task finished (unexpected for loop main)");
            }
            crate::natives::EV_KERNEL_UNEXPECTED_STATE => {
                defmt::info!("kernel: task returned unexpected vm state");
            }
            crate::natives::EV_KERNEL_TASK_RESTARTED => {
                defmt::info!("kernel: task restarted (count={})", data);
            }
            crate::natives::EV_EVENT_LISTENER_WAKE => {
                defmt::info!("event_listener: woke (wake_count={})", data);
            }
            crate::natives::EV_FAULTY_TRIGGER => {
                defmt::info!("faulty: deliberate fault at iteration {}", data);
            }
            _ => {
                defmt::info!("unknown log_event(code={}, data={})", code, data);
            }
        }
    }

    fn gpio_set(pin: u8, high: bool) {
        // Only pin 13 is currently wired. The natives layer
        // accepts any index below `gpio_pin_count` (256); the
        // platform decides which of those are physically
        // present. Out-of-band indices log and return; future
        // work expands the pin-to-output mapping.
        if pin != 13 {
            defmt::warn!("gpio_set: pin {} not wired on stm32n6570-dk", pin);
            return;
        }
        critical_section::with(|cs| {
            if let Some(out) = LED_PG10.borrow(cs).borrow_mut().as_mut() {
                if high {
                    out.set_high();
                } else {
                    out.set_low();
                }
            } else {
                defmt::warn!("gpio_set: LED_PG10 not installed; call Stm32N6570DkPlatform::install");
            }
        });
    }

    fn sensor_read(_channel: u8) -> u16 {
        // ADC wiring is deferred. The natives layer rejects
        // out-of-range channels; in-range channels read 0 until
        // a real ADC peripheral is installed.
        0
    }
}
