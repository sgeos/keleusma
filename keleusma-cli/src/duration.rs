//! Humanized duration parser for the `--tick-interval` flag and
//! the `shell::set_tick_interval` native.
//!
//! Accepts a single integer followed by a unit suffix:
//! - `Nms` — milliseconds
//! - `Ns`  — seconds
//! - `Nm`  — minutes
//! - `Nh`  — hours
//! - `Nd`  — days
//! - `Nw`  — weeks
//!
//! Maximum representable interval is 4 weeks. Operators who need
//! longer cadences should either use an external scheduler (cron,
//! systemd timers) or implement noop yield cycles in the script
//! that count internal ticks against the longer interval. See
//! `docs/guide/SECURITY_POLICY.md` for guidance.
//!
//! Composite forms like `1h30m` are NOT supported. Operators
//! should express composite durations as a single unit
//! (`1h30m` -> `90m`).

use std::time::Duration;

/// Maximum admitted duration. Four weeks. Longer cadences should
/// use external scheduling (cron) or noop yield cycles within the
/// script.
pub const MAX_DURATION_SECS: u64 = 4 * 7 * 24 * 60 * 60; // 4 weeks

/// Parse a humanized duration string into a [`Duration`]. Returns
/// an error message suitable for CLI or VmError output on
/// malformed input.
pub fn parse(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err(String::from("duration: empty input"));
    }

    // Split into numeric prefix and unit suffix. The numeric
    // portion is the longest leading run of ASCII digits.
    let split = s
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit())
        .map(|(i, _)| i)
        .unwrap_or(s.len());

    if split == 0 {
        return Err(format!("duration: expected integer prefix, got `{}`", s));
    }

    let numeric: u64 = s[..split]
        .parse()
        .map_err(|e| format!("duration: cannot parse `{}` as integer: {}", &s[..split], e))?;

    let unit = &s[split..];
    let seconds: u64 = match unit {
        "ms" => {
            // Milliseconds: special-case, we want Duration::from_millis.
            let total_ms = numeric;
            let dur = Duration::from_millis(total_ms);
            if dur.as_secs() > MAX_DURATION_SECS {
                return Err(format!(
                    "duration: {} exceeds maximum of 4 weeks; use cron or noop yield cycles for longer cadences",
                    s
                ));
            }
            return Ok(dur);
        }
        "s" => numeric,
        "m" => numeric
            .checked_mul(60)
            .ok_or_else(|| format!("duration: {} overflows when converted to seconds", s))?,
        "h" => numeric
            .checked_mul(3600)
            .ok_or_else(|| format!("duration: {} overflows when converted to seconds", s))?,
        "d" => numeric
            .checked_mul(86_400)
            .ok_or_else(|| format!("duration: {} overflows when converted to seconds", s))?,
        "w" => numeric
            .checked_mul(7 * 86_400)
            .ok_or_else(|| format!("duration: {} overflows when converted to seconds", s))?,
        "" => {
            return Err(format!(
                "duration: missing unit suffix on `{}`; expected ms, s, m, h, d, or w",
                s
            ));
        }
        other => {
            return Err(format!(
                "duration: unknown unit `{}` on `{}`; expected ms, s, m, h, d, or w",
                other, s
            ));
        }
    };

    if seconds > MAX_DURATION_SECS {
        return Err(format!(
            "duration: {} exceeds maximum of 4 weeks; use cron or noop yield cycles for longer cadences",
            s
        ));
    }

    Ok(Duration::from_secs(seconds))
}

/// Format a [`Duration`] as a humanized string. Used by the
/// `shell::tick_interval` getter native to return the current
/// interval to a script. Picks the largest unit that represents
/// the duration without fractional remainder. Falls back to ms
/// if the value is sub-second.
pub fn format(d: Duration) -> String {
    let ms = d.as_millis() as u64;
    if ms == 0 {
        return String::from("0ms");
    }
    if ms.is_multiple_of(7 * 86_400_000) {
        return format!("{}w", ms / (7 * 86_400_000));
    }
    if ms.is_multiple_of(86_400_000) {
        return format!("{}d", ms / 86_400_000);
    }
    if ms.is_multiple_of(3_600_000) {
        return format!("{}h", ms / 3_600_000);
    }
    if ms.is_multiple_of(60_000) {
        return format!("{}m", ms / 60_000);
    }
    if ms.is_multiple_of(1000) {
        return format!("{}s", ms / 1000);
    }
    format!("{}ms", ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_milliseconds() {
        assert_eq!(parse("1ms").unwrap(), Duration::from_millis(1));
        assert_eq!(parse("100ms").unwrap(), Duration::from_millis(100));
        assert_eq!(parse("999ms").unwrap(), Duration::from_millis(999));
    }

    #[test]
    fn parse_seconds() {
        assert_eq!(parse("1s").unwrap(), Duration::from_secs(1));
        assert_eq!(parse("60s").unwrap(), Duration::from_secs(60));
    }

    #[test]
    fn parse_minutes_hours_days_weeks() {
        assert_eq!(parse("1m").unwrap(), Duration::from_secs(60));
        assert_eq!(parse("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse("1d").unwrap(), Duration::from_secs(86_400));
        assert_eq!(parse("1w").unwrap(), Duration::from_secs(7 * 86_400));
        assert_eq!(parse("4w").unwrap(), Duration::from_secs(28 * 86_400));
    }

    #[test]
    fn rejects_over_four_weeks() {
        assert!(parse("5w").is_err());
        assert!(parse("30d").is_err());
        assert!(parse("100h").is_ok()); // 100h = ~4.1 days, OK
        assert!(parse("1000h").is_err()); // 1000h = ~5.9 weeks, rejected
    }

    #[test]
    fn rejects_empty() {
        assert!(parse("").is_err());
        assert!(parse("   ").is_err());
    }

    #[test]
    fn rejects_no_unit() {
        assert!(parse("100").is_err());
    }

    #[test]
    fn rejects_unknown_unit() {
        assert!(parse("100x").is_err());
        assert!(parse("100ns").is_err()); // nanoseconds not supported
        assert!(parse("100us").is_err()); // microseconds not supported
    }

    #[test]
    fn rejects_no_prefix() {
        assert!(parse("ms").is_err());
        assert!(parse("h").is_err());
    }

    #[test]
    fn rejects_composite() {
        // Composite forms are not supported.
        assert!(parse("1h30m").is_err());
        assert!(parse("1d12h").is_err());
    }

    #[test]
    fn format_roundtrip() {
        assert_eq!(format(Duration::from_millis(0)), "0ms");
        assert_eq!(format(Duration::from_millis(1)), "1ms");
        assert_eq!(format(Duration::from_millis(100)), "100ms");
        assert_eq!(format(Duration::from_secs(1)), "1s");
        assert_eq!(format(Duration::from_secs(60)), "1m");
        assert_eq!(format(Duration::from_secs(3600)), "1h");
        assert_eq!(format(Duration::from_secs(86_400)), "1d");
        assert_eq!(format(Duration::from_secs(7 * 86_400)), "1w");
        // Non-aligned values fall back to smaller unit.
        assert_eq!(format(Duration::from_secs(90)), "90s");
        assert_eq!(format(Duration::from_millis(1500)), "1500ms");
    }

    #[test]
    fn whitespace_tolerated() {
        assert_eq!(parse("  100ms  ").unwrap(), Duration::from_millis(100));
    }
}
