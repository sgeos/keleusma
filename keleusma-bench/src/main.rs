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

    if let Some(log) = from_log {
        return run_from_log(log, output_path);
    }

    let counter = default_counter();
    let counter_frequency_hz = counter.frequency_hz();
    let cpu_hz = assumed_cpu_hz();
    eprintln!("counter: {}", counter.name());
    eprintln!(
        "counter tick frequency: {} Hz ({:.3} MHz)",
        counter_frequency_hz,
        counter_frequency_hz as f64 / 1_000_000.0
    );
    eprintln!(
        "assumed CPU clock:      {:.0} Hz ({:.3} GHz) (override with KELEUSMA_BENCH_CPU_HZ)",
        cpu_hz,
        cpu_hz / 1_000_000_000.0
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

    let source = emit_cost_model_source(
        &measurements,
        counter.name(),
        counter_frequency_hz,
        cpu_hz,
    );

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
    eprintln!("  keleusma-bench [--output <path>]");
    eprintln!("  keleusma-bench --from-log <path> [--output <path>]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -o, --output <path>   Write generated source to file (default: stdout)");
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
fn run_from_log(log_path: PathBuf, output_path: Option<PathBuf>) -> ExitCode {
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
    let cpu_hz_f = match cpu_hz {
        Some(hz) => hz as f64,
        None => {
            eprintln!("error: BENCH_DONE marker not found or missing cpu_hz");
            return ExitCode::FAILURE;
        }
    };

    eprintln!(
        "parsed {} measurements from {}",
        measurements.len(),
        log_path.display()
    );
    eprintln!(
        "counter: {} ({} Hz)",
        counter_name, counter_hz
    );
    eprintln!("cpu_hz: {} Hz", cpu_hz_f);

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
