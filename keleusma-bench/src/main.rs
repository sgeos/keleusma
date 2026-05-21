//! Command-line entry point for `keleusma-bench`.
//!
//! Runs the cost-model benchmark suite on the host CPU and writes a
//! generated Rust source fragment containing a `measured_op_cycles`
//! function and a `MEASURED_COST_MODEL` constant.
//!
//! Usage:
//!
//! ```sh
//! keleusma-bench --output measured_cost_model.rs
//! ```
//!
//! Without `--output`, the generated source is written to stdout.
//!
//! For cross-host measurement (Cortex-M via the N6-DK bench binary
//! at `examples/rtos/src/bin/bench_n6.rs`), the operator captures
//! the device's defmt RTT output into a log file and feeds it back
//! here:
//!
//! ```sh
//! # On the host with the N6 connected via probe-rs:
//! cargo run --release --manifest-path examples/rtos/Cargo.toml \
//!     --bin bench_n6 --target thumbv8m.main-none-eabihf \
//!     --no-default-features --features stm32n6570dk-platform \
//!     2>&1 | tee /tmp/bench_n6.log
//!
//! # Then convert the captured log to a fragment:
//! keleusma-bench --from-log /tmp/bench_n6.log \
//!     --output keleusma-bench/measured_cost_models/thumbv8m_main_none_eabihf.rs
//! ```
//!
//! The `--from-log` path matches the BENCH and BENCH_DONE markers
//! emitted by the embedded binary, reconstructs the f64 measurements
//! from their bit patterns, and emits the same fragment shape as
//! the host bench path.

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use keleusma_bench::counter::{assumed_cpu_hz, default_counter};
use keleusma_bench::{Measurement, emit_cost_model_source, measure_all};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut output_path: Option<PathBuf> = None;
    let mut from_log: Option<PathBuf> = None;
    let mut cpu_hz_override: Option<f64> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --output requires a path");
                    return ExitCode::FAILURE;
                }
                output_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--from-log" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --from-log requires a path");
                    return ExitCode::FAILURE;
                }
                from_log = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--cpu-hz" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --cpu-hz requires a value in Hz");
                    return ExitCode::FAILURE;
                }
                match args[i + 1].parse::<f64>() {
                    Ok(hz) if hz.is_finite() && hz > 0.0 => {
                        cpu_hz_override = Some(hz);
                    }
                    _ => {
                        eprintln!(
                            "error: --cpu-hz expects a positive finite number in Hz, got `{}`",
                            args[i + 1]
                        );
                        return ExitCode::FAILURE;
                    }
                }
                i += 2;
            }
            "--help" | "-h" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("error: unknown argument `{}`", other);
                print_help();
                return ExitCode::FAILURE;
            }
        }
    }

    // `--cpu-hz` overrides the `KELEUSMA_BENCH_CPU_HZ` environment
    // variable for the rest of this process. The override is set
    // before `default_counter()` constructs any counter so the
    // counter's `cpu_cycles_per_count` reads the new value.
    //
    // SAFETY: `set_var` is marked unsafe in the 2024 edition because
    // it is not thread-safe. The bench tool is single-threaded at
    // this point in main; no other thread reads env vars
    // concurrently. The change is intentional and is the supported
    // mechanism for per-invocation calibration on x86_64 and AArch64
    // hosts whose counter rate differs from CPU clock.
    if let Some(hz) = cpu_hz_override {
        unsafe {
            env::set_var("KELEUSMA_BENCH_CPU_HZ", format!("{}", hz));
        }
    }

    if let Some(log) = from_log {
        return run_from_log(log, output_path, cpu_hz_override);
    }

    let counter = default_counter();
    let counter_frequency_hz = counter.frequency_hz();
    let cpu_hz = assumed_cpu_hz();
    let cpu_hz_origin = if cpu_hz_override.is_some() {
        "--cpu-hz override"
    } else if env::var("KELEUSMA_BENCH_CPU_HZ").is_ok() {
        "KELEUSMA_BENCH_CPU_HZ env var"
    } else {
        "DEFAULT_ASSUMED_CPU_HZ"
    };
    eprintln!("counter: {}", counter.name());
    eprintln!(
        "counter tick frequency: {} Hz ({:.3} MHz)",
        counter_frequency_hz,
        counter_frequency_hz as f64 / 1_000_000.0
    );
    eprintln!(
        "assumed CPU clock:      {:.0} Hz ({:.3} GHz) (source: {})",
        cpu_hz,
        cpu_hz / 1_000_000_000.0,
        cpu_hz_origin
    );
    eprintln!(
        "scale (CPU cycles per counter tick): {:.3}",
        counter.cpu_cycles_per_count()
    );
    eprintln!(
        "running {} benchmark specs...",
        keleusma_bench::OPCODE_SPECS.len()
    );

    let measurements = measure_all(counter.as_ref());

    eprintln!();
    eprintln!("results: CPU cycles per pattern (raw f64), per op (u32, min 1)");
    for m in &measurements {
        eprintln!(
            "  {:<28} {:>16.4}   {:>10}",
            m.name, m.cycles_per_pattern, m.cycles_per_op
        );
    }
    eprintln!();

    let source =
        emit_cost_model_source(&measurements, counter.name(), counter_frequency_hz, cpu_hz);

    match output_path {
        Some(path) => match fs::File::create(&path) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(source.as_bytes()) {
                    eprintln!("error: writing {}: {}", path.display(), e);
                    return ExitCode::FAILURE;
                }
                eprintln!("wrote {}", path.display());
            }
            Err(e) => {
                eprintln!("error: creating {}: {}", path.display(), e);
                return ExitCode::FAILURE;
            }
        },
        None => {
            print!("{}", source);
        }
    }

    ExitCode::SUCCESS
}

fn print_help() {
    eprintln!("keleusma-bench: Keleusma cost-model calibration tool");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  keleusma-bench [--cpu-hz <Hz>] [--output <path>]");
    eprintln!("  keleusma-bench --from-log <path> [--cpu-hz <Hz>] [--output <path>]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -o, --output <path>   Write generated source to file (default: stdout)");
    eprintln!("      --cpu-hz <Hz>     Override the assumed CPU clock for this invocation.");
    eprintln!("                        Takes precedence over the KELEUSMA_BENCH_CPU_HZ");
    eprintln!("                        environment variable. In host-bench mode, scales");
    eprintln!("                        counter ticks to CPU cycles for counters that run");
    eprintln!("                        below CPU clock (AArch64 CNTVCT_EL0, Instant");
    eprintln!("                        fallback). In --from-log mode, overrides the");
    eprintln!("                        BENCH_DONE marker's cpu_hz field in the emitted");
    eprintln!("                        fragment header so operators on Cortex-M whose");
    eprintln!("                        actual clock differs from the hardcoded 800 MHz");
    eprintln!("                        can correct the documentation after capture.");
    eprintln!("      --from-log <path> Parse a captured N6 bench defmt log instead of");
    eprintln!("                        running the host bench. The log is a text file");
    eprintln!("                        containing the defmt RTT output of the embedded");
    eprintln!("                        bench binary; the parser extracts BENCH and");
    eprintln!("                        BENCH_DONE markers and reconstructs the f64");
    eprintln!("                        measurements from their u64 bit patterns.");
    eprintln!("  -h, --help            Show this help");
    eprintln!();
    eprintln!("Description:");
    eprintln!("  Without --from-log: measures pipelined-cycle cost per Keleusma");
    eprintln!("  opcode on the host CPU and emits a Rust source fragment defining");
    eprintln!("  `measured_op_cycles` and `MEASURED_COST_MODEL`. The host includes");
    eprintln!("  the fragment into its build to use the calibrated cost model in");
    eprintln!("  WCET analysis.");
    eprintln!();
    eprintln!("  With --from-log: parses a defmt log captured from the N6 bench");
    eprintln!("  binary at `examples/rtos/src/bin/bench_n6.rs` and emits a fragment");
    eprintln!("  calibrated for the embedded target.");
    eprintln!();
    eprintln!("  See the crate README for methodology and limitations.");
}

/// Parse a defmt log captured from the N6 bench binary and emit a
/// fragment. The log is expected to contain one `BENCH idx=I/N
/// name=NAME bits=BITS per_op=COST` line per spec, followed by a
/// single `BENCH_DONE cpu_hz=HZ counter_hz=HZ` line.
fn run_from_log(
    log_path: PathBuf,
    output_path: Option<PathBuf>,
    cpu_hz_override: Option<f64>,
) -> ExitCode {
    let contents = match fs::read_to_string(&log_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: reading {}: {}", log_path.display(), e);
            return ExitCode::FAILURE;
        }
    };

    let mut measurements: Vec<Measurement> = Vec::new();
    let mut cpu_hz: Option<u64> = None;
    let mut counter_hz: Option<u64> = None;
    let mut counter_name: String = String::from("Cortex-M DWT_CYCCNT");

    for line in contents.lines() {
        // Look for a token-sequence match anywhere on the line. Lines
        // typically look like:
        //   0.000123 INFO  BENCH idx=1/17 name=Const bits=4639... per_op=58
        // so we scan for the BENCH marker and parse the trailing
        // key=value pairs.
        if let Some(payload) = line.split("BENCH_DONE").nth(1) {
            for tok in payload.split_whitespace() {
                if let Some(v) = tok.strip_prefix("cpu_hz=") {
                    cpu_hz = v.parse().ok();
                } else if let Some(v) = tok.strip_prefix("counter_hz=") {
                    counter_hz = v.parse().ok();
                }
            }
            continue;
        }
        if let Some(payload) = line.split("BENCH ").nth(1) {
            // Tolerate the line containing further tokens past the
            // payload. Each token is `key=value`.
            let mut name: Option<&str> = None;
            let mut bits: Option<u64> = None;
            let mut per_op: Option<u32> = None;
            for tok in payload.split_whitespace() {
                if let Some(v) = tok.strip_prefix("name=") {
                    name = Some(v);
                } else if let Some(v) = tok.strip_prefix("bits=") {
                    bits = v.parse().ok();
                } else if let Some(v) = tok.strip_prefix("per_op=") {
                    per_op = v.parse().ok();
                }
            }
            match (name, bits, per_op) {
                (Some(n), Some(b), Some(o)) => {
                    let cycles_per_pattern = f64::from_bits(b);
                    measurements.push(Measurement {
                        // Lookup the static spec name. The parser
                        // stores the captured String inline. Cost
                        // emit only matches names that appear in
                        // OPCODE_SPECS so we leak the captured
                        // string to obtain a `&'static str`.
                        name: Box::leak(n.to_string().into_boxed_str()),
                        cycles_per_pattern,
                        ops_per_pattern: 0,
                        cycles_per_op: o,
                    });
                }
                _ => {
                    eprintln!("warning: malformed BENCH line: {}", line);
                }
            }
        }
        if let Some(payload) = line.split("Counter:").nth(1) {
            counter_name = payload.trim().to_string();
        }
    }

    if measurements.is_empty() {
        eprintln!("error: no BENCH lines found in {}", log_path.display());
        return ExitCode::FAILURE;
    }
    let counter_hz = match counter_hz {
        Some(hz) => hz,
        None => {
            eprintln!("error: BENCH_DONE marker not found or missing counter_hz");
            return ExitCode::FAILURE;
        }
    };
    // `--cpu-hz` on the CLI overrides the BENCH_DONE-reported value.
    // The override is the operator's stated assumption about the
    // device's actual CPU clock; the BENCH_DONE value is what the
    // embedded binary hardcoded. Either may be correct; the
    // override wins because the operator has fresher information.
    let (cpu_hz_f, cpu_hz_origin) = match (cpu_hz_override, cpu_hz) {
        (Some(hz), _) => (hz, "--cpu-hz override"),
        (None, Some(hz)) => (hz as f64, "BENCH_DONE marker"),
        (None, None) => {
            eprintln!(
                "error: BENCH_DONE marker not found or missing cpu_hz; \
                 pass --cpu-hz <Hz> to supply the value explicitly"
            );
            return ExitCode::FAILURE;
        }
    };

    eprintln!(
        "parsed {} measurements from {}",
        measurements.len(),
        log_path.display()
    );
    eprintln!("counter: {} ({} Hz)", counter_name, counter_hz);
    eprintln!("cpu_hz: {} Hz (source: {})", cpu_hz_f, cpu_hz_origin);

    let source = emit_cost_model_source(&measurements, &counter_name, counter_hz, cpu_hz_f);

    match output_path {
        Some(path) => match fs::File::create(&path) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(source.as_bytes()) {
                    eprintln!("error: writing {}: {}", path.display(), e);
                    return ExitCode::FAILURE;
                }
                eprintln!("wrote {}", path.display());
            }
            Err(e) => {
                eprintln!("error: creating {}: {}", path.display(), e);
                return ExitCode::FAILURE;
            }
        },
        None => {
            print!("{}", source);
        }
    }

    ExitCode::SUCCESS
}
