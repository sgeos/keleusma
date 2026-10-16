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
            "usage: keleusma validate <bytecode> [--validator rust|self-hosted] [--arena-capacity bytes] [--require-trap-free]"
        );
        return ExitCode::from(2);
    };
    let mut backend = Backend::Rust;
    let mut require_trap_free = false;
    let mut capacity = keleusma::vm::DEFAULT_ARENA_CAPACITY;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--require-trap-free" {
            require_trap_free = true;
            i += 1;
            continue;
        }
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
            Ok(()) => accepted(&module, backend, require_trap_free),
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
                    accepted(&module, backend, require_trap_free)
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

fn accepted(module: &Module, backend: Backend, require: bool) -> ExitCode {
    if !require {
        println!("validation passed");
        return ExitCode::SUCCESS;
    }
    use keleusma::trap_analysis::{TrapAnalysisError, TrapVerdict, classify_traps};
    let verdict = match backend {
        Backend::Rust => classify_traps(module),
        Backend::SelfHosted => {
            let actual = keleusma::selfhost::classify_traps_via_kel(module);
            if let Err(TrapAnalysisError::Unavailable { detail }) = &actual {
                eprintln!("self-hosted trap analysis unavailable: {detail}");
                return ExitCode::from(2);
            }
            let expected = classify_traps(module);
            let both_invalid = matches!(
                (&actual, &expected),
                (
                    Err(TrapAnalysisError::InvalidModule { .. }),
                    Err(TrapAnalysisError::InvalidModule { .. })
                )
            );
            if actual != expected && !both_invalid {
                eprintln!(
                    "trap classifier disagreement: self-hosted {actual:?}, reference {expected:?}"
                );
                return ExitCode::from(2);
            }
            actual
        }
    };
    match verdict {
        Ok(TrapVerdict::TrapFree) => {
            println!("validation passed; trap-free");
            ExitCode::SUCCESS
        }
        Ok(TrapVerdict::MayTrap { first }) => {
            eprintln!("trap policy rejected: MayTrap at {first:?}");
            ExitCode::FAILURE
        }
        Err(TrapAnalysisError::InvalidModule { detail }) => {
            eprintln!("validation rejected: {detail}");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("trap analysis unavailable: {error}");
            ExitCode::from(2)
        }
    }
}
