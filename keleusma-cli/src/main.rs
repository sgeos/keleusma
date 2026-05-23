#![deny(missing_docs)]
//! Standalone command-line frontend for Keleusma.
//!
//! Provides three subcommands modeled after Rhai's CLI tooling:
//!
//! - `run <file>` parses, compiles, verifies, and executes a Keleusma
//!   script. Pre-registers utility and math natives.
//! - `compile <file> [-o <output>]` produces a serialized bytecode
//!   file that hosts can load through `Vm::load_bytes`.
//! - `repl` starts an interactive prompt where expressions and
//!   declarations accumulate into a session prefix.
//!
//! As a shorthand, any first argument ending in `.kel` is treated as
//! a `run` invocation, so `keleusma hello.kel` runs the script.
//!
//! The CLI gates bytecode execution through optional signing and
//! encryption policies. Discovery of enrolled signing keys from a
//! platform-conventional directory or the `KELEUSMA_TRUSTED_KEYS_DIR`
//! environment variable activates strict signing mode. The strict
//! mode rejects source files, unsigned bytecode, and bytecode signed
//! by non-enrolled keys. See `strict_mode` for the policy mechanics.

mod strict_mode;

use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::ExitCode;

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::stddsl;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

use strict_mode::{PolicyContext, X25519_PRIVATE_KEY_LEN, build_policy_context};

const REPL_BANNER: &str = "Keleusma REPL. Type :help for commands, :quit to exit.";

const REPL_RETURN_TYPES: &[&str] = &["Word", "Float", "bool", "Text", "()"];

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let subcommand = match args.get(1) {
        Some(s) => s.as_str(),
        None => {
            print_help();
            return ExitCode::SUCCESS;
        }
    };

    match subcommand {
        "run" => run_subcommand(&args[2..]),
        "compile" => compile_subcommand(&args[2..]),
        "keygen" => keygen_subcommand(&args[2..]),
        "repl" => repl_subcommand(&args[2..]),
        "--help" | "-h" | "help" => {
            print_help();
            ExitCode::SUCCESS
        }
        "--version" | "-V" | "version" => {
            println!("keleusma {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        other => {
            // Treat any remaining argument as a script path. This
            // covers both `keleusma file.kel` (extension shorthand)
            // and `keleusma /path/to/extensionless-shebang-script`
            // when the kernel invokes us through a `#!/usr/bin/env
            // keleusma` line. If the file is missing, the error
            // surfaces in `run_file`.
            if std::path::Path::new(other).is_file() {
                let ctx = match build_policy_context() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("error: {}", e);
                        return ExitCode::FAILURE;
                    }
                };
                let verifying = ctx.enrolled_keys.clone();
                let decrypting = ctx.decryption_keys.clone();
                run_file(other, &verifying, &decrypting, &ctx)
            } else {
                eprintln!("error: unknown subcommand or missing file `{}`", other);
                print_help();
                ExitCode::FAILURE
            }
        }
    }
}

fn print_help() {
    println!("keleusma: command-line frontend for the Keleusma scripting language");
    println!();
    println!("Usage:");
    println!("  keleusma <subcommand> [options]");
    println!("  keleusma <file>.kel               (shorthand for `run`)");
    println!();
    println!("Subcommands:");
    println!("  run <file> [--verifying-key <keyfile> ...]");
    println!("                                    Compile and execute a script.");
    println!("                                    Pass --verifying-key (repeatable) to verify");
    println!("                                    signed compiled bytecode against the");
    println!("                                    supplied 32-byte Ed25519 public-key files.");
    println!("  compile <file> [-o <output>] [--signing-key <keyfile>]");
    println!("                                    Compile to bytecode. With --signing-key,");
    println!("                                    sign the output with the supplied 32-byte");
    println!("                                    Ed25519 seed file. The source script must");
    println!("                                    declare the entry function with the");
    println!("                                    `signed` modifier; otherwise the resulting");
    println!("                                    bytecode is unsigned and the toolchain");
    println!("                                    refuses the signing key argument silently.");
    println!("  keygen --seed <out> --public <out>");
    println!("                                    Generate a fresh Ed25519 keypair from the");
    println!("                                    OS RNG. Writes the 32-byte signing seed to");
    println!("                                    one file and the 32-byte verifying key to");
    println!("                                    another. Treat the seed as a private");
    println!("                                    secret; the verifying key may be");
    println!("                                    distributed to hosts that load signed");
    println!("                                    bytecode.");
    println!("  repl                              Start interactive REPL");
    println!("  help, --help, -h                  Show this help");
    println!("  version, --version, -V            Show version");
    println!();
    println!("Examples:");
    println!("  keleusma run hello.kel");
    println!("  keleusma hello.kel");
    println!("  keleusma compile hello.kel -o hello.kel.bin");
    println!("  keleusma keygen --seed key.seed --public key.pub");
    println!("  keleusma compile hello.kel --signing-key key.seed -o hello.kel.bin");
    println!("  keleusma run hello.kel.bin --verifying-key key.pub");
    println!("  keleusma repl");
    println!();
    println!("Strict-mode policies (signing and encryption):");
    println!("  Signing: place 32-byte Ed25519 public keys as `*.pub`");
    println!("  files in /etc/keleusma/trusted_keys (Unix) or");
    println!("  %PROGRAMDATA%\\keleusma\\trusted_keys (Windows), or in");
    println!("  the directory named by KELEUSMA_TRUSTED_KEYS_DIR. The");
    println!("  CLI refuses source files, unsigned bytecode, and");
    println!("  bytecode signed by keys not in the trust store. The");
    println!("  --verifying-key argument is rejected. Set");
    println!("  KELEUSMA_REQUIRE_SIGNED=1 to force strict signing mode");
    println!("  even with an empty trust store.");
    println!();
    println!("  Encryption: place 32-byte X25519 private keys as");
    println!("  `*.seed` files in /etc/keleusma/decryption_keys (Unix)");
    println!("  or the equivalent Windows path, or in the directory");
    println!("  named by KELEUSMA_DECRYPTION_KEYS_DIR. The CLI refuses");
    println!("  unencrypted bytecode and bytecode encrypted to a key");
    println!("  not in the decryption-key store. The --decryption-key");
    println!("  argument is rejected. Set KELEUSMA_REQUIRE_ENCRYPTED=1");
    println!("  to force strict encryption mode.");
    println!();
    println!("  The two policies are independent: neither, signing");
    println!("  only, encryption only, or both may be active.");
    println!();
    println!("Examples (encryption):");
    println!("  keleusma keygen --kind encryption --seed dest.seed --public dest.pub");
    println!("  keleusma compile script.kel --signing-key sign.seed \\");
    println!("           --encryption-key dest.pub -o script.kel.bin");
    println!("  keleusma run script.kel.bin --verifying-key sign.pub \\");
    println!("           --decryption-key dest.seed");
}

fn run_subcommand(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("error: `run` requires a script path");
        return ExitCode::FAILURE;
    }
    let path = &args[0];

    // Build the policy context at the start of every run invocation.
    // Discovery is fail-closed; a malformed key file in either the
    // signing trust store or the decryption-key store causes the
    // CLI to refuse to start.
    let ctx = match build_policy_context() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::FAILURE;
        }
    };

    let mut command_line_keys: Vec<ed25519_dalek::VerifyingKey> = Vec::new();
    let mut command_line_decryption_keys: Vec<[u8; X25519_PRIVATE_KEY_LEN]> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--verifying-key" => {
                if i + 1 >= args.len() {
                    eprintln!(
                        "error: --verifying-key requires a path to a 32-byte public-key file"
                    );
                    return ExitCode::FAILURE;
                }
                match read_verifying_key(&args[i + 1]) {
                    Ok(k) => command_line_keys.push(k),
                    Err(e) => {
                        eprintln!("error: {}", e);
                        return ExitCode::FAILURE;
                    }
                }
                i += 2;
            }
            "--decryption-key" => {
                if i + 1 >= args.len() {
                    eprintln!(
                        "error: --decryption-key requires a path to a 32-byte X25519 seed file"
                    );
                    return ExitCode::FAILURE;
                }
                match read_x25519_private_key(&args[i + 1]) {
                    Ok(k) => command_line_decryption_keys.push(k),
                    Err(e) => {
                        eprintln!("error: {}", e);
                        return ExitCode::FAILURE;
                    }
                }
                i += 2;
            }
            other => {
                eprintln!("error: unknown option `{}`", other);
                return ExitCode::FAILURE;
            }
        }
    }

    // In strict signing mode the trust store is system-managed.
    // The --verifying-key flag is rejected so an unprivileged
    // operator cannot relax the policy at the command line.
    if ctx.strict_signing && !command_line_keys.is_empty() {
        eprintln!(
            "error: strict mode: --verifying-key is rejected; the trust list is system-managed through KELEUSMA_TRUSTED_KEYS_DIR or the platform-conventional directory"
        );
        return ExitCode::FAILURE;
    }

    // In strict encryption mode the decryption-key store is
    // system-managed. The --decryption-key flag is rejected for the
    // same reason as --verifying-key.
    if ctx.strict_encryption && !command_line_decryption_keys.is_empty() {
        eprintln!(
            "error: strict mode: --decryption-key is rejected; the decryption-key store is system-managed through KELEUSMA_DECRYPTION_KEYS_DIR or the platform-conventional directory"
        );
        return ExitCode::FAILURE;
    }

    // Merge the enrolled trust list (used in strict mode) with the
    // command-line keys (used in permissive mode). Only one of the
    // two is non-empty after the rejection check above.
    let mut verifying_keys = ctx.enrolled_keys.clone();
    verifying_keys.extend(command_line_keys);
    let mut decryption_keys = ctx.decryption_keys.clone();
    decryption_keys.extend(command_line_decryption_keys);

    run_file(path, &verifying_keys, &decryption_keys, &ctx)
}

fn run_file(
    path: &str,
    verifying_keys: &[ed25519_dalek::VerifyingKey],
    decryption_keys: &[[u8; X25519_PRIVATE_KEY_LEN]],
    policy: &PolicyContext,
) -> ExitCode {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: reading {}: {}", path, e);
            return ExitCode::FAILURE;
        }
    };
    // Detect compiled bytecode by magic. The bytecode loader strips
    // a leading shebang line, so check both at offset 0 and after a
    // `#!...\n` envelope.
    let result = if looks_like_bytecode(&bytes) {
        execute_bytecode(&bytes, verifying_keys, decryption_keys, policy)
    } else if policy.strict_signing || policy.strict_encryption {
        eprintln!(
            "error: strict mode: source execution disabled; compile{} the source before running",
            if policy.strict_encryption {
                ", sign, and encrypt"
            } else {
                " and sign"
            }
        );
        return ExitCode::FAILURE;
    } else if !verifying_keys.is_empty() || !decryption_keys.is_empty() {
        eprintln!(
            "error: --verifying-key or --decryption-key supplied but {} is source, not bytecode",
            path
        );
        return ExitCode::FAILURE;
    } else {
        let source = match core::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(_) => {
                eprintln!(
                    "error: {} is neither valid UTF-8 source nor recognised bytecode",
                    path
                );
                return ExitCode::FAILURE;
            }
        };
        execute_source(source)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn looks_like_bytecode(bytes: &[u8]) -> bool {
    let after_shebang = if bytes.starts_with(b"#!") {
        match bytes.iter().position(|&b| b == b'\n') {
            Some(nl) => &bytes[nl + 1..],
            None => return false,
        }
    } else {
        bytes
    };
    after_shebang.starts_with(b"KELE")
}

fn compile_subcommand(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("error: `compile` requires a script path");
        return ExitCode::FAILURE;
    }
    let input = &args[0];
    let mut output: Option<String> = None;
    let mut signing_key_path: Option<String> = None;
    let mut encryption_key_path: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --output requires a path");
                    return ExitCode::FAILURE;
                }
                output = Some(args[i + 1].clone());
                i += 2;
            }
            "--signing-key" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --signing-key requires a path to a 32-byte seed file");
                    return ExitCode::FAILURE;
                }
                signing_key_path = Some(args[i + 1].clone());
                i += 2;
            }
            "--encryption-key" => {
                if i + 1 >= args.len() {
                    eprintln!(
                        "error: --encryption-key requires a path to a 32-byte X25519 public-key file"
                    );
                    return ExitCode::FAILURE;
                }
                encryption_key_path = Some(args[i + 1].clone());
                i += 2;
            }
            other => {
                eprintln!("error: unknown option `{}`", other);
                return ExitCode::FAILURE;
            }
        }
    }
    let output_path = output.unwrap_or_else(|| default_output_path(input));

    // Encryption requires signing because the wire format ties the
    // two together. The signature covers the encrypted body so an
    // adversary cannot strip the encryption layer.
    if encryption_key_path.is_some() && signing_key_path.is_none() {
        eprintln!(
            "error: --encryption-key requires --signing-key; encrypted artefacts must be signed"
        );
        return ExitCode::FAILURE;
    }

    // Compile-time strict-mode warning. If the local host runs
    // strict signing or strict encryption mode, warn the operator
    // when the compile would produce an artefact that the local
    // host would not accept. The warning does not fail the compile;
    // operators may legitimately produce artefacts for other hosts.
    if let Ok(policy) = build_policy_context() {
        if policy.strict_signing && signing_key_path.is_none() {
            eprintln!(
                "warning: local host runs strict signing mode; the produced artefact will be unsigned and will not run on this host"
            );
        }
        if policy.strict_signing
            && let Some(ref sign_path) = signing_key_path
            && let Ok(signing_key) = read_signing_key(sign_path)
        {
            let verifying = signing_key.verifying_key();
            if !policy.enrolled_keys.contains(&verifying) {
                eprintln!(
                    "warning: the signing key's verifying counterpart is not in this host's trust list; the produced artefact will not run on this host"
                );
            }
        }
        if policy.strict_encryption && encryption_key_path.is_none() {
            eprintln!(
                "warning: local host runs strict encryption mode; the produced artefact will be unencrypted and will not run on this host"
            );
        }
    }

    let source = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: reading {}: {}", input, e);
            return ExitCode::FAILURE;
        }
    };
    let module = match compile_source(&source) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::FAILURE;
        }
    };
    let bytes = match (signing_key_path, encryption_key_path) {
        (Some(sign_path), Some(enc_path)) => {
            // Signed-and-encrypted path.
            let signing_key = match read_signing_key(&sign_path) {
                Ok(k) => k,
                Err(e) => {
                    eprintln!("error: {}", e);
                    return ExitCode::FAILURE;
                }
            };
            let recipient_pk = match read_x25519_public_key(&enc_path) {
                Ok(k) => k,
                Err(e) => {
                    eprintln!("error: {}", e);
                    return ExitCode::FAILURE;
                }
            };
            // Generate a fresh ephemeral X25519 seed from the OS RNG
            // for this module. The ephemeral private key is consumed
            // by the encryption operation and discarded; only the
            // ephemeral public key persists in the artefact.
            let mut ephemeral_seed = [0u8; X25519_PRIVATE_KEY_LEN];
            use rand_core::RngCore;
            rand_core::OsRng.fill_bytes(&mut ephemeral_seed);
            match keleusma::wire_format::module_to_encrypted_signed_wire_bytes(
                &module,
                &signing_key,
                &recipient_pk,
                &ephemeral_seed,
            ) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: encrypting and signing bytecode: {:?}", e);
                    return ExitCode::FAILURE;
                }
            }
        }
        (Some(sign_path), None) => {
            // Signed-only path (existing behavior).
            let signing_key = match read_signing_key(&sign_path) {
                Ok(k) => k,
                Err(e) => {
                    eprintln!("error: {}", e);
                    return ExitCode::FAILURE;
                }
            };
            match keleusma::wire_format::module_to_signed_wire_bytes(&module, &signing_key) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: signing bytecode: {:?}", e);
                    return ExitCode::FAILURE;
                }
            }
        }
        (None, _) => {
            // Unsigned, unencrypted (existing default).
            match module.to_bytes() {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: serializing bytecode: {:?}", e);
                    return ExitCode::FAILURE;
                }
            }
        }
    };
    if let Err(e) = fs::write(&output_path, &bytes) {
        eprintln!("error: writing {}: {}", output_path, e);
        return ExitCode::FAILURE;
    }
    eprintln!("wrote {} ({} bytes)", output_path, bytes.len());
    ExitCode::SUCCESS
}

/// Generate a fresh keypair for either signing (Ed25519) or
/// encryption (X25519). The two key kinds are not interchangeable:
/// signing keys authenticate code provenance; encryption keys
/// receive encrypted code. Operators typically generate both for
/// any host that participates in the encrypted delivery flow.
fn keygen_subcommand(args: &[String]) -> ExitCode {
    let mut seed_path: Option<String> = None;
    let mut pub_path: Option<String> = None;
    let mut kind = KeyKind::Signing;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --seed requires a path");
                    return ExitCode::FAILURE;
                }
                seed_path = Some(args[i + 1].clone());
                i += 2;
            }
            "--public" | "--public-key" | "--verifying-key" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --public requires a path");
                    return ExitCode::FAILURE;
                }
                pub_path = Some(args[i + 1].clone());
                i += 2;
            }
            "--kind" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --kind requires either 'signing' or 'encryption'");
                    return ExitCode::FAILURE;
                }
                kind = match args[i + 1].as_str() {
                    "signing" => KeyKind::Signing,
                    "encryption" => KeyKind::Encryption,
                    other => {
                        eprintln!(
                            "error: --kind must be 'signing' or 'encryption'; got '{}'",
                            other
                        );
                        return ExitCode::FAILURE;
                    }
                };
                i += 2;
            }
            other => {
                eprintln!("error: unknown option `{}`", other);
                return ExitCode::FAILURE;
            }
        }
    }
    let seed_path = match seed_path {
        Some(p) => p,
        None => {
            eprintln!("error: keygen requires --seed <path>");
            return ExitCode::FAILURE;
        }
    };
    let pub_path = match pub_path {
        Some(p) => p,
        None => {
            eprintln!("error: keygen requires --public <path>");
            return ExitCode::FAILURE;
        }
    };
    if Path::new(&seed_path).exists() {
        eprintln!(
            "error: refusing to overwrite existing seed file {}; remove or rename first",
            seed_path
        );
        return ExitCode::FAILURE;
    }
    if Path::new(&pub_path).exists() {
        eprintln!(
            "error: refusing to overwrite existing public-key file {}; remove or rename first",
            pub_path
        );
        return ExitCode::FAILURE;
    }
    let (seed_bytes, public_bytes, kind_label) = match kind {
        KeyKind::Signing => {
            let signing_key = ed25519_dalek::SigningKey::generate(&mut rand_core::OsRng);
            let verifying_key = signing_key.verifying_key();
            (signing_key.to_bytes(), verifying_key.to_bytes(), "Ed25519")
        }
        KeyKind::Encryption => {
            // X25519 private keys can be any 32 bytes; the StaticSecret
            // constructor clamps internally per the X25519 specification.
            // We generate raw bytes from the OS RNG.
            use rand_core::RngCore;
            let mut seed = [0u8; X25519_PRIVATE_KEY_LEN];
            rand_core::OsRng.fill_bytes(&mut seed);
            let public = keleusma::encryption::public_key_from_private(&seed);
            (seed, public, "X25519")
        }
    };
    if let Err(e) = fs::write(&seed_path, seed_bytes) {
        eprintln!("error: writing seed file {}: {}", seed_path, e);
        return ExitCode::FAILURE;
    }
    if let Err(e) = fs::write(&pub_path, public_bytes) {
        eprintln!("error: writing public-key file {}: {}", pub_path, e);
        return ExitCode::FAILURE;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(&seed_path, std::fs::Permissions::from_mode(0o600)) {
            eprintln!(
                "warning: could not tighten permissions on {}: {}",
                seed_path, e
            );
        }
    }
    eprintln!(
        "wrote {} seed to {} (32 bytes; keep secret)",
        kind_label, seed_path
    );
    let public_role = match kind {
        KeyKind::Signing => "distribute to verifiers",
        KeyKind::Encryption => "distribute to compilers producing artefacts for this host",
    };
    eprintln!(
        "wrote {} public key to {} (32 bytes; {})",
        kind_label, pub_path, public_role
    );
    ExitCode::SUCCESS
}

/// Selects the cryptographic primitive family for the `keygen`
/// subcommand. Signing produces Ed25519 keypairs; encryption
/// produces X25519 keypairs.
#[derive(Debug, Clone, Copy)]
enum KeyKind {
    Signing,
    Encryption,
}

/// Read a raw 32-byte Ed25519 seed from `path` and construct a
/// `SigningKey`. Returns an error message string suitable for
/// CLI output if the file is missing, the wrong size, or
/// unreadable.
fn read_signing_key(path: &str) -> Result<ed25519_dalek::SigningKey, String> {
    let bytes = fs::read(path).map_err(|e| format!("reading signing key {}: {}", path, e))?;
    if bytes.len() != 32 {
        return Err(format!(
            "signing key file {} must be exactly 32 bytes (raw Ed25519 seed); got {} bytes",
            path,
            bytes.len()
        ));
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&bytes);
    Ok(ed25519_dalek::SigningKey::from_bytes(&seed))
}

/// Read a raw 32-byte X25519 private key (seed) from `path`.
/// Used by the run subcommand's `--decryption-key` flag and by
/// the strict-mode decryption-key store discovery.
fn read_x25519_private_key(path: &str) -> Result<[u8; X25519_PRIVATE_KEY_LEN], String> {
    let bytes = fs::read(path).map_err(|e| format!("reading decryption key {}: {}", path, e))?;
    if bytes.len() != X25519_PRIVATE_KEY_LEN {
        return Err(format!(
            "decryption key file {} must be exactly {} bytes (raw X25519 seed); got {} bytes",
            path,
            X25519_PRIVATE_KEY_LEN,
            bytes.len()
        ));
    }
    let mut seed = [0u8; X25519_PRIVATE_KEY_LEN];
    seed.copy_from_slice(&bytes);
    Ok(seed)
}

/// Read a raw 32-byte X25519 public key from `path`. Used by the
/// compile subcommand's `--encryption-key` flag where the operator
/// supplies the destination workstation's public key.
fn read_x25519_public_key(path: &str) -> Result<[u8; X25519_PRIVATE_KEY_LEN], String> {
    let bytes = fs::read(path).map_err(|e| format!("reading encryption key {}: {}", path, e))?;
    if bytes.len() != X25519_PRIVATE_KEY_LEN {
        return Err(format!(
            "encryption key file {} must be exactly {} bytes (raw X25519 public key); got {} bytes",
            path,
            X25519_PRIVATE_KEY_LEN,
            bytes.len()
        ));
    }
    let mut key = [0u8; X25519_PRIVATE_KEY_LEN];
    key.copy_from_slice(&bytes);
    Ok(key)
}

/// Read a raw 32-byte Ed25519 public key from `path` and
/// construct a `VerifyingKey`.
fn read_verifying_key(path: &str) -> Result<ed25519_dalek::VerifyingKey, String> {
    let bytes = fs::read(path).map_err(|e| format!("reading verifying key {}: {}", path, e))?;
    if bytes.len() != 32 {
        return Err(format!(
            "verifying key file {} must be exactly 32 bytes (raw Ed25519 public key); got {} bytes",
            path,
            bytes.len()
        ));
    }
    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&bytes);
    ed25519_dalek::VerifyingKey::from_bytes(&key_bytes).map_err(|e| {
        format!(
            "verifying key {} is not a valid Ed25519 public key: {}",
            path, e
        )
    })
}

fn default_output_path(input: &str) -> String {
    let path = Path::new(input);
    if path.extension().and_then(|s| s.to_str()) == Some("kel") {
        format!("{}.bin", input)
    } else {
        format!("{}.kel.bin", input)
    }
}

fn repl_subcommand(_args: &[String]) -> ExitCode {
    println!("{}", REPL_BANNER);
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut prefix = String::new();
    let mut input = String::new();

    loop {
        {
            let mut out = stdout.lock();
            let _ = out.write_all(b"> ");
            let _ = out.flush();
        }
        input.clear();
        let bytes_read = match stdin.lock().read_line(&mut input) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("error: reading input: {}", e);
                return ExitCode::FAILURE;
            }
        };
        if bytes_read == 0 {
            // EOF (Ctrl-D).
            println!();
            return ExitCode::SUCCESS;
        }
        let line = input.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(stripped) = line.strip_prefix(':') {
            match stripped {
                "quit" | "q" | "exit" => return ExitCode::SUCCESS,
                "help" | "h" => print_repl_help(),
                "reset" => {
                    prefix.clear();
                    println!("session prefix cleared");
                }
                "show" => {
                    if prefix.is_empty() {
                        println!("(empty session prefix)");
                    } else {
                        println!("{}", prefix);
                    }
                }
                other => {
                    eprintln!("error: unknown REPL command `:{}`", other);
                }
            }
            continue;
        }
        evaluate_repl_input(&mut prefix, line);
    }
}

fn print_repl_help() {
    println!("Keleusma REPL commands:");
    println!("  :help, :h               Show this help");
    println!("  :quit, :q, :exit        Exit the REPL");
    println!("  :reset                  Clear the session prefix");
    println!("  :show                   Display the current session prefix");
    println!();
    println!("Otherwise, type:");
    println!("  An expression to evaluate it (`1 + 2`, `double(21)`)");
    println!(
        "  A declaration to add to the session prefix (`fn`, `struct`, `enum`, `trait`, `impl`, `use`)"
    );
}

/// Decide whether a REPL line is a declaration (added to the prefix)
/// or an expression (evaluated against the prefix).
fn is_declaration(line: &str) -> bool {
    let starters = [
        "fn ", "yield ", "loop ", "struct ", "enum ", "trait ", "impl ", "use ", "data ",
    ];
    starters.iter().any(|s| line.starts_with(s))
}

fn evaluate_repl_input(prefix: &mut String, line: &str) {
    if is_declaration(line) {
        // Tentatively append; verify it parses and compiles within
        // the prefix before committing.
        let candidate = format!("{}\n{}", prefix.trim_end(), line);
        let candidate = candidate.trim().to_string();
        // Add a trivial main if the prefix lacks one so compilation
        // can proceed. The trivial main is dropped before commit.
        let probe = if has_main(&candidate) {
            candidate.clone()
        } else {
            format!("{}\n\nfn main() -> Word {{ 0 }}\n", candidate)
        };
        match compile_source(&probe) {
            Ok(_) => {
                *prefix = candidate;
                if let Some(name) = extract_decl_name(line) {
                    println!("defined: {}", name);
                } else {
                    println!("declaration accepted");
                }
            }
            Err(e) => {
                eprintln!("error: {}", e);
            }
        }
        return;
    }

    // Expression. Try wrapping with each candidate return type until
    // one type-checks. Run the first that compiles.
    for return_type in REPL_RETURN_TYPES {
        let body = if *return_type == "()" {
            format!("{}; ()", line)
        } else {
            line.to_string()
        };
        let program = format!(
            "{}\n\nfn main() -> {} {{ {} }}\n",
            prefix.trim_end(),
            return_type,
            body
        );
        if execute_source(&program).is_ok() {
            return;
        }
    }
    // None of the types worked. Run with i64 to surface the actual
    // error message to the user.
    let program = format!(
        "{}\n\nfn main() -> Word {{ {} }}\n",
        prefix.trim_end(),
        line
    );
    if let Err(e) = execute_source(&program) {
        eprintln!("error: {}", e);
    }
}

fn has_main(source: &str) -> bool {
    // Very rough heuristic: look for "fn main", "yield main", or
    // "loop main" at the start of any line. Sufficient for REPL
    // pipelining where users either declare a main themselves or
    // expect the REPL to wrap.
    source.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("fn main")
            || trimmed.starts_with("yield main")
            || trimmed.starts_with("loop main")
    })
}

fn extract_decl_name(line: &str) -> Option<String> {
    // For declarations, extract the name following the keyword. Used
    // for the REPL "defined: name" feedback.
    let mut tokens = line.split_whitespace();
    let kw = tokens.next()?;
    let name = match kw {
        "fn" | "yield" | "loop" | "struct" | "enum" | "trait" | "data" => {
            let next = tokens.next()?;
            // The name may have boundary characters (`(`, `<`, `{`,
            // `:`) attached without a space. Split at the first such
            // character and take the prefix.
            let end = next.find(['(', '<', '{', ':']).unwrap_or(next.len());
            next[..end].to_string()
        }
        "impl" => {
            // `impl Trait for Type { ... }` -- show "impl Trait for Type".
            let rest = tokens.collect::<Vec<&str>>().join(" ");
            let head = rest.split('{').next().unwrap_or(&rest).trim().to_string();
            format!("impl {}", head)
        }
        "use" => {
            let next = tokens.next()?;
            format!("use {}", next.trim_end_matches([';', ','].as_ref()))
        }
        _ => return None,
    };
    Some(name)
}

/// Compile a complete source program through the standard pipeline,
/// returning either the resulting `Module` or a stringified error
/// with location information.
fn compile_source(source: &str) -> Result<keleusma::bytecode::Module, String> {
    let tokens = tokenize(source).map_err(|e| format_err("lex", &e.message, e.span))?;
    let program = parse(&tokens).map_err(|e| format_err("parse", &e.message, e.span))?;
    let module = compile(&program).map_err(|e| format_err("compile", &e.message, e.span))?;
    Ok(module)
}

/// Run a source program through compile and execute. The runner
/// pre-registers utility and math natives so scripts can use
/// `to_string`, `length`, `concat`, `slice`, `println`, and the
/// `math::*` family without explicit registration.
fn execute_source(source: &str) -> Result<(), String> {
    let module = compile_source(source)?;
    let entry_kind = detect_entry_kind(&module)?;
    let persistent_bytes = keleusma::vm::required_persistent_capacity_for(&module);
    let transient_bytes =
        keleusma::vm::auto_arena_capacity_for(&module, &[]).unwrap_or(DEFAULT_ARENA_CAPACITY);
    let total = (persistent_bytes + transient_bytes).max(DEFAULT_ARENA_CAPACITY);
    let mut arena = Arena::with_capacity(total);
    arena
        .resize_persistent(persistent_bytes)
        .map_err(|e| format!("arena: resize_persistent: {:?}", e))?;
    let mut vm = Vm::new(module, &arena).map_err(|e| format!("verify: {:?}", e))?;
    drive_to_completion(&mut vm, &arena, entry_kind)
}

fn execute_bytecode(
    bytes: &[u8],
    verifying_keys: &[ed25519_dalek::VerifyingKey],
    decryption_keys: &[[u8; X25519_PRIVATE_KEY_LEN]],
    policy: &PolicyContext,
) -> Result<(), String> {
    let signed = keleusma::wire_format::header_requires_signature(bytes);
    let encrypted = keleusma::wire_format::header_requires_encryption(bytes);

    // In strict signing mode, unsigned bytecode is rejected
    // regardless of how it would normally load.
    if policy.strict_signing && !signed {
        return Err(String::from("strict mode: unsigned bytecode disabled"));
    }

    // In strict encryption mode, unencrypted bytecode is rejected
    // regardless of how it would normally load.
    if policy.strict_encryption && !encrypted {
        return Err(String::from("strict mode: unencrypted bytecode disabled"));
    }

    // Parse the Module first so we can inspect the entry chunk's
    // block type. This allows the runner to choose between the
    // atomic-fn-main path and the productive-divergent loop-main
    // path based on the script's signature.
    let module = load_module(bytes, verifying_keys, decryption_keys, policy)?;

    // Auto-size the arena based on the module's declared bounds.
    // The persistent portion holds the script's `.data` section;
    // the transient portion is sized to the worst-case stream-
    // iteration usage. Fall back to DEFAULT_ARENA_CAPACITY for
    // trivial modules to ensure a working minimum.
    let persistent_bytes = keleusma::vm::required_persistent_capacity_for(&module);
    let transient_bytes =
        keleusma::vm::auto_arena_capacity_for(&module, &[]).unwrap_or(DEFAULT_ARENA_CAPACITY);
    let total = (persistent_bytes + transient_bytes).max(DEFAULT_ARENA_CAPACITY);
    let mut arena = Arena::with_capacity(total);
    arena
        .resize_persistent(persistent_bytes)
        .map_err(|e| format!("arena: resize_persistent: {:?}", e))?;

    // Determine the script's entry-block kind. Loop main is driven
    // through the tick-counter convention; atomic fn main runs to
    // completion in a single call.
    let entry_kind = detect_entry_kind(&module)?;

    // Construct the VM. The module was parsed with the policy
    // checks already applied; signature verification (if signed)
    // happened during load_module. The flag is cleared in
    // load_module so Vm::new accepts the module without further
    // checks.
    let mut vm = Vm::new(module, &arena).map_err(|e| format!("verify: {:?}", e))?;
    for key in verifying_keys {
        vm.register_verifying_key(*key);
    }

    drive_to_completion(&mut vm, &arena, entry_kind)
}

/// Outcome of inspecting the loaded module's entry chunk. Drives
/// the dispatch between the atomic-fn-main runner and the
/// productive-divergent loop-main runner.
#[derive(Debug, Clone, Copy)]
enum EntryKind {
    /// Atomic total function. Runs to completion in a single
    /// `vm.call(&[])` invocation. Returns a `Value::Finished`.
    AtomicFn,
    /// Productive divergent loop. Driven by the tick-counter
    /// convention. Terminates only on `shell::exit(code)` or
    /// SIGINT.
    LoopMain,
}

fn detect_entry_kind(module: &keleusma::bytecode::Module) -> Result<EntryKind, String> {
    let entry_idx = module
        .entry_point
        .ok_or_else(|| String::from("module has no entry point; cannot determine entry kind"))?;
    let entry = module
        .chunks
        .get(entry_idx)
        .ok_or_else(|| format!("entry_point {} out of bounds", entry_idx))?;
    use keleusma::bytecode::BlockType;
    match entry.block_type {
        BlockType::Func => {
            if entry.param_count != 0 {
                return Err(format!(
                    "fn main: CLI runner expects zero parameters; got {}",
                    entry.param_count
                ));
            }
            Ok(EntryKind::AtomicFn)
        }
        BlockType::Stream => {
            if entry.param_count != 1 {
                return Err(format!(
                    "loop main: CLI runner expects exactly one parameter (tick: Word); got {}",
                    entry.param_count
                ));
            }
            Ok(EntryKind::LoopMain)
        }
        BlockType::Reentrant => Err(String::from(
            "yield main: the CLI runner does not drive yield-shaped entry functions; \
             use fn main or loop main",
        )),
    }
}

/// Parse a Module from the on-disk bytes, applying the policy
/// gates for signature verification and decryption. Returns the
/// Module with the FLAG_REQUIRES_SIGNATURE flag cleared so the
/// caller can construct a Vm without the signed-module gate
/// triggering.
fn load_module(
    bytes: &[u8],
    verifying_keys: &[ed25519_dalek::VerifyingKey],
    decryption_keys: &[[u8; X25519_PRIVATE_KEY_LEN]],
    policy: &PolicyContext,
) -> Result<keleusma::bytecode::Module, String> {
    let signed = keleusma::wire_format::header_requires_signature(bytes);
    let encrypted = keleusma::wire_format::header_requires_encryption(bytes);

    if encrypted {
        if decryption_keys.is_empty() {
            return Err(String::from(
                "encrypted bytecode requires --decryption-key or an enrolled decryption-key store",
            ));
        }
        // Try each decryption key. The right key matches the
        // recipient_key_id; mismatched keys produce WrongRecipient.
        let mut last_err: Option<keleusma::bytecode::LoadError> = None;
        for key in decryption_keys {
            match keleusma::wire_format::decrypt_encrypted_signed_to_signed_bytes(
                bytes,
                verifying_keys,
                key,
            ) {
                Ok(plaintext) => {
                    let mut module = keleusma::bytecode::Module::from_bytes(&plaintext)
                        .map_err(|e| format!("decoded module: {:?}", e))?;
                    // Clear the signed flag so Vm::new accepts the
                    // module; signature verification already
                    // happened inside decrypt_encrypted_signed_to_signed_bytes.
                    module.flags &= !keleusma::wire_format::FLAG_REQUIRES_SIGNATURE;
                    return Ok(module);
                }
                Err(e) => last_err = Some(e),
            }
        }
        let err = last_err.expect("at least one key attempted");
        Err(if policy.strict_encryption {
            format!(
                "strict mode: no enrolled decryption key matches the artefact ({:?})",
                err
            )
        } else {
            format!("decrypt_encrypted_signed_to_signed_bytes: {:?}", err)
        })
    } else if signed {
        keleusma::wire_format::verify_module_signature(bytes, verifying_keys).map_err(|e| {
            if policy.strict_signing {
                format!(
                    "strict mode: signature does not match any enrolled key ({:?})",
                    e
                )
            } else {
                format!("verify_module_signature: {:?}", e)
            }
        })?;
        let mut module = keleusma::bytecode::Module::from_bytes(bytes)
            .map_err(|e| format!("module: {:?}", e))?;
        module.flags &= !keleusma::wire_format::FLAG_REQUIRES_SIGNATURE;
        Ok(module)
    } else {
        if !verifying_keys.is_empty() {
            return Err(String::from(
                "--verifying-key supplied but the bytecode does not carry FLAG_REQUIRES_SIGNATURE",
            ));
        }
        if !decryption_keys.is_empty() {
            return Err(String::from(
                "--decryption-key supplied but the bytecode does not carry FLAG_ENCRYPTED",
            ));
        }
        keleusma::bytecode::Module::from_bytes(bytes).map_err(|e| format!("module: {:?}", e))
    }
}

fn drive_to_completion(vm: &mut Vm, arena: &Arena, entry_kind: EntryKind) -> Result<(), String> {
    // Register the standard DSL bundles on every CLI-driven script.
    // Hosts that embed the library directly choose which libraries
    // to register; the CLI registers all of them so scripts run
    // from the command line have access to math, audio, shell, and
    // the bundled utility natives by default.
    keleusma::utility_natives::register_utility_natives(vm);
    // Override the bundled println with one that writes to stdout.
    // The library default is a no-op suitable for no_std hosts; the
    // CLI is std-only and benefits from real output.
    vm.register_native_closure("println", |args| {
        if let Some(arg) = args.first() {
            print_value_inline(arg);
        }
        println!();
        Ok(Value::Unit)
    });
    vm.register_library(stddsl::Math);
    vm.register_library(stddsl::Audio);
    vm.register_library(stddsl::Shell);

    match entry_kind {
        EntryKind::AtomicFn => drive_atomic_fn(vm, arena),
        EntryKind::LoopMain => drive_loop_main(vm, arena),
    }
}

/// Drive an `fn main()` to completion in a single call. Prints the
/// returned value and returns. Yielded or Reset states from an
/// atomic fn are unexpected and produce an error.
fn drive_atomic_fn(vm: &mut Vm, arena: &Arena) -> Result<(), String> {
    match vm.call(&[]).map_err(|e| format!("vm: {:?}", e))? {
        VmState::Finished(v) => {
            print_value(&v, arena);
            Ok(())
        }
        VmState::Yielded(v) => Err(format!(
            "fn main yielded unexpectedly (atomic fn should run to completion): {:?}",
            v
        )),
        VmState::Reset => Err(String::from(
            "fn main reset unexpectedly (atomic fn should run to completion)",
        )),
    }
}

/// Drive a `loop main(tick: Word) -> Word` indefinitely. Termination
/// happens only when the script calls `shell::exit(code)` (which
/// terminates the process via `std::process::exit`) or when the OS
/// delivers SIGINT (which terminates the process via the default
/// signal disposition).
///
/// Tick mechanics per the V0.2.1 convention:
/// - Initial call passes tick = 1.
/// - Script yields a `Word` value.
/// - Host computes `next_tick = yielded_value.wrapping_add(1)`.
/// - Host resumes with `Value::Int(next_tick)`.
/// - Yield value 0 produces next_tick 1 (reset-equivalent).
/// - Yield value `Word::MAX` produces next_tick 0 (overflow indicator).
///
/// Reset events (script triggers `Op::Reset`) are transparent to
/// the tick mechanism. The arena is cleared by the VM; the host
/// continues to the next iteration with the current tick state
/// preserved.
fn drive_loop_main(vm: &mut Vm, _arena: &Arena) -> Result<(), String> {
    let mut tick: i64 = 1;
    let mut state = vm
        .call(&[Value::Int(tick)])
        .map_err(|e| format!("vm: {:?}", e))?;
    loop {
        match state {
            VmState::Finished(v) => {
                return Err(format!(
                    "loop main finished unexpectedly (productive divergent loops should never return): {:?}",
                    v
                ));
            }
            VmState::Yielded(v) => {
                // The yielded value must be a Word per the loop
                // main signature. Anything else is an error.
                let yielded = match v {
                    Value::Int(n) => n,
                    other => {
                        return Err(format!(
                            "loop main yielded a non-Word value (signature requires Word): {:?}",
                            other
                        ));
                    }
                };
                tick = yielded.wrapping_add(1);
                state = vm
                    .resume(Value::Int(tick))
                    .map_err(|e| format!("vm: {:?}", e))?;
            }
            VmState::Reset => {
                // The script triggered a Reset (Op::Reset). The VM
                // has cleared the transient arena region; the
                // persistent .data section is preserved. Continue
                // the loop with the current tick state.
                state = vm
                    .resume(Value::Int(tick))
                    .map_err(|e| format!("vm: {:?}", e))?;
            }
        }
    }
}

fn format_err(stage: &str, msg: &str, span: keleusma::token::Span) -> String {
    if span.line == 0 && span.column == 0 {
        format!("{}: {}", stage, msg)
    } else {
        format!("{}: {}:{}: {}", stage, span.line, span.column, msg)
    }
}

/// Print a value to stdout without a trailing newline. Used by the
/// CLI's `println` override. The full `print_value` variant adds the
/// trailing newline.
fn print_value_inline(v: &Value) {
    match v {
        Value::Int(n) => print!("{}", n),
        Value::Float(f) => print!("{}", f),
        Value::Bool(b) => print!("{}", b),
        Value::StaticStr(s) => print!("{}", s),
        Value::Unit => print!("()"),
        Value::None => print!("None"),
        other => print!("{:?}", other),
    }
}

fn print_value(v: &Value, arena: &Arena) {
    match v {
        Value::Int(n) => println!("{}", n),
        Value::Float(f) => println!("{}", f),
        Value::Bool(b) => println!("{}", b),
        Value::StaticStr(s) => println!("{}", s),
        Value::KStr(h) => match h.get(arena) {
            Ok(s) => println!("{}", s),
            Err(_) => println!("<stale KStr>"),
        },
        Value::Unit => println!("()"),
        Value::None => println!("None"),
        other => println!("{:?}", other),
    }
}
