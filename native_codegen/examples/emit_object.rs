//! Emit a native object file and a C header from a Keleusma policy.
//!
//! Usage: `cargo run --example emit_object -- <policy.kel> <out-dir>`
//!
//! # Why a header is emitted rather than documented
//!
//! The operator's ruling on the fixed-point shared-slot ABI is that the host is
//! expected to know the interpretation of the bits, on the analogy of a C header
//! laying out the contract for a separately compiled procedure. **Emitting the
//! header from the module makes that contract derived rather than transcribed**,
//! so it cannot drift from the layout it describes.
//!
//! This is an EXAMPLE, not a specification. How the layout is communicated in
//! general is an open question the operator has explicitly left open.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};

fn main() {
    let mut args = std::env::args().skip(1);
    let src_path = args
        .next()
        .expect("usage: emit_object <policy.kel> <out-dir>");
    let out_dir = args
        .next()
        .expect("usage: emit_object <policy.kel> <out-dir>");

    let src = std::fs::read_to_string(&src_path).expect("read policy");
    let module = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(&src).expect("lex")).expect("parse"),
    )
    .expect("compile");

    let entry = module.entry_point.expect("the policy needs an entry point");
    let sym = format!("kel_chunk_{entry}");

    let ctx = Context::create();
    let lm = ctx.create_module("policy");
    keleusma_native::lower_module(&ctx, &lm, &module, keleusma_native::LowerOptions::default())
        .expect("lower");
    lm.verify().expect("LLVM module verification");

    Target::initialize_native(&InitializationConfig::default()).expect("target init");
    let triple = TargetMachine::get_default_triple();
    let tm = Target::from_triple(&triple)
        .expect("target")
        .create_target_machine(
            &triple,
            &TargetMachine::get_host_cpu_name().to_string_lossy(),
            &TargetMachine::get_host_cpu_features().to_string_lossy(),
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine");

    std::fs::create_dir_all(&out_dir).expect("out dir");
    let obj = format!("{out_dir}/policy.o");
    tm.write_to_file(&lm, FileType::Object, std::path::Path::new(&obj))
        .expect("write object");

    // The header, derived from the module rather than transcribed.
    let shared_bytes = keleusma::vm::shared_data_bytes_for(&module);
    let mut h = String::new();
    h.push_str("/* GENERATED from policy.kel by `cargo run --example emit_object`.\n");
    h.push_str(" * Do not edit. The layout below is read out of the compiled module,\n");
    h.push_str(" * so it cannot drift from the code it describes. */\n");
    h.push_str("#ifndef KEL_POLICY_H\n#define KEL_POLICY_H\n#include <stdint.h>\n\n");
    h.push_str(&format!(
        "#define KEL_SHARED_BYTES {shared_bytes}\n#define KEL_ENTRY {sym}\n\n"
    ));
    if let Some(dl) = module.data_layout.as_ref() {
        h.push_str("/* Shared slots: offset, width and how to read the bits.\n");
        h.push_str(" * A Fixed slot is a two's-complement integer whose scale is\n");
        h.push_str(" * stated here and NOT carried in the value. */\n");
        // **ARRAY ELEMENTS SHARE A SLOT NAME AND C REJECTS A REDEFINITION.**
        // The first version emitted `KEL_IO_ZONE_TEMP_OFFSET` three times with
        // three different values, which is not a warning but an error, and the
        // header would not have compiled. Found by reading the generated file
        // rather than by trusting the generator. Names that repeat are suffixed
        // with the element index; names that do not are left alone, so the
        // common case stays readable.
        let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for sl in dl.slots.iter().take(dl.shared_layout.len()) {
            *counts.entry(sl.name.as_str()).or_default() += 1;
        }
        let mut seen: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for (i, e) in dl.shared_layout.iter().enumerate() {
            // **THE SLOT'S OWN NAME, NOT ITS INDEX.** A header of numbered
            // offsets is a table; a header of named offsets is a contract, and
            // the name is what a host programmer writes against. Sanitised for
            // C, since a Keleusma path like `io.zone_temp[0]` is not an
            // identifier.
            let raw = dl.slots.get(i).map(|s| s.name.as_str()).unwrap_or("slot");
            let mut name: String = raw
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_uppercase()
                    } else {
                        '_'
                    }
                })
                .collect();
            if counts.get(raw).copied().unwrap_or(0) > 1 {
                let n = seen.entry(raw).or_default();
                name.push_str(&format!("_{n}"));
                *n += 1;
            }
            let (kind, how) = match e.kind {
                1 => ("bool", "0 or 1, one byte"),
                2 => ("Byte", "unsigned, one byte"),
                3 => ("Word", "int64_t"),
                4 => ("Fixed", "int64_t of Q-format bits; divide by 2^F for units"),
                5 => ("Float", "IEEE-754, little-endian"),
                _ => ("other", "not lowered by this backend"),
            };
            h.push_str(&format!(
                "#define KEL_{name}_OFFSET {:<6} /* {kind}: {how} */\n",
                e.offset
            ));
        }
    }
    h.push_str("\nint64_t ");
    h.push_str(&sym);
    h.push_str("(int64_t a, int64_t b, unsigned char *shared,\n");
    h.push_str("                          unsigned char *private_region,\n");
    h.push_str("                          unsigned char *composite_region);\n");
    h.push_str("\n#endif\n");
    std::fs::write(format!("{out_dir}/policy.h"), h).expect("write header");

    // **THE GUARANTEE, SHOWN RATHER THAN ASSERTED.** The point of this example
    // is not that the policy runs but that its worst case was known before it
    // ran. These come from the verifier, not from a claim in prose.
    println!();
    println!("PROVEN BOUNDS, from the verifier rather than from prose:");
    for (i, c) in module.chunks.iter().enumerate() {
        match keleusma::verify::wcet_whole_chunk(c) {
            Ok(t) => println!("  chunk {i} ({:<12}) WCET {t:>6} cost units", c.name),
            Err(e) => println!("  chunk {i} ({}) WCET unprovable: {}", c.name, e.message),
        }
    }
    // **MODULE-LEVEL MEMORY, NOT THE STREAM-ITERATION CALL.** This policy is a
    // plain function called once per control cycle rather than a stream, so
    // `wcmu_stream_iteration` correctly refuses it for want of a `Stream` block.
    // Reporting that refusal as a limitation would have been a misread of the
    // tool rather than a fact about the policy.
    match keleusma::verify::module_wcmu(&module, &[]) {
        Ok(per_chunk) => {
            let stack = per_chunk.iter().map(|(s, _)| *s).max().unwrap_or(0);
            let heap = per_chunk.iter().map(|(_, h)| *h).max().unwrap_or(0);
            println!("  worst case over all chunks: stack {stack} B, heap {heap} B");
        }
        Err(e) => println!("  module WCMU unprovable: {}", e.message),
    }
    println!("  shared segment {shared_bytes} B, preallocated by the host and never grown");
    println!("  NOTHING GROWS AT RUN TIME: the host supplies every region up front.");
    println!();

    println!("wrote {obj}");
    println!("wrote {out_dir}/policy.h");
    println!("entry symbol: {sym}");
    println!("shared bytes: {shared_bytes}");
}
