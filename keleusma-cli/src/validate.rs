//! Offline bytecode validation. No subject instruction is executed.
use keleusma::bytecode::Module;
use keleusma::selfhost::{VerificationVerdict, admit_module_via_kel};
use std::process::ExitCode;

#[derive(Clone, Copy)]
enum Backend {
    Rust,
    SelfHosted,
}
fn reference(module: &Module, capacity: usize) -> Result<(), String> {
    keleusma::verify::verify(module).map_err(|e| format!("{e:?}"))?;
    keleusma::verify::verify_resource_bounds(module, capacity).map_err(|e| format!("{e:?}"))
}
pub(super) fn run(args: &[String]) -> ExitCode {
    let Some(path) = args.first() else {
        eprintln!(
            "usage: keleusma validate <bytecode> [--validator rust|self-hosted] [--arena-capacity bytes]"
        );
        return ExitCode::from(2);
    };
    let mut backend = Backend::Rust;
    let mut capacity = keleusma::vm::DEFAULT_ARENA_CAPACITY;
    let mut i = 1;
    while i < args.len() {
        let Some(value) = args.get(i + 1) else {
            eprintln!("error: {} requires a value", args[i]);
            return ExitCode::from(2);
        };
        match args[i].as_str() {
            "--validator" => match value.as_str() {
                "rust" => backend = Backend::Rust,
                "self-hosted" => backend = Backend::SelfHosted,
                _ => {
                    eprintln!("error: validator must be rust or self-hosted");
                    return ExitCode::from(2);
                }
            },
            "--arena-capacity" => match value.parse::<usize>() {
                Ok(n) => capacity = n,
                Err(_) => {
                    eprintln!("error: arena capacity must be a nonnegative byte count");
                    return ExitCode::from(2);
                }
            },
            _ => {
                eprintln!("error: unknown validation option {}", args[i]);
                return ExitCode::from(2);
            }
        }
        i += 2;
    }
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("error: reading {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let module = match Module::from_bytes(&bytes) {
        Ok(module) => module,
        Err(e) => {
            eprintln!("invalid bytecode: {e:?}");
            return ExitCode::FAILURE;
        }
    };
    match backend {
        Backend::Rust => match reference(&module, capacity) {
            Ok(()) => {
                println!("validation passed");
                ExitCode::SUCCESS
            }
            Err(detail) => {
                eprintln!("validation rejected: {detail}");
                ExitCode::FAILURE
            }
        },
        Backend::SelfHosted => {
            // Evaluate independently before obtaining the reference verdict.
            let verdict = match admit_module_via_kel(&module, capacity) {
                Ok(verdict) => verdict,
                Err(error) => {
                    eprintln!(
                        "self-hosted validation unavailable: {error}; retry with --validator rust"
                    );
                    return ExitCode::from(2);
                }
            };
            match (verdict, reference(&module, capacity)) {
                (VerificationVerdict::Accepted, Ok(())) => {
                    println!("validation passed");
                    ExitCode::SUCCESS
                }
                (VerificationVerdict::Rejected { stage, chunk }, Err(_)) => {
                    eprintln!("validation rejected by {stage} at chunk {chunk:?}");
                    ExitCode::FAILURE
                }
                (actual, expected) => {
                    eprintln!(
                        "validator disagreement: self-hosted {actual:?}, reference {expected:?}; retry with --validator rust"
                    );
                    ExitCode::from(2)
                }
            }
        }
    }
}
