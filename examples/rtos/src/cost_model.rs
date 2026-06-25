//! Measured cost-model selection for the RTOS demonstrator.
//!
//! The bundled `NOMINAL_COST_MODEL` in the keleusma runtime returns
//! per-opcode pipelined-cycle estimates suitable for relative
//! ordering of programs on a single platform. The values are not
//! measured for any specific host CPU and treat 1 as the unit of
//! data movement.
//!
//! For an RTOS demonstrator targeting the STM32N6570-DK, the
//! interesting question is: how many CPU cycles per task iteration
//! does each script consume on the actual deployment hardware? The
//! `keleusma-bench` workspace member answers that by measuring the
//! per-opcode cost on each target and emitting a Rust source
//! fragment that defines a `MEASURED_COST_MODEL` constant.
//!
//! This module exposes a single `MEASURED_COST_MODEL` constant that
//! resolves to the appropriate pre-generated fragment via
//! `cfg(target_arch = ...)`:
//!
//! - `aarch64-apple-darwin` (the development host) uses the M1 Max
//!   fragment calibrated at 3.228 GHz.
//! - `thumbv8m.main-none-eabihf` (the STM32N6570-DK) uses the
//!   Cortex-M55 fragment calibrated at 800 MHz with DWT_CYCCNT.
//! - Other targets fall back to `NOMINAL_COST_MODEL` so the crate
//!   continues to compile and run; the resulting cycle counts are
//!   relative weights, not calibrated CPU cycles.
//!
//! Hosts that ship measured fragments for additional architectures
//! place them under `keleusma-bench/measured_cost_models/` and add
//! the matching `cfg` arm below.

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../keleusma-bench/measured_cost_models/aarch64_apple_darwin.rs"
));

#[cfg(target_arch = "arm")]
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../keleusma-bench/measured_cost_models/thumbv8m_main_none_eabihf.rs"
));

#[cfg(not(any(all(target_arch = "aarch64", target_os = "macos"), target_arch = "arm",)))]
pub const MEASURED_COST_MODEL: keleusma::bytecode::CostModel =
    keleusma::bytecode::NOMINAL_COST_MODEL;

/// Shared inner logic. Given a Module, computes per-iteration WCET
/// under both `NOMINAL_COST_MODEL` and `MEASURED_COST_MODEL` and
/// returns the maximum across Stream chunks. Returns `None` if no
/// Stream-classified chunk is present.
fn report_module_wcet(module: &keleusma::bytecode::Module) -> Option<(u32, u32)> {
    let mut nominal_max: u32 = 0;
    let mut measured_max: u32 = 0;
    let mut found_stream = false;
    for chunk in module.chunks.iter() {
        if chunk.block_type != keleusma::bytecode::BlockType::Stream {
            continue;
        }
        found_stream = true;
        // Script-only per-iteration WCET; no native attestations (#50).
        if let Ok(n) = keleusma::verify::wcet_stream_iteration_with_cost_model(
            chunk,
            &keleusma::bytecode::NOMINAL_COST_MODEL,
            &[],
        ) && n > nominal_max
        {
            nominal_max = n;
        }
        if let Ok(m) = keleusma::verify::wcet_stream_iteration_with_cost_model(
            chunk,
            &MEASURED_COST_MODEL,
            &[],
        ) && m > measured_max
        {
            measured_max = m;
        }
    }
    if found_stream {
        Some((nominal_max, measured_max))
    } else {
        None
    }
}

/// Compute the per-iteration WCET for each Stream chunk in a
/// precompiled module, under both the bundled `NOMINAL_COST_MODEL`
/// and the target-specific `MEASURED_COST_MODEL`. Returns
/// `Some((nominal, measured))` (max across Stream chunks) or
/// `None` if the bytecode fails to decode or contains no Stream.
///
/// Intended for boot-time logging in the embedded demonstrator
/// binary where task scripts are precompiled to bytecode in
/// `OUT_DIR` and shipped via `include_bytes!`.
pub fn report_measured_wcet(bytecode: &[u8]) -> Option<(u32, u32)> {
    let module = keleusma::bytecode::Module::from_bytes(bytecode).ok()?;
    report_module_wcet(&module)
}

/// Compile a source string then compute per-iteration WCET as in
/// [`report_measured_wcet`]. The std-platform demonstrator uses
/// this path because the embedded `keleusma-compile` feature is
/// enabled and the bytecode artefacts are not committed at the
/// rtos crate boundary.
///
/// Gated on the `keleusma-compile` feature so it links only when
/// the lexer, parser, and compiler are in the runtime image.
#[cfg(feature = "keleusma-compile")]
pub fn report_measured_wcet_from_source(source: &str) -> Option<(u32, u32)> {
    let tokens = keleusma::lexer::tokenize(source).ok()?;
    let program = keleusma::parser::parse(&tokens).ok()?;
    let module = keleusma::compiler::compile(&program).ok()?;
    report_module_wcet(&module)
}
