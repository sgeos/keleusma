//! The std-backed platform implementation.
//!
//! Used for development on the developer's machine. The clock
//! is wall-clock since process start. `sleep_until` blocks the
//! calling thread. Logging goes to stdout. GPIO and sensor
//! operations are simulated: GPIO writes print a tagged line,
//! and the sensor produces a triangular wave so the
//! threshold-crossing logic in the demonstrator tasks has
//! something interesting to react to.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::platform::{Platform, PlatformResources};

static START: OnceLock<Instant> = OnceLock::new();

fn start() -> Instant {
    *START.get_or_init(Instant::now)
}

pub struct StdPlatform;

impl Platform for StdPlatform {
    const NAME: &'static str = "std-host";
    const RESOURCES: PlatformResources = PlatformResources {
        // Simulated GPIO. The host can render any pin index to
        // stdout; sixteen is enough for the demonstrator's
        // needs and matches the rogue-game expectation of a
        // small fixed set.
        gpio_pin_count: 16,
        // One simulated analogue channel (the triangular wave
        // on channel 0) plus one constant-valued channel
        // for tests that need a stable reading.
        sensor_channel_count: 2,
        // No real peripherals on the host. The values are
        // declared zero so a task that queries them sees
        // exactly the available count.
        uart_count: 0,
        spi_count: 0,
        i2c_count: 0,
        timer_count: 0,
    };

    fn now_ms() -> u64 {
        start().elapsed().as_millis() as u64
    }

    async fn sleep_until(at_ms: u64) {
        // Synchronous thread sleep inside an async function.
        // The future is single-poll: poll it once and the
        // thread blocks for the required duration, then the
        // future returns Ready. This is fine for the std
        // demonstrator because everything runs on one thread.
        // The embassy port replaces this with a proper async
        // timer that yields to the executor.
        let now = Self::now_ms();
        if at_ms > now {
            std::thread::sleep(Duration::from_millis(at_ms - now));
        }
    }

    fn log(line: &str) {
        let t = Self::now_ms();
        println!("[t={:>6}ms] {}", t, line);
    }

    fn gpio_set(pin: u8, high: bool) {
        let t = Self::now_ms();
        println!(
            "[t={:>6}ms] [gpio {:>2}] -> {}",
            t,
            pin,
            if high { "H" } else { "L" }
        );
    }

    fn sensor_read(channel: u8) -> u16 {
        // Simulated triangular wave on channel 0 with a period
        // of one second and a peak of 1500 (deliberately above
        // the demonstrator task's threshold of 1000 so the
        // alarm path fires periodically).
        //
        // Other channels return a constant for now; extend as
        // tests demand.
        match channel {
            0 => {
                let phase = (Self::now_ms() % 1000) as u16;
                if phase < 500 {
                    phase * 3
                } else {
                    (1000 - phase) * 3
                }
            }
            _ => 512,
        }
    }
}
