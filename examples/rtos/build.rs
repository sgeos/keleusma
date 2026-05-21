//! Build script. Two responsibilities, both target-conditional.
//!
//! Linker flags. When `CARGO_CFG_TARGET_OS == "none"` (the
//! bare-metal target), the script emits `--nmagic`, `-Tlink.x`,
//! and `-Tdefmt.x` so the embedded binary links correctly. The
//! host-side `three-task-std` binary builds under `target_os =
//! "macos"` or similar and bypasses these arguments entirely.
//!
//! Script precompilation. When the runtime `keleusma-compile`
//! feature is off, the runtime image cannot tokenize, parse,
//! and compile scripts at boot. This script then invokes the
//! parent crate's compile pipeline (pulled in through the
//! `[build-dependencies]` entry with `compile` and `verify`
//! enabled) on every `.kel` source under `scripts/`, including
//! the prelude prepended at the head of each, and writes the
//! resulting bytecode to `$OUT_DIR/<name>.kel.bin`. The
//! microkernel's `setup` module then loads through
//! `include_bytes!` plus `Module::from_bytes`.
//!
//! When `keleusma-compile` is on, this script does nothing
//! beyond the linker flags; the runtime carries the compile
//! pipeline and tokenises the source at boot. Source mode is
//! the default and matches the host demonstrator's behaviour.

use std::env;

fn main() {
    emit_link_args();
    #[cfg(not(feature = "keleusma-compile"))]
    precompile_scripts();
    #[cfg(feature = "keleusma-signatures")]
    emit_signed_self_test();
}

/// Generate a signed bytecode blob plus the matching public key
/// at build time so the runtime image carries a deterministic
/// fixture that exercises `wire_format::verify_module_signature`
/// at boot. The seed is hardcoded (`[0xA5; 32]`); the test is
/// strictly for hardware coverage of the cryptographic path and
/// must not be reused for any operational signing identity.
///
/// Produces two files in `OUT_DIR`:
/// - `signed_self_test.kel.bin`: the signed bytecode.
/// - `signed_self_test_pub.bin`: the 32-byte verifying key.
///
/// Gated on the `keleusma-signatures` cargo feature so builds
/// without it pay no ed25519-dalek compile cost; the setup
/// module's `include_bytes!` references are gated on the same
/// feature.
#[cfg(feature = "keleusma-signatures")]
fn emit_signed_self_test() {
    use std::fs;
    use std::path::PathBuf;

    use ed25519_dalek::SigningKey;
    use keleusma::compiler::compile;
    use keleusma::lexer::tokenize;
    use keleusma::parser::parse;
    use keleusma::wire_format::module_to_signed_wire_bytes;

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));

    let src = "signed fn main() -> Word { 42 }";
    let tokens = tokenize(src).expect("self-test: lex");
    let program = parse(&tokens).expect("self-test: parse");
    let module = compile(&program).expect("self-test: compile");

    let seed: [u8; 32] = [0xA5; 32];
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();
    let bytes = module_to_signed_wire_bytes(&module, &signing_key).expect("self-test: sign");

    fs::write(out_dir.join("signed_self_test.kel.bin"), &bytes)
        .expect("self-test: write signed bytes");
    fs::write(
        out_dir.join("signed_self_test_pub.bin"),
        verifying_key.to_bytes(),
    )
    .expect("self-test: write verifying key");
}

fn emit_link_args() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "none" {
        println!("cargo:rustc-link-arg-bins=--nmagic");
        println!("cargo:rustc-link-arg-bins=-Tlink.x");
        println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
        println!("cargo:rerun-if-changed=memory.x");
    }
}

#[cfg(not(feature = "keleusma-compile"))]
fn precompile_scripts() {
    use std::fs;
    use std::path::PathBuf;

    use keleusma::compiler::compile;
    use keleusma::lexer::tokenize;
    use keleusma::parser::parse;

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let scripts_dir = manifest_dir.join("scripts");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));

    let prelude_path = scripts_dir.join("prelude.kel");
    let prelude_src = fs::read_to_string(&prelude_path)
        .unwrap_or_else(|e| panic!("read prelude {:?}: {}", prelude_path, e));
    println!("cargo:rerun-if-changed={}", prelude_path.display());

    for name in ["led", "sensor", "heartbeat", "event_listener", "faulty"] {
        let src_path = scripts_dir.join(format!("{}.kel", name));
        println!("cargo:rerun-if-changed={}", src_path.display());

        let task_src = fs::read_to_string(&src_path)
            .unwrap_or_else(|e| panic!("read script {:?}: {}", src_path, e));
        let combined = format!("{}\n{}", prelude_src, task_src);

        let tokens = tokenize(&combined).unwrap_or_else(|e| panic!("lex {}: {:?}", name, e));
        let program = parse(&tokens).unwrap_or_else(|e| panic!("parse {}: {:?}", name, e));
        let module = compile(&program).unwrap_or_else(|e| panic!("compile {}: {:?}", name, e));
        let bytes = module
            .to_bytes()
            .unwrap_or_else(|e| panic!("serialize {}: {:?}", name, e));

        let out_path = out_dir.join(format!("{}.kel.bin", name));
        fs::write(&out_path, &bytes).unwrap_or_else(|e| panic!("write {:?}: {}", out_path, e));
    }
}
