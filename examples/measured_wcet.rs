//! Measured WCET worked example.
//!
//! Demonstrates how a host crate consults the keleusma-bench
//! measured cost model instead of the bundled
//! [`NOMINAL_COST_MODEL`]. The wiring is three steps:
//!
//! 1. Generate a fragment with `keleusma-bench` for the host, or use
//!    one of the pre-committed fragments under
//!    `keleusma-bench/measured_cost_models/`.
//! 2. `include!` the fragment as Rust source so the
//!    `MEASURED_COST_MODEL` constant lands in the host crate's
//!    namespace.
//! 3. Pass the measured model into the `_with_cost_model` variant of
//!    whichever WCET API the host calls
//!    ([`wcet_stream_iteration_with_cost_model`],
//!    [`verify_resource_bounds_with_cost_model`]).
//!
//! Run: `cargo run --release --example measured_wcet`
//!
//! The example compiles a small Stream-classified program, computes
//! the per-iteration WCET under both `NOMINAL_COST_MODEL` and
//! `MEASURED_COST_MODEL`, and prints the comparison.
//!
//! **Calibration caveat.** The fragment included below was measured
//! on the development host (Apple M1 Max, aarch64-apple-darwin). On
//! other hosts the numbers are dev-host estimates rather than
//! calibrated for the executing CPU. Regenerate the fragment per
//! host with `keleusma-bench --output <fragment>` (or
//! `keleusma-bench --from-log <captured.log>` for embedded targets)
//! to get accurate cycle counts. See
//! `keleusma-bench/measured_cost_models/README.md` for the workflow.

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/keleusma-bench/measured_cost_models/aarch64_apple_darwin.rs"
));

use keleusma::bytecode::{BlockType, NOMINAL_COST_MODEL};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify::wcet_stream_iteration_with_cost_model;

/// A small Stream-classified program with a productive divergent
/// `loop main` body. The body performs trivial arithmetic and
/// yields once per iteration, so the WCET per iteration is the
/// sum of the per-op costs in the loop body plus the yield
/// boundary. Local bindings in Keleusma are immutable; the loop
/// reads the host's resume value, computes a derived value, and
/// yields it.
const SOURCE: &str = r#"
loop main(seed: Word) -> Word {
    let doubled = seed + seed;
    let scaled = doubled * 3;
    let final = scaled - 1;
    yield final;
    0
}
"#;

fn main() {
    println!("=== Measured WCET worked example ===\n");
    println!("Source:");
    for line in SOURCE.trim_start().lines() {
        println!("  {}", line);
    }
    println!();

    let tokens = tokenize(SOURCE).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");

    println!(
        "Compiled {} chunk(s). Computing per-iteration WCET under both models.\n",
        module.chunks.len()
    );

    let mut found_stream = false;
    for chunk in module.chunks.iter() {
        if chunk.block_type != BlockType::Stream {
            continue;
        }
        found_stream = true;

        // The script-only per-iteration WCET (no native attestations, #50).
        let nominal_cycles = wcet_stream_iteration_with_cost_model(chunk, &NOMINAL_COST_MODEL, &[])
            .expect("nominal wcet");
        let measured_cycles =
            wcet_stream_iteration_with_cost_model(chunk, &MEASURED_COST_MODEL, &[])
                .expect("measured wcet");

        let ratio = measured_cycles as f64 / nominal_cycles as f64;
        println!(
            "Stream chunk `{name}`:\n  NOMINAL_COST_MODEL  : {nominal:>8} cycles per iteration\n  MEASURED_COST_MODEL : {measured:>8} cycles per iteration\n  Ratio (measured / nominal): {ratio:.2}x",
            name = chunk.name,
            nominal = nominal_cycles,
            measured = measured_cycles,
            ratio = ratio,
        );
    }

    if !found_stream {
        eprintln!("error: source contained no Stream-classified chunk");
        std::process::exit(1);
    }

    println!(
        "\nThe ratio reflects the relative scale between the nominal\nestimates (relative weights) and the calibrated CPU-cycle model\nthe bench produced for the dev host. The two are honest under\ndifferent meanings: nominal is for relative ordering on any host;\nmeasured is in CPU cycles for the host that ran the bench.\nWhen consuming measured numbers, document the host whose fragment\nyou included; otherwise the absolute scale is meaningless.\n"
    );
}
