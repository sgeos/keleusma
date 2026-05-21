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

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use keleusma_bench::counter::{assumed_cpu_hz, default_counter};
use keleusma_bench::{emit_cost_model_source, measure_all};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut output_path: Option<PathBuf> = None;
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
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -o, --output <path>   Write generated source to file (default: stdout)");
    eprintln!("  -h, --help            Show this help");
    eprintln!();
    eprintln!("Description:");
    eprintln!("  Measures pipelined-cycle cost per Keleusma opcode on the host CPU");
    eprintln!("  and emits a Rust source fragment defining `measured_op_cycles` and");
    eprintln!("  `MEASURED_COST_MODEL`. The host includes the fragment into its build");
    eprintln!("  to use the calibrated cost model in WCET analysis.");
    eprintln!();
    eprintln!("  See the crate README for methodology and limitations.");
}
