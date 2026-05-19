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

        let tokens =
            tokenize(&combined).unwrap_or_else(|e| panic!("lex {}: {:?}", name, e));
        let program =
            parse(&tokens).unwrap_or_else(|e| panic!("parse {}: {:?}", name, e));
        let module =
            compile(&program).unwrap_or_else(|e| panic!("compile {}: {:?}", name, e));
        let bytes = module
            .to_bytes()
            .unwrap_or_else(|e| panic!("serialize {}: {:?}", name, e));

        let out_path = out_dir.join(format!("{}.kel.bin", name));
        fs::write(&out_path, &bytes)
            .unwrap_or_else(|e| panic!("write {:?}: {}", out_path, e));
    }
}
