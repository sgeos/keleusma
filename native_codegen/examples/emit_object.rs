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
    // ── The host's half of the contract ───────────────────────────────────
    //
    // **DERIVED FROM THE LOWERED MODULE, NOT FROM A PARALLEL COMPUTATION.**
    //
    // What this buys was MEASURED rather than assumed, and the obvious claim
    // is false: a declaration does NOT turn a missing definition into a
    // compile error. It turns a MISMATCHED definition into one. Without the
    // header, a host defining a native with the wrong arity compiles clean
    // and passes garbage across the ABI; with it, `conflicting types`. That
    // mirrors the backend's own refusal when two call sites disagree on a
    // native's arity, so both sides of the boundary now check the same thing.
    // The brief for this proposed exposing the backend's name-mangling function
    // so the header could call it. Implementing it found a better source: the
    // module about to be written to the object already carries each native's
    // declaration — mangled name, arity and return type — so reading them here
    // means the header is derived from the artefact rather than computed
    // alongside it. It cannot drift, because there is nothing to drift from.
    //
    // **Arity is not available any other way.** A module records native NAMES
    // and return shapes; the argument count comes from the call sites and is
    // resolved during lowering. Emitting a prototype from the name alone would
    // have been the invented signature the brief forbids — and C would have
    // accepted the mismatched call and passed the wrong thing.
    //
    // **This covers the HOST half only.** A linked object also needs
    // compiler-runtime and C-library symbols — `__divti3` and `bzero` on this
    // host, eleven others on `thumbv8m.main-none-eabihf` — which an embedder
    // does not declare. Those are recorded in
    // `docs/decisions/LINKAGE_SYMBOL_CENSUS.md` and
    // `docs/decisions/NARROW_TARGET_LINKAGE.md`.
    let natives = keleusma_native::host_native_declarations(&lm);

    if natives.is_empty() {
        h.push_str("\n/* This policy requires no host natives. */\n");
    } else {
        h.push_str(concat!(
            "\n/* THE HOST MUST DEFINE THESE. Each is a native this policy calls.\n",
            "\n",
            "   MEASURED, because the obvious claim is false. Declaring these does\n",
            "   NOT make a MISSING definition a compile error -- that stays a link\n",
            "   error. What it catches is a MISMATCHED one: a definition with the\n",
            "   wrong arity or return type is a `conflicting types` error here,\n",
            "   where without the declaration it compiles clean and passes garbage\n",
            "   across the ABI. Verified both ways with a C compiler.\n",
            "\n",
            "   Arity and return type are read from the emitted object's own\n",
            "   declarations, so they cannot disagree with it.\n",
            "\n",
            "   This is the HOST half of the contract only. A linked binary also\n",
            "   needs compiler-runtime and C-library symbols, which you do not\n",
            "   declare; see docs/decisions/LINKAGE_SYMBOL_CENSUS.md. */\n",
        ));
        for (name, argc, returns_float) in &natives {
            let ret = if *returns_float { "double" } else { "int64_t" };
            let args = if *argc == 0 {
                String::from("void")
            } else {
                (0..*argc).map(|_| "int64_t").collect::<Vec<_>>().join(", ")
            };
            h.push_str(&format!("{ret} {name}({args});\n"));
        }
    }

    h.push_str("\n#endif\n");
    std::fs::write(format!("{out_dir}/policy.h"), h).expect("write header");

    // **THE GUARANTEE, SHOWN RATHER THAN ASSERTED.** The point of this example
    // is not that the policy runs but that its worst case was known before it
    // ran. These come from the verifier, not from a claim in prose.
    //
    // # ⚠ THE TWO HALVES ARE NOT THE SAME KIND OF CLAIM, AND THIS BLOCK USED TO
    // # PRESENT THEM AS THOUGH THEY WERE
    //
    // The MEMORY figures transfer to the object emitted here, and
    // `tests/bound_transfer.rs` measures that: provisioned operand slots against
    // `max_operand_slots`, and `region_total_bytes` against `max_heap_bytes`.
    //
    // **The TIME figure is measured against nothing.** It is a bytecode-level
    // count under a cost model calibrated for the VIRTUAL MACHINE. Printing it
    // beside bounds that genuinely transfer, under one heading, in an example
    // whose subject is a C host linking a NATIVE object, invited it to be read
    // as a bound on native execution. **Nothing establishes that.**
    //
    // The figure is kept because it is TRUE ABOUT THE BYTECODE and useful. Only
    // its subject is now stated. See `docs/decisions/NATIVE_WCET_ASYMMETRY.md`.
    println!();
    println!("TWO DIFFERENT KINDS OF CLAIM, KEPT APART:");
    println!();
    // **MODULE-LEVEL MEMORY, NOT THE STREAM-ITERATION CALL.** This policy is a
    // plain function called once per control cycle rather than a stream, so
    // `wcmu_stream_iteration` correctly refuses it for want of a `Stream` block.
    // Reporting that refusal as a limitation would have been a misread of the
    // tool rather than a fact about the policy.
    println!("MEMORY, which describes THIS OBJECT. The backend's provisioning is");
    println!("checked against these figures by the bound-transfer tests.");
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
    println!("BYTECODE COST, from the verifier's virtual-machine cost model:");
    for (i, c) in module.chunks.iter().enumerate() {
        match keleusma::verify::wcet_whole_chunk(c) {
            Ok(t) => println!("  chunk {i} ({:<12}) {t:>6} cost units", c.name),
            Err(e) => println!("  chunk {i} ({}) cost unprovable: {}", c.name, e.message),
        }
    }
    println!("  ^ NOT A BOUND ON NATIVE EXECUTION TIME. It counts bytecode under the");
    println!("    interpreter's cost model. No measurement in this project relates it");
    println!("    to the machine code emitted above, and that code may call");
    println!("    compiler-runtime routines with no bytecode counterpart at all.");
    println!();
    println!();

    println!("wrote {obj}");
    println!("wrote {out_dir}/policy.h");
    println!("entry symbol: {sym}");
    println!("shared bytes: {shared_bytes}");
}
