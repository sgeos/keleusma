//! Emit a coroutine library object, its C header and its Rust bindings.
//!
//! Usage: `cargo run --example emit_static_lib -- <source.kel> <prefix> <out-dir>`
//!
//! Writes `<prefix>.o`, `<prefix>.h` and `<prefix>.rs` into the output
//! directory. Archive the object as `lib<prefix>.a` with the platform `ar`.
//! `examples/lexer_host/run.sh` does that and links a Rust host against it.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use keleusma_native::coroutine::HostContracts;
use keleusma_native::linkage;

fn main() {
    let usage = "usage: emit_static_lib <source.kel> <prefix> <out-dir>";
    let mut args = std::env::args().skip(1);
    let (Some(src_path), Some(prefix), Some(out_dir)) = (args.next(), args.next(), args.next())
    else {
        eprintln!("{usage}");
        std::process::exit(64);
    };
    let src = std::fs::read_to_string(&src_path).expect("read source");
    let module = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(&src).expect("lex")).expect("parse"),
    )
    .expect("compile");

    Target::initialize_native(&InitializationConfig::default()).expect("target init");
    let triple = TargetMachine::get_default_triple();
    let tm = Target::from_triple(&triple)
        .expect("target")
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine");

    let ctx = Context::create();
    let lib = linkage::build(&ctx, &module, &tm, &prefix, &HostContracts::default())
        .unwrap_or_else(|e| panic!("build {prefix}: {e:?}"));
    let out = std::path::Path::new(&out_dir);
    std::fs::create_dir_all(out).expect("out dir");
    lib.write_object(&tm, &out.join(format!("{prefix}.o")), true)
        .expect("write object");
    std::fs::write(out.join(format!("{prefix}.h")), lib.c_header()).expect("write header");
    std::fs::write(out.join(format!("{prefix}.rs")), lib.rust_bindings()).expect("write bindings");
    println!(
        "{prefix}: frame {} B, slot {} B, shared {} B, private {} B, region {} B, trap verdict {}",
        lib.frame_bytes,
        lib.slot_bytes,
        lib.shared_bytes,
        lib.private_bytes,
        lib.region_bytes,
        lib.trap.code()
    );
}
